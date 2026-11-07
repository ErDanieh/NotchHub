//! File converter: images (with a weight reducer), and the commands for audio and video (ffmpeg).
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Pdf,
    Audio,
    Video,
    Other,
}

pub fn kind_of(path: &Path) -> Kind {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("jpg" | "jpeg" | "png" | "webp" | "tif" | "tiff" | "bmp" | "gif") => Kind::Image,
        Some("pdf") => Kind::Pdf,
        Some("mp3" | "m4a" | "aac" | "ogg" | "opus" | "flac" | "wav" | "wma") => Kind::Audio,
        Some("mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "flv") => Kind::Video,
        _ => Kind::Other,
    }
}

/// Output file next to the original with a "-converted" suffix; never overwrites an existing file.
pub fn output_path(src: &Path, ext: &str) -> PathBuf {
    let dir = src.parent().unwrap_or_else(|| Path::new("."));
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let mut p = dir.join(format!("{stem}-converted.{ext}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem}-converted-{n}.{ext}"));
        n += 1;
    }
    p
}

pub fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

// MARK: images

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImgFormat {
    Jpg,
    Png,
    Webp,
    Tiff,
    Bmp,
}

impl ImgFormat {
    pub fn ext(self) -> &'static str {
        match self {
            ImgFormat::Jpg => "jpg",
            ImgFormat::Png => "png",
            ImgFormat::Webp => "webp",
            ImgFormat::Tiff => "tiff",
            ImgFormat::Bmp => "bmp",
        }
    }
}

fn flatten_for_jpeg(img: DynamicImage) -> DynamicImage {
    // JPEG has no transparency: put the picture on white
    if !img.color().has_alpha() {
        return DynamicImage::ImageRgb8(img.to_rgb8());
    }
    let rgba = img.to_rgba8();
    let mut out = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as f32 / 255.0;
        let mix = |c: u8| (c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        out.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
    }
    DynamicImage::ImageRgb8(out)
}

pub fn encode_jpeg(img: &DynamicImage, quality: u8) -> Vec<u8> {
    let rgb = flatten_for_jpeg(img.clone()).to_rgb8();
    let mut buf = vec![];
    let _ = JpegEncoder::new_with_quality(&mut buf, quality).encode_image(&rgb);
    buf
}

/// Converts one image. `quality` (1..=100) applies to JPG.
pub fn convert_image(src: &Path, fmt: ImgFormat, quality: u8) -> Result<PathBuf, String> {
    let img = image::open(src).map_err(|e| e.to_string())?;
    let dst = output_path(src, fmt.ext());
    match fmt {
        ImgFormat::Jpg => std::fs::write(&dst, encode_jpeg(&img, quality)).map_err(|e| e.to_string())?,
        ImgFormat::Png => img.save_with_format(&dst, ImageFormat::Png).map_err(|e| e.to_string())?,
        ImgFormat::Webp => img.save_with_format(&dst, ImageFormat::WebP).map_err(|e| e.to_string())?,
        ImgFormat::Tiff => img.save_with_format(&dst, ImageFormat::Tiff).map_err(|e| e.to_string())?,
        ImgFormat::Bmp => img.save_with_format(&dst, ImageFormat::Bmp).map_err(|e| e.to_string())?,
    }
    Ok(dst)
}

/// The weight reducer: the best JPEG that fits in `target_bytes`. First lowers the quality, and if that is
/// not enough, shrinks the picture a little at a time (never below 320 px on the long side).
pub fn shrink_to(src: &Path, target_bytes: u64) -> Result<(PathBuf, u64), String> {
    let original = image::open(src).map_err(|e| e.to_string())?;
    let mut img = flatten_for_jpeg(original);
    let (mut best, mut best_size): (Vec<u8>, u64) = (vec![], u64::MAX);
    loop {
        // binary search for the highest quality that fits
        let (mut lo, mut hi) = (25u8, 95u8);
        let mut fit: Option<Vec<u8>> = None;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            let data = encode_jpeg(&img, mid);
            if (data.len() as u64) <= target_bytes {
                fit = Some(data);
                lo = mid + 1;
            } else {
                if (data.len() as u64) < best_size {
                    best_size = data.len() as u64;
                    best = data;
                }
                if mid == 0 {
                    break;
                }
                hi = mid - 1;
            }
        }
        if let Some(data) = fit {
            let dst = output_path(src, "jpg");
            let n = data.len() as u64;
            std::fs::write(&dst, data).map_err(|e| e.to_string())?;
            return Ok((dst, n));
        }
        let long = img.width().max(img.height());
        if long <= 320 {
            // cannot get any smaller: save the best we had
            let dst = output_path(src, "jpg");
            std::fs::write(&dst, &best).map_err(|e| e.to_string())?;
            return Ok((dst, best.len() as u64));
        }
        let (w, h) = ((img.width() as f32 * 0.85) as u32, (img.height() as f32 * 0.85) as u32);
        img = img.resize_exact(w.max(1), h.max(1), FilterType::Lanczos3);
    }
}

// MARK: audio and video (ffmpeg)

pub fn ffmpeg_path() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join("ffmpeg")).find(|f| f.is_file()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaFormat {
    Mp3,
    M4a,
    Ogg,
    Flac,
    Wav,
    Mp4,
    Webm,
    Gif,
}

impl MediaFormat {
    pub fn ext(self) -> &'static str {
        match self {
            MediaFormat::Mp3 => "mp3",
            MediaFormat::M4a => "m4a",
            MediaFormat::Ogg => "ogg",
            MediaFormat::Flac => "flac",
            MediaFormat::Wav => "wav",
            MediaFormat::Mp4 => "mp4",
            MediaFormat::Webm => "webm",
            MediaFormat::Gif => "gif",
        }
    }
    /// Does the result keep the picture?
    pub fn has_video(self) -> bool {
        matches!(self, MediaFormat::Mp4 | MediaFormat::Webm | MediaFormat::Gif)
    }
}

/// The ffmpeg arguments (after the program name) for a conversion.
pub fn ffmpeg_args(src: &Path, dst: &Path, fmt: MediaFormat) -> Vec<String> {
    let mut a: Vec<String> = ["-nostdin", "-y", "-hide_banner", "-loglevel", "error", "-progress", "pipe:1", "-nostats", "-i"].iter().map(|s| s.to_string()).collect();
    a.push(src.to_string_lossy().to_string());
    let codec: &[&str] = match fmt {
        MediaFormat::Mp3 => &["-vn", "-c:a", "libmp3lame", "-q:a", "2"],
        MediaFormat::M4a => &["-vn", "-c:a", "aac", "-b:a", "192k"],
        MediaFormat::Ogg => &["-vn", "-c:a", "libvorbis", "-q:a", "5"],
        MediaFormat::Flac => &["-vn", "-c:a", "flac"],
        MediaFormat::Wav => &["-vn", "-c:a", "pcm_s16le"],
        MediaFormat::Mp4 => &["-c:v", "libx264", "-preset", "medium", "-crf", "23", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart"],
        MediaFormat::Webm => &["-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0", "-c:a", "libopus"],
        MediaFormat::Gif => &["-vf", "fps=12,scale=480:-1:flags=lanczos", "-an"],
    };
    a.extend(codec.iter().map(|s| s.to_string()));
    a.push(dst.to_string_lossy().to_string());
    a
}

fn duration_seconds(src: &Path) -> Option<f64> {
    let o = Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"]).arg(src).output().ok()?;
    String::from_utf8_lossy(&o.stdout).trim().parse().ok()
}

/// Reads ffmpeg's `-progress` output: the `out_time_us=` lines.
pub fn parse_progress_line(line: &str) -> Option<f64> {
    line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms=")).and_then(|v| v.trim().parse::<f64>().ok()).map(|us| us / 1_000_000.0)
}

/// Runs ffmpeg, reporting progress 0..1. Blocking: call it from a worker thread.
pub fn run_ffmpeg(src: &Path, fmt: MediaFormat, mut progress: impl FnMut(f64)) -> Result<PathBuf, String> {
    let ff = ffmpeg_path().ok_or("ffmpeg is not installed. Install it with: sudo apt install ffmpeg")?;
    let dst = output_path(src, fmt.ext());
    let total = duration_seconds(src);
    let mut child = Command::new(ff).args(ffmpeg_args(src, &dst, fmt)).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    if let Some(out) = child.stdout.take() {
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let (Some(t), Some(total)) = (parse_progress_line(&line), total) {
                progress((t / total.max(0.001)).clamp(0.0, 0.99));
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        progress(1.0);
        Ok(dst)
    } else {
        let _ = std::fs::remove_file(&dst);
        Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("ffmpeg failed").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nh-conv-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A noisy picture, so it does not compress to nothing.
    fn noisy(path: &Path, w: u32, h: u32) {
        let mut seed = 12345u32;
        let img = RgbImage::from_fn(w, h, |x, y| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let n = (seed >> 24) as u8;
            Rgb([(x * 255 / w) as u8 ^ n / 4, (y * 255 / h) as u8, n])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn kinds_by_extension() {
        assert_eq!(kind_of(Path::new("a.JPG")), Kind::Image);
        assert_eq!(kind_of(Path::new("a.pdf")), Kind::Pdf);
        assert_eq!(kind_of(Path::new("a.flac")), Kind::Audio);
        assert_eq!(kind_of(Path::new("a.mkv")), Kind::Video);
        assert_eq!(kind_of(Path::new("a.xyz")), Kind::Other);
    }

    #[test]
    fn output_never_overwrites() {
        let d = tmp("out");
        let src = d.join("pic.png");
        std::fs::write(&src, "x").unwrap();
        let a = output_path(&src, "jpg");
        assert!(a.ends_with("pic-converted.jpg"));
        std::fs::write(&a, "x").unwrap();
        assert!(output_path(&src, "jpg").ends_with("pic-converted-2.jpg"));
    }

    #[test]
    fn converts_between_formats() {
        let d = tmp("fmt");
        let src = d.join("pic.png");
        noisy(&src, 64, 48);
        for f in [ImgFormat::Jpg, ImgFormat::Webp, ImgFormat::Tiff, ImgFormat::Bmp] {
            let out = convert_image(&src, f, 85).unwrap();
            let back = image::open(&out).unwrap();
            assert_eq!((back.width(), back.height()), (64, 48), "{f:?}");
        }
    }

    #[test]
    fn transparency_goes_on_white_for_jpeg() {
        let d = tmp("alpha");
        let src = d.join("t.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 0])).save(&src).unwrap();
        let out = convert_image(&src, ImgFormat::Jpg, 90).unwrap();
        let p = image::open(out).unwrap().to_rgb8().get_pixel(4, 4).0;
        assert!(p[0] > 240 && p[1] > 240 && p[2] > 240);
    }

    #[test]
    fn reducer_fits_the_target() {
        let d = tmp("shrink");
        let src = d.join("big.png");
        noisy(&src, 900, 700);
        let (out, size) = shrink_to(&src, 60 * 1024).unwrap();
        assert!(size <= 60 * 1024, "size {size}");
        assert_eq!(std::fs::metadata(&out).unwrap().len(), size);
        assert!(size > 20 * 1024, "should not overshoot into a tiny file: {size}");
    }

    #[test]
    fn reducer_with_generous_target_keeps_quality_high() {
        let d = tmp("shrink2");
        let src = d.join("a.png");
        noisy(&src, 200, 200);
        let (_, size) = shrink_to(&src, 10 * 1024 * 1024).unwrap();
        assert!(size > 10 * 1024);
    }

    #[test]
    fn ffmpeg_arguments_per_format() {
        let a = ffmpeg_args(Path::new("/in.wav"), Path::new("/out.mp3"), MediaFormat::Mp3);
        assert!(a.contains(&"libmp3lame".to_string()) && a.contains(&"-vn".to_string()) && a.last().unwrap() == "/out.mp3");
        let v = ffmpeg_args(Path::new("/in.mov"), Path::new("/out.mp4"), MediaFormat::Mp4);
        assert!(v.contains(&"libx264".to_string()) && !v.contains(&"-vn".to_string()));
        assert!(MediaFormat::Gif.has_video() && !MediaFormat::Mp3.has_video());
    }

    #[test]
    fn progress_lines() {
        assert_eq!(parse_progress_line("out_time_us=2500000"), Some(2.5));
        assert_eq!(parse_progress_line("frame=12"), None);
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(human_size(512 * 1024), "512 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    }
}

#[cfg(test)]
mod ffmpeg_tests {
    use super::*;

    #[test]
    fn converts_a_wav_to_mp3_and_m4a_when_ffmpeg_is_installed() {
        if ffmpeg_path().is_none() {
            eprintln!("ffmpeg not installed: skipped");
            return;
        }
        let d = std::env::temp_dir().join(format!("nh-ff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let wav = d.join("tone.wav");
        let ok = Command::new("ffmpeg").args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=2"]).arg(&wav).status().unwrap();
        assert!(ok.success());
        let mut last = 0.0;
        let mp3 = run_ffmpeg(&wav, MediaFormat::Mp3, |p| last = p).unwrap();
        assert!(mp3.exists() && std::fs::metadata(&mp3).unwrap().len() > 1000);
        assert_eq!(last, 1.0);
        let m4a = run_ffmpeg(&wav, MediaFormat::M4a, |_| {}).unwrap();
        assert!(m4a.exists());
        let _ = std::fs::remove_dir_all(d);
    }
}

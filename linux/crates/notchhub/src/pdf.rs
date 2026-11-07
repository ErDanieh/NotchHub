//! PDF conversions with poppler (reading) and cairo (writing).
use gtk::cairo;
use notchhub_core::convert::{encode_jpeg, output_path};
use std::path::{Path, PathBuf};

fn open(src: &Path) -> Result<poppler::Document, String> {
    let uri = gtk::glib::filename_to_uri(src, None).map_err(|e| e.to_string())?;
    poppler::Document::from_file(&uri, None).map_err(|e| format!("Could not open the PDF ({e})"))
}

/// Renders one page to an RGB image at `dpi`.
fn render_page(page: &poppler::Page, dpi: f64) -> Result<image::RgbImage, String> {
    let (w, h) = page.size();
    let k = dpi / 72.0;
    let (pw, ph) = ((w * k).ceil() as i32, (h * k).ceil() as i32);
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, pw, ph).map_err(|e| e.to_string())?;
    {
        let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.paint();
        cr.scale(k, k);
        page.render(&cr);
    }
    let stride = surface.stride() as usize;
    let data = surface.take_data().map_err(|e| e.to_string())?;
    let mut img = image::RgbImage::new(pw as u32, ph as u32);
    for y in 0..ph as usize {
        for x in 0..pw as usize {
            let i = y * stride + x * 4;
            img.put_pixel(x as u32, y as u32, image::Rgb([data[i + 2], data[i + 1], data[i]]));
        }
    }
    Ok(img)
}

/// Every page to a picture (PNG or JPG at 150 dpi) in a new folder next to the PDF.
pub fn pdf_to_images(src: &Path, jpg: bool, progress: &mut dyn FnMut(f64)) -> Result<PathBuf, String> {
    let doc = open(src)?;
    let n = doc.n_pages();
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "pdf".into());
    let mut dir = src.parent().unwrap_or(Path::new(".")).join(format!("{stem}-pages"));
    let mut k = 2;
    while dir.exists() {
        dir = src.parent().unwrap_or(Path::new(".")).join(format!("{stem}-pages-{k}"));
        k += 1;
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for i in 0..n {
        let page = doc.page(i).ok_or("Could not read a page")?;
        let img = render_page(&page, 150.0)?;
        let name = format!("page-{:03}.{}", i + 1, if jpg { "jpg" } else { "png" });
        if jpg {
            std::fs::write(dir.join(name), encode_jpeg(&image::DynamicImage::ImageRgb8(img), 88)).map_err(|e| e.to_string())?;
        } else {
            img.save(dir.join(name)).map_err(|e| e.to_string())?;
        }
        progress((i + 1) as f64 / n as f64);
    }
    Ok(dir)
}

/// Draws JPEG data onto the current PDF page, scaled to (w, h) points.
fn draw_jpeg(cr: &cairo::Context, jpeg: Vec<u8>, px: (u32, u32), size: (f64, f64)) -> Result<(), String> {
    let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, px.0 as i32, px.1 as i32).map_err(|e| e.to_string())?;
    // cairo writes the JPEG into the PDF as it is (no recompression) when it is attached like this
    surface.set_mime_data("image/jpeg", jpeg).map_err(|e| e.to_string())?;
    cr.save().map_err(|e| e.to_string())?;
    cr.scale(size.0 / px.0 as f64, size.1 / px.1 as f64);
    cr.set_source_surface(&surface, 0.0, 0.0).map_err(|e| e.to_string())?;
    cr.paint().map_err(|e| e.to_string())?;
    cr.restore().map_err(|e| e.to_string())
}

/// Several pictures into one PDF, one per page, fitted to A4.
pub fn images_to_pdf(paths: &[PathBuf], progress: &mut dyn FnMut(f64)) -> Result<PathBuf, String> {
    let first = paths.first().ok_or("No pictures")?;
    let dst = output_path(first, "pdf");
    let surface = cairo::PdfSurface::new(595.0, 842.0, &dst).map_err(|e| e.to_string())?;
    let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
    for (i, p) in paths.iter().enumerate() {
        let img = image::open(p).map_err(|e| e.to_string())?;
        let (w, h) = (img.width(), img.height());
        let (pw, ph) = if w > h { (842.0, 595.0) } else { (595.0, 842.0) };
        let fit = (pw / w as f64).min(ph / h as f64);
        let (dw, dh) = (w as f64 * fit, h as f64 * fit);
        surface.set_size(pw, ph).map_err(|e| e.to_string())?;
        cr.save().map_err(|e| e.to_string())?;
        cr.translate((pw - dw) / 2.0, (ph - dh) / 2.0);
        draw_jpeg(&cr, encode_jpeg(&img, 90), (w, h), (dw, dh))?;
        cr.restore().map_err(|e| e.to_string())?;
        cr.show_page().map_err(|e| e.to_string())?;
        progress((i + 1) as f64 / paths.len() as f64);
    }
    drop(cr);
    surface.finish();
    Ok(dst)
}

/// Smaller PDF: every page becomes a JPEG. The text is no longer selectable.
pub fn shrink_pdf(src: &Path, dpi: f64, quality: u8, progress: &mut dyn FnMut(f64)) -> Result<(PathBuf, u64), String> {
    let doc = open(src)?;
    let n = doc.n_pages();
    let dst = output_path(src, "pdf");
    let first = doc.page(0).ok_or("The PDF has no pages")?.size();
    let surface = cairo::PdfSurface::new(first.0, first.1, &dst).map_err(|e| e.to_string())?;
    let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
    for i in 0..n {
        let page = doc.page(i).ok_or("Could not read a page")?;
        let (w, h) = page.size();
        let img = render_page(&page, dpi)?;
        let (pw, ph) = (img.width(), img.height());
        surface.set_size(w, h).map_err(|e| e.to_string())?;
        draw_jpeg(&cr, encode_jpeg(&image::DynamicImage::ImageRgb8(img), quality), (pw, ph), (w, h))?;
        cr.show_page().map_err(|e| e.to_string())?;
        progress((i + 1) as f64 / n as f64);
    }
    drop(cr);
    surface.finish();
    let size = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
    Ok((dst, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noisy(path: &Path, w: u32, h: u32, seed0: u32) {
        let mut seed = seed0;
        image::RgbImage::from_fn(w, h, |_, _| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            image::Rgb([(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8])
        })
        .save(path)
        .unwrap();
    }

    #[test]
    fn images_to_pdf_then_pages_back_then_smaller() {
        let d = std::env::temp_dir().join(format!("nh-pdf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let (a, b) = (d.join("a.png"), d.join("b.png"));
        noisy(&a, 800, 600, 1);
        noisy(&b, 600, 800, 2);
        let pdf_path = images_to_pdf(&[a.clone(), b.clone()], &mut |_| {}).unwrap();
        assert!(std::fs::metadata(&pdf_path).unwrap().len() > 1000);
        let doc = open(&pdf_path).unwrap();
        assert_eq!(doc.n_pages(), 2);

        let dir = pdf_to_images(&pdf_path, false, &mut |_| {}).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["page-001.png", "page-002.png"]);
        let first = image::open(dir.join("page-001.png")).unwrap();
        assert!(first.width() > 1000, "150 dpi A4 landscape is wider than 1000 px");

        let (small, size) = shrink_pdf(&pdf_path, 72.0, 40, &mut |_| {}).unwrap();
        let orig = std::fs::metadata(&pdf_path).unwrap().len();
        assert_eq!(open(&small).unwrap().n_pages(), 2);
        assert!(size < orig, "smaller pdf {size} should be below {orig}");
        let _ = std::fs::remove_dir_all(d);
    }
}

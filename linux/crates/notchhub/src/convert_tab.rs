//! Convert tab: images (with the weight reducer), PDF, audio and video.
use gtk::glib;
use gtk::prelude::*;
use notchhub_core::convert::{self, human_size, ImgFormat, Kind, MediaFormat};
use notchhub_core::settings::{Settings, Tab};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;

use crate::panel::Panel;
use crate::pdf;
use crate::ui;

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Img(ImgFormat),
    ImgPdf,
    Reduce,
    PdfImages(bool),
    PdfSmaller(u8),
    Media(MediaFormat),
}

fn targets(kind: Kind) -> Vec<(&'static str, Target)> {
    match kind {
        Kind::Image => vec![
            ("JPG", Target::Img(ImgFormat::Jpg)),
            ("PNG", Target::Img(ImgFormat::Png)),
            ("WebP", Target::Img(ImgFormat::Webp)),
            ("TIFF", Target::Img(ImgFormat::Tiff)),
            ("BMP", Target::Img(ImgFormat::Bmp)),
            ("PDF", Target::ImgPdf),
            ("Reduce weight…", Target::Reduce),
        ],
        Kind::Pdf => vec![("Pictures (PNG)", Target::PdfImages(false)), ("Pictures (JPG)", Target::PdfImages(true)), ("Smaller PDF", Target::PdfSmaller(0)), ("Much smaller PDF", Target::PdfSmaller(1))],
        Kind::Audio => vec![("MP3", Target::Media(MediaFormat::Mp3)), ("M4A", Target::Media(MediaFormat::M4a)), ("OGG", Target::Media(MediaFormat::Ogg)), ("FLAC", Target::Media(MediaFormat::Flac)), ("WAV", Target::Media(MediaFormat::Wav))],
        Kind::Video => vec![("MP4", Target::Media(MediaFormat::Mp4)), ("WebM", Target::Media(MediaFormat::Webm)), ("GIF", Target::Media(MediaFormat::Gif)), ("Audio MP3", Target::Media(MediaFormat::Mp3)), ("Audio M4A", Target::Media(MediaFormat::M4a))],
        Kind::Other => vec![],
    }
}

enum Msg {
    Progress(f64),
    Done(Result<(PathBuf, String), String>),
}

pub struct Convert {
    list: gtk::Box,
    empty: gtk::Label,
    merge: gtk::Button,
    files: RefCell<Vec<PathBuf>>,
    panel: Panel,
    settings: Rc<RefCell<Settings>>,
}

pub fn setup(panel: &Panel, settings: Rc<RefCell<Settings>>) -> Option<Rc<Convert>> {
    let page = ui::fresh_page(panel, Tab::Convert)?;
    let (head, actions) = ui::header("Drop files here: images, PDF, audio, video.");
    let merge = ui::flat_button("Images → one PDF");
    merge.set_visible(false);
    let choose = ui::flat_button("Choose files…");
    actions.append(&merge);
    actions.append(&choose);
    page.append(&head);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let empty = ui::muted("Nothing to convert yet.");
    empty.set_vexpand(true);
    page.append(&empty);
    page.append(&ui::scrolled(&list));
    let c = Rc::new(Convert { list, empty, merge: merge.clone(), files: RefCell::new(vec![]), panel: panel.clone(), settings });
    let me = c.clone();
    panel.connect_drop(Tab::Convert, move |paths| me.add(paths));
    let me = c.clone();
    choose.connect_clicked(move |b| {
        let dialog = gtk::FileDialog::builder().title("Choose files to convert").build();
        let me = me.clone();
        let win = b.root().and_downcast::<gtk::Window>();
        dialog.open_multiple(win.as_ref(), gtk::gio::Cancellable::NONE, move |r| {
            if let Ok(files) = r {
                let paths: Vec<PathBuf> = files.iter::<gtk::gio::File>().flatten().filter_map(|f| f.path()).collect();
                me.add(paths);
            }
        });
    });
    let me = c.clone();
    merge.connect_clicked(move |b| {
        let imgs: Vec<PathBuf> = me.files.borrow().iter().filter(|p| convert::kind_of(p) == Kind::Image).cloned().collect();
        me.run_job(b, "Images → PDF".into(), move |progress| {
            let dst = pdf::images_to_pdf(&imgs, &mut |p| progress(p))?;
            Ok((dst.clone(), format!("{} images → {}", imgs.len(), file_name(&dst))))
        });
    });
    Some(c)
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

impl Convert {
    fn add(self: &Rc<Self>, paths: Vec<PathBuf>) {
        for p in paths {
            if convert::kind_of(&p) == Kind::Other || self.files.borrow().contains(&p) {
                continue;
            }
            self.files.borrow_mut().push(p.clone());
            self.list.append(&self.row(&p));
        }
        self.empty.set_visible(self.files.borrow().is_empty());
        let images = self.files.borrow().iter().filter(|p| convert::kind_of(p) == Kind::Image).count();
        self.merge.set_visible(images >= 2);
    }

    fn row(self: &Rc<Self>, path: &Path) -> gtk::Box {
        let kind = convert::kind_of(path);
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row.add_css_class("row");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let size = std::fs::metadata(path).map(|m| human_size(m.len())).unwrap_or_default();
        let name = gtk::Label::new(Some(&format!("{}  ·  {}", file_name(path), size)));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        top.append(&name);
        let opts = targets(kind);
        let labels: Vec<&str> = opts.iter().map(|(l, _)| *l).collect();
        let dd = gtk::DropDown::from_strings(&labels);
        top.append(&dd);
        let kb = gtk::SpinButton::with_range(10.0, 50_000.0, 50.0);
        kb.set_value(500.0);
        kb.set_tooltip_text(Some("Wanted size in KB"));
        kb.set_visible(false);
        top.append(&kb);
        let go = gtk::Button::with_label("Convert");
        go.add_css_class("suggested-action");
        top.append(&go);
        let remove = ui::icon_button("window-close-symbolic", "Remove from the list");
        top.append(&remove);
        row.append(&top);

        let (k2, o2) = (kb.clone(), opts.clone());
        dd.connect_selected_notify(move |d| k2.set_visible(o2.get(d.selected() as usize).map_or(false, |(_, t)| *t == Target::Reduce)));

        let (me, p, row2) = (self.clone(), path.to_path_buf(), row.clone());
        remove.connect_clicked(move |_| {
            me.files.borrow_mut().retain(|x| x != &p);
            me.list.remove(&row2);
            me.empty.set_visible(me.files.borrow().is_empty());
        });
        let (me, p) = (self.clone(), path.to_path_buf());
        let row3 = row.clone();
        go.connect_clicked(move |b| {
            let Some((_, target)) = opts.get(dd.selected() as usize).copied() else { return };
            let kbv = kb.value() as u64;
            let p = p.clone();
            me.run_job(&row3.clone(), String::new(), move |progress| run_target(&p, target, kbv, progress));
            b.set_sensitive(false);
        });
        row
    }

    /// Runs the work on a thread, shows progress inside `anchor` (a row or a button inside one) and then the result.
    fn run_job(self: &Rc<Self>, anchor: &impl IsA<gtk::Widget>, _title: String, work: impl FnOnce(&mut dyn FnMut(f64)) -> Result<(PathBuf, String), String> + Send + 'static) {
        let row = anchor.ancestor(gtk::Box::static_type()).and_then(|w| w.downcast::<gtk::Box>().ok());
        let host = match anchor.dynamic_cast_ref::<gtk::Box>() {
            Some(b) if b.has_css_class("row") => Some(b.clone()),
            _ => {
                let mut w = anchor.clone().upcast::<gtk::Widget>();
                loop {
                    if w.has_css_class("row") {
                        break w.downcast::<gtk::Box>().ok();
                    }
                    match w.parent() {
                        Some(p) => w = p,
                        None => break row.clone(),
                    }
                }
            }
        };
        let bar = gtk::ProgressBar::new();
        let status = ui::muted("");
        status.set_xalign(0.0);
        status.set_visible(false);
        if let Some(h) = &host {
            h.append(&bar);
            h.append(&status);
        }
        let (tx, rx) = async_channel::unbounded::<Msg>();
        thread::spawn(move || {
            let txp = tx.clone();
            let mut progress = move |p: f64| {
                let _ = txp.send_blocking(Msg::Progress(p));
            };
            let res = work(&mut progress);
            let _ = tx.send_blocking(Msg::Done(res));
        });
        let me = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(m) = rx.recv().await {
                match m {
                    Msg::Progress(p) => {
                        if p <= 0.0 {
                            bar.pulse()
                        } else {
                            bar.set_fraction(p)
                        }
                    }
                    Msg::Done(res) => {
                        bar.set_visible(false);
                        status.set_visible(true);
                        match res {
                            Ok((dst, text)) => {
                                status.set_text(&format!("✓ {text}"));
                                status.add_css_class("running");
                                if let Some(h) = &host {
                                    let show = ui::flat_button("Show in folder");
                                    let d = dst.clone();
                                    show.connect_clicked(move |_| {
                                        let f = gtk::gio::File::for_path(&d);
                                        let target = if d.is_dir() { f } else { f.parent().unwrap_or(f) };
                                        let _ = gtk::gio::AppInfo::launch_default_for_uri(&target.uri(), None::<&gtk::gio::AppLaunchContext>);
                                    });
                                    h.append(&show);
                                }
                                if me.settings.borrow().alerts_conversion {
                                    me.panel.toast(&format!("✓ Converted {}", file_name(&dst)));
                                }
                            }
                            Err(e) => {
                                status.set_text(&format!("✗ {e}"));
                                status.add_css_class("danger");
                            }
                        }
                        break;
                    }
                }
            }
        });
    }
}

fn run_target(p: &Path, target: Target, kb: u64, progress: &mut dyn FnMut(f64)) -> Result<(PathBuf, String), String> {
    let before = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let summary = |dst: &Path| {
        let after = std::fs::metadata(dst).map(|m| m.len()).unwrap_or(0);
        format!("{} ({} → {})", file_name(dst), human_size(before), human_size(after))
    };
    progress(0.0);
    match target {
        Target::Img(f) => {
            let dst = convert::convert_image(p, f, 88)?;
            Ok((dst.clone(), summary(&dst)))
        }
        Target::Reduce => {
            let (dst, _) = convert::shrink_to(p, kb * 1024)?;
            Ok((dst.clone(), summary(&dst)))
        }
        Target::ImgPdf => {
            let dst = pdf::images_to_pdf(&[p.to_path_buf()], &mut |x| progress(x))?;
            Ok((dst.clone(), summary(&dst)))
        }
        Target::PdfImages(jpg) => {
            let dir = pdf::pdf_to_images(p, jpg, &mut |x| progress(x))?;
            let n = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
            Ok((dir.clone(), format!("{n} pages in {}", file_name(&dir))))
        }
        Target::PdfSmaller(level) => {
            let (dpi, q) = if level == 0 { (110.0, 60) } else { (72.0, 45) };
            let (dst, _) = pdf::shrink_pdf(p, dpi, q, &mut |x| progress(x))?;
            Ok((dst.clone(), summary(&dst)))
        }
        Target::Media(f) => {
            let dst = convert::run_ffmpeg(p, f, |x| progress(x))?;
            Ok((dst.clone(), summary(&dst)))
        }
    }
}

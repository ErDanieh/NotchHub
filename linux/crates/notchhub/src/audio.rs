//! Listens to the sound the computer plays (the monitor of the default output, through PulseAudio or
//! PipeWire's PulseAudio layer) so the halo can follow the beat. Nothing is recorded or saved.
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use notchhub_core::beat::BeatFilter;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioLevel {
    level: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    pub status: Arc<Mutex<String>>,
}

impl AudioLevel {
    pub fn new() -> Self {
        AudioLevel {
            level: Arc::new(AtomicU32::new(0)),
            stop: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new("Off".into())),
        }
    }

    pub fn level(&self) -> f64 {
        f32::from_bits(self.level.load(Ordering::Relaxed)) as f64
    }

    pub fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.stop.store(false, Ordering::SeqCst);
        let (level, stop, running, status) = (self.level.clone(), self.stop.clone(), self.running.clone(), self.status.clone());
        std::thread::spawn(move || {
            let spec = Spec { format: Format::S16le, channels: 1, rate: 22050 };
            match Simple::new(None, "NotchHub", Direction::Record, Some("@DEFAULT_MONITOR@"), "beat", &spec, None, None) {
                Err(e) => *status.lock().unwrap() = format!("Could not listen to the audio ({e})"),
                Ok(s) => {
                    *status.lock().unwrap() = "Listening".into();
                    let mut filter = BeatFilter::new(22050);
                    let mut buf = [0u8; 2048];
                    while !stop.load(Ordering::SeqCst) {
                        if s.read(&mut buf).is_err() {
                            *status.lock().unwrap() = "The audio stream stopped".into();
                            break;
                        }
                        let samples: Vec<i16> = buf.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
                        level.store(filter.process(&samples).to_bits(), Ordering::Relaxed);
                    }
                }
            }
            level.store(0f32.to_bits(), Ordering::Relaxed);
            running.store(false, Ordering::SeqCst);
        });
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

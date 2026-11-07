//! Pixel pets: the drawings and how they behave (walk, stop, sleep, peek out from under the pill).

pub struct Sprite {
    pub id: &'static str,
    pub name: &'static str,
    /// Walk frames, all the same size. '.' is empty.
    pub frames: Vec<Vec<&'static str>>,
    pub palette: Vec<(char, (u8, u8, u8))>,
    /// What the eyes ('E') turn into when blinking.
    pub body: char,
}

impl Sprite {
    pub fn cols(&self) -> usize {
        self.frames[0][0].chars().count()
    }
    pub fn rows(&self) -> usize {
        self.frames[0].len()
    }
    pub fn color(&self, c: char) -> Option<(u8, u8, u8)> {
        self.palette.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
    }
}

fn hex(v: u32) -> (u8, u8, u8) {
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn with_legs(body: &[&'static str], a: &[&'static str], b: &[&'static str]) -> Vec<Vec<&'static str>> {
    vec![body.iter().chain(a.iter()).copied().collect(), body.iter().chain(b.iter()).copied().collect()]
}

pub fn all() -> Vec<Sprite> {
    vec![
        Sprite {
            id: "clawd",
            name: "Clawd (orange critter)",
            frames: with_legs(
                &["..OOOOOOOOOO..", "..OOEOOOOEOO..", "OOOOEOOOOEOOOO", "OOOOOOOOOOOOOO", "..OOOOOOOOOO..", "..OOOOOOOOOO.."],
                &["..O.O....O.O..", "..O.O....O.O.."],
                &["...O.O..O.O...", "...O.O..O.O..."],
            ),
            palette: vec![('O', hex(0xD97757)), ('E', hex(0x2A1810))],
            body: 'O',
        },
        Sprite {
            id: "cat",
            name: "Cat",
            frames: with_legs(
                &[".O........O.", ".OO......OO.", ".OOOOOOOOOO.", ".OEOOOOOOEO.", ".OOOOPPOOOO.", "..OOOOOOOO.O", "..OOOOOOOO.O"],
                &["..OO....OO..", "..OO....OO.."],
                &["...OO..OO...", "...OO..OO..."],
            ),
            palette: vec![('O', hex(0xE8A25B)), ('E', hex(0x24180E)), ('P', hex(0xF08FA0))],
            body: 'O',
        },
        Sprite {
            id: "ghost",
            name: "Ghost",
            frames: with_legs(
                &["...WWWWWW...", "..WWWWWWWW..", ".WWWWWWWWWW.", ".WWEEWWEEWW.", ".WWEEWWEEWW.", ".WWWWWWWWWW.", ".WWWWWWWWWW.", ".WWWWWWWWWW."],
                &[".WW.WWWW.WW."],
                &["WW.WWWW.WW.W"],
            ),
            palette: vec![('W', hex(0xDDE6FF)), ('E', hex(0x1C2340))],
            body: 'W',
        },
        Sprite {
            id: "robot",
            name: "Robot",
            frames: with_legs(
                &[".....RR.....", "..GGGGGGGG..", "..GEEGGEEG..", "..GGGGGGGG..", ".DGGGGGGGGD.", "DDGGGGGGGGDD", ".DGGGGGGGGD.", "..GGGGGGGG.."],
                &["..DD....DD..", "..DD....DD.."],
                &["...DD..DD...", "...DD..DD..."],
            ),
            palette: vec![('G', hex(0x9AA5B1)), ('E', hex(0x5BE3FF)), ('R', hex(0xFF5C5C)), ('D', hex(0x4A5560))],
            body: 'G',
        },
        Sprite {
            id: "slime",
            name: "Slime",
            frames: vec![
                vec!["....SSSS....", "..SSSSSSSS..", ".SSSSSSSSSS.", ".SSESSSSESS.", ".SSSSSSSSSS.", "SSSSSSSSSSSS", "SSSSSSSSSSSS"],
                vec!["............", "....SSSS....", "..SSSSSSSS..", ".SSESSSSESS.", ".SSSSSSSSSS.", "SSSSSSSSSSSS", "SSSSSSSSSSSS"],
            ],
            palette: vec![('S', hex(0x5FD068)), ('E', hex(0x12301A))],
            body: 'S',
        },
        Sprite {
            id: "frog",
            name: "Frog",
            frames: with_legs(
                &[".GGG....GGG.", ".GEG....GEG.", ".GGGGGGGGGG.", "GGGGGGGGGGGG", "GLLGGGGGGLLG", ".GGGGGGGGGG."],
                &["GG........GG", "GG........GG"],
                &[".GG......GG.", ".GG......GG."],
            ),
            palette: vec![('G', hex(0x6CBF4B)), ('E', hex(0x17210F)), ('L', hex(0x2E5A22))],
            body: 'G',
        },
        Sprite {
            id: "invader",
            name: "Space invader",
            frames: with_legs(
                &["..I......I..", "...I....I...", "..IIIIIIII..", ".IIEIIIIEII.", "IIIIIIIIIIII", "I.IIIIIIII.I", "I.I......I.I"],
                &["...II..II..."],
                &["..II....II.."],
            ),
            palette: vec![('I', hex(0xC77DFF)), ('E', hex(0x24103A))],
            body: 'I',
        },
        Sprite {
            id: "penguin",
            name: "Penguin",
            frames: with_legs(
                &["...KKKK...", "..KKKKKK..", "..KEKKEK..", "..KKBBKK..", ".KKWWWWKK.", ".KKWWWWKK.", "KKKWWWWKKK", ".KKWWWWKK.", "..KKKKKK.."],
                &["..BB..BB.."],
                &["...BB.BB.."],
            ),
            palette: vec![('K', hex(0x2B3140)), ('W', hex(0xF2F4F8)), ('B', hex(0xFF9F1C)), ('E', hex(0xFFFFFF))],
            body: 'K',
        },
        Sprite {
            id: "bunny",
            name: "Bunny",
            frames: with_legs(
                &[".WW....WW.", ".WP....PW.", ".WP....PW.", ".WWWWWWWW.", ".WEWWWWEW.", ".WWWWNWWW.", "..WWWWWW..", ".WWWWWWWW.", ".WWWWWWWW."],
                &["..WW..WW.."],
                &["...WW.WW.."],
            ),
            palette: vec![('W', hex(0xF4EFEA)), ('P', hex(0xF6A9BB)), ('E', hex(0x2A1F24)), ('N', hex(0xF08FA0))],
            body: 'W',
        },
    ]
}

pub fn sprite(id: &str) -> Sprite {
    let mut v = all();
    let i = v.iter().position(|s| s.id == id).unwrap_or(0);
    v.swap_remove(i)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Walk,
    Idle,
    PeekIn,
    PeekHold,
    PeekOut,
    Sleep,
}

pub struct Pet {
    pub x: f64,
    pub facing: f64,
    pub step: usize,
    /// 0 hidden behind the pill, 1 fully out.
    pub peek: f64,
    pub mode: Mode,
    pub blink: bool,
    pub clock: f64,
    pub width: f64,
    pub notch_x: f64,
    target: f64,
    timer: f64,
    step_t: f64,
    blink_t: f64,
    wants_peek: bool,
    rng: u32,
}

impl Pet {
    pub fn new(seed: u32) -> Pet {
        Pet { x: 300.0, facing: 1.0, step: 0, peek: 0.0, mode: Mode::Idle, blink: false, clock: 0.0, width: 1400.0, notch_x: 700.0, target: 0.0, timer: 1.0, step_t: 0.0, blink_t: 3.0, wants_peek: false, rng: seed.max(1) }
    }

    fn rand(&mut self) -> f64 {
        // xorshift32
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f64 / u32::MAX as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.rand()
    }

    pub fn peeking(&self) -> bool {
        matches!(self.mode, Mode::PeekIn | Mode::PeekHold | Mode::PeekOut)
    }

    /// Advances the pet by `dt` seconds. `scale` is the size in pixels per sprite pixel / 2.
    pub fn update(&mut self, dt: f64, scale: f64) {
        self.clock += dt;
        self.blink_t -= dt;
        if self.blink_t <= 0.0 {
            self.blink = !self.blink;
            self.blink_t = if self.blink { 0.14 } else { self.range(2.0, 5.0) };
        }
        let speed = 16.0 * scale;
        match self.mode {
            Mode::Walk => {
                let d = self.target - self.x;
                if d.abs() < 2.0 {
                    if self.wants_peek {
                        self.mode = Mode::PeekIn;
                        self.wants_peek = false;
                    } else {
                        self.pick_next();
                    }
                } else {
                    self.facing = if d > 0.0 { 1.0 } else { -1.0 };
                    self.x += self.facing * d.abs().min(speed * dt);
                    self.step_t += dt;
                    if self.step_t > 0.2 {
                        self.step_t = 0.0;
                        self.step = 1 - self.step;
                    }
                }
            }
            Mode::Idle | Mode::Sleep => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.pick_next();
                }
            }
            Mode::PeekIn => {
                self.peek = (self.peek + dt / 0.7).min(1.0);
                if self.peek >= 1.0 {
                    self.mode = Mode::PeekHold;
                    self.timer = self.range(2.0, 4.0);
                }
            }
            Mode::PeekHold => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.mode = Mode::PeekOut;
                }
            }
            Mode::PeekOut => {
                self.peek = (self.peek - dt / 0.6).max(0.0);
                if self.peek <= 0.0 {
                    self.mode = Mode::Idle;
                    self.timer = 1.5;
                    self.step = 0;
                }
            }
        }
    }

    fn pick_next(&mut self) {
        self.step = 0;
        let r = self.rand();
        if r < 0.5 {
            let hi = (self.width - 40.0).max(41.0);
            self.target = self.range(40.0, hi);
            self.mode = Mode::Walk;
        } else if r < 0.68 {
            self.mode = Mode::Idle;
            self.timer = self.range(2.0, 5.0);
        } else if r < 0.9 {
            self.target = self.notch_x;
            self.wants_peek = true;
            self.mode = Mode::Walk;
        } else {
            self.mode = Mode::Sleep;
            self.timer = self.range(6.0, 10.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sprite_is_rectangular_with_a_full_palette() {
        for s in all() {
            let (cols, rows) = (s.cols(), s.rows());
            assert!(s.frames.len() >= 2, "{}", s.id);
            for f in &s.frames {
                assert_eq!(f.len(), rows, "{} frame height", s.id);
                for line in f {
                    assert_eq!(line.chars().count(), cols, "{} row {line:?}", s.id);
                    for c in line.chars().filter(|c| *c != '.') {
                        assert!(s.color(c).is_some(), "{} has no colour for {c}", s.id);
                    }
                }
            }
            assert!(s.color(s.body).is_some());
        }
        assert_eq!(all().len(), 9);
    }

    #[test]
    fn unknown_id_falls_back_to_the_first() {
        assert_eq!(sprite("nope").id, "clawd");
        assert_eq!(sprite("frog").id, "frog");
    }

    #[test]
    fn pet_stays_inside_the_screen_and_eventually_peeks_and_sleeps() {
        let mut p = Pet::new(7);
        p.width = 1000.0;
        p.notch_x = 500.0;
        let (mut peeked, mut slept) = (false, false);
        for _ in 0..(20 * 600) {
            p.update(0.05, 1.5);
            assert!(p.x >= 0.0 && p.x <= 1000.0, "x {}", p.x);
            assert!((0.0..=1.0).contains(&p.peek));
            peeked |= p.mode == Mode::PeekHold;
            slept |= p.mode == Mode::Sleep;
            if p.peeking() {
                assert!((p.x - 500.0).abs() < 2.1, "peeks only from the pill, x {}", p.x);
            }
        }
        assert!(peeked && slept);
    }

    #[test]
    fn blinking_toggles() {
        let mut p = Pet::new(3);
        let mut seen = false;
        for _ in 0..400 {
            p.update(0.05, 1.0);
            seen |= p.blink;
        }
        assert!(seen);
    }
}

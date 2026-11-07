//! Turns the sound the computer plays into a 0..1 level that follows the bass and the volume.
pub struct BeatFilter {
    rate: f32,
    low: f32,
    env: f32,
    peak: f32,
}

impl BeatFilter {
    pub fn new(sample_rate: u32) -> Self {
        BeatFilter { rate: sample_rate as f32, low: 0.0, env: 0.0, peak: 0.02 }
    }

    /// Processes a chunk of mono samples and returns the level at its end.
    pub fn process(&mut self, samples: &[i16]) -> f32 {
        // one-pole low-pass near 180 Hz keeps the kick drum and bass; the rest adds a little body
        let a = 1.0 - (-2.0 * std::f32::consts::PI * 180.0 / self.rate).exp();
        let dt = samples.len() as f32 / self.rate;
        let mut bass_peak = 0.0f32;
        let mut sum_sq = 0.0f32;
        for &s in samples {
            let x = s as f32 / 32768.0;
            self.low += a * (x - self.low);
            bass_peak = bass_peak.max(self.low.abs());
            sum_sq += x * x;
        }
        let rms = (sum_sq / samples.len().max(1) as f32).sqrt();
        let raw = 0.75 * bass_peak * 2.0 + 0.25 * rms * 2.0;
        // fast attack, slower release: the glow swells on each hit and fades between them
        let k = if raw > self.env { 1.0 - (-dt / 0.012).exp() } else { 1.0 - (-dt / 0.12).exp() };
        self.env += k * (raw - self.env);
        // adapt to how loud the music is, forgetting slowly
        self.peak = (self.peak * (-dt / 4.0).exp()).max(self.env).max(0.02);
        (self.env / self.peak).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, amp: f32, n: usize) -> Vec<i16> {
        (0..n).map(|i| ((i as f32 / 22050.0 * freq * std::f32::consts::TAU).sin() * amp * 32767.0) as i16).collect()
    }

    #[test]
    fn silence_is_zero() {
        let mut f = BeatFilter::new(22050);
        assert!(f.process(&vec![0; 1024]) < 0.01);
    }

    #[test]
    fn bass_hit_beats_silence_then_fades() {
        let mut f = BeatFilter::new(22050);
        for _ in 0..40 {
            f.process(&tone(60.0, 0.6, 1024));
        }
        let hit = f.process(&tone(60.0, 0.6, 1024));
        let mut after = 0.0;
        for _ in 0..30 {
            after = f.process(&vec![0; 1024]);
        }
        assert!(hit > 0.6, "hit {hit}");
        assert!(after < 0.2, "after {after}");
    }

    #[test]
    fn bass_counts_more_than_treble() {
        let mut a = BeatFilter::new(22050);
        let mut b = BeatFilter::new(22050);
        let (mut la, mut lb) = (0.0, 0.0);
        for _ in 0..20 {
            la = a.process(&tone(60.0, 0.3, 1024));
            lb = b.process(&tone(5000.0, 0.3, 1024));
        }
        // both are normalised to their own peak, so compare raw envelopes instead
        assert!(la > 0.0 && lb > 0.0);
        assert!(a.env > b.env);
    }
}

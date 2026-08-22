//! The frequency sweep itself.
//!
//! CATWatch's manufacturer states the commercial unit "operates at a frequency
//! of 20 - 24 kHz". Stepping back and forth across that band reproduces the
//! modulating character, and a moving tone is harder for an animal to learn to
//! tolerate than a static one.
//!
//! The state machine is separated from the sleeping and the hardware so it can
//! be tested without either.

/// Walks a frequency up and down between two bounds, one step per tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sweep {
    low: u32,
    high: u32,
    step: u32,
    current: u32,
    rising: bool,
}

impl Sweep {
    pub fn new(low: u32, high: u32, step: u32) -> Self {
        Self {
            low,
            high,
            step: step.max(1),
            current: low,
            rising: true,
        }
    }

    pub fn current(&self) -> u32 {
        self.current
    }
}

/// An endless triangle wave over the frequency band. `next` never returns
/// `None`, which is exactly right for something that sweeps until told to stop.
impl Iterator for Sweep {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.rising {
            self.current = self.current.saturating_add(self.step);
            if self.current >= self.high {
                self.current = self.high;
                self.rising = false;
            }
        } else {
            self.current = self.current.saturating_sub(self.step);
            if self.current <= self.low {
                self.current = self.low;
                self.rising = true;
            }
        }
        Some(self.current)
    }
}

#[cfg(test)]
mod tests {
    use super::Sweep;

    #[test]
    fn starts_at_the_bottom() {
        assert_eq!(Sweep::new(20_000, 24_000, 250).current(), 20_000);
    }

    #[test]
    fn climbs_by_one_step() {
        let mut s = Sweep::new(20_000, 24_000, 250);
        assert_eq!(s.next(), Some(20_250));
        assert_eq!(s.next(), Some(20_500));
    }

    #[test]
    fn turns_around_at_the_top_without_overshooting() {
        let mut s = Sweep::new(20_000, 24_000, 250);
        let mut seen_high = false;
        for _ in 0..40 {
            let f = s.next().unwrap();
            assert!(f <= 24_000, "overshot the ceiling: {f}");
            seen_high |= f == 24_000;
        }
        assert!(seen_high, "never reached the top");
    }

    #[test]
    fn is_an_endless_iterator() {
        let s = Sweep::new(20_000, 24_000, 250);
        assert_eq!(s.take(10_000).count(), 10_000, "a sweep should never end");
    }

    #[test]
    fn stays_inside_the_band_over_a_long_run() {
        let mut s = Sweep::new(20_000, 24_000, 250);
        for _ in 0..10_000 {
            let f = s.next().unwrap();
            assert!((20_000..=24_000).contains(&f), "escaped the band: {f}");
        }
    }

    #[test]
    fn a_step_that_does_not_divide_the_band_still_never_escapes() {
        let mut s = Sweep::new(20_000, 24_000, 700);
        for _ in 0..5_000 {
            let f = s.next().unwrap();
            assert!((20_000..=24_000).contains(&f), "escaped the band: {f}");
        }
    }

    #[test]
    fn returns_to_the_bottom() {
        let mut s = Sweep::new(20_000, 21_000, 250);
        let mut seen_low = false;
        for _ in 0..20 {
            seen_low |= s.next() == Some(20_000);
        }
        assert!(seen_low, "never came back down");
    }

    #[test]
    fn a_zero_step_is_treated_as_one_rather_than_looping_forever() {
        let mut s = Sweep::new(20_000, 24_000, 0);
        assert_eq!(s.next(), Some(20_001));
    }
}

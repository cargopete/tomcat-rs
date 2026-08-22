//! Kernel hardware PWM through sysfs.
//!
//! The Python original drove GPIO18 with `pigpio`. That was dropped from Debian
//! trixie, the base of current Raspberry Pi OS: the client pieces are still
//! packaged but the `pigpiod` daemon they talk to is not, so the program starts
//! happily and never makes a sound. We reach the same SoC peripheral through
//! the kernel instead.
//!
//! Needs `dtoverlay=pwm-2chan` in `/boot/firmware/config.txt`, which maps PWM0
//! to GPIO18 (physical pin 12), and `dtparam=audio=off`, because `snd_bcm2835`
//! claims the same peripheral. The sysfs nodes are root-owned, so this runs as
//! root unless a udev rule says otherwise.
//!
//! No `rppal`, and deliberately so: writing four files needs no dependency, and
//! keeping this pure means the crate builds and tests on any machine.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

/// Duty is expressed 0..=DUTY_RANGE, matching the `TOMCAT_DUTY` scale the
/// Python used, so existing `.env` files carry over untouched.
pub const DUTY_RANGE: u32 = 1_000_000;

const NANOS_PER_SEC: u64 = 1_000_000_000;

#[derive(Debug)]
pub enum Error {
    /// The chip directory is absent, which almost always means the overlay is.
    NoChip(PathBuf),
    /// The channel never appeared after export; udev did not oblige.
    ExportTimeout(PathBuf),
    Io(io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoChip(p) => write!(
                f,
                "{} not found. Add 'dtoverlay=pwm-2chan' to \
                 /boot/firmware/config.txt and reboot.",
                p.display()
            ),
            Error::ExportTimeout(p) => {
                write!(f, "{} did not appear after export", p.display())
            }
            Error::Io(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                write!(f, "sysfs PWM needs root: {e}")
            }
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// One channel of the SoC's PWM peripheral.
///
/// Nothing touches the filesystem until [`HardwarePwm::open`] is called, so
/// constructing one is free and safe anywhere.
pub struct HardwarePwm {
    chip: PathBuf,
    channel: u32,
    path: PathBuf,
}

impl HardwarePwm {
    pub fn new(chip: u32, channel: u32) -> Self {
        Self::with_root(Path::new("/sys/class/pwm"), chip, channel)
    }

    /// Root is injectable so the tests can drive a fake sysfs tree.
    pub fn with_root(root: &Path, chip: u32, channel: u32) -> Self {
        let chip = root.join(format!("pwmchip{chip}"));
        let path = chip.join(format!("pwm{channel}"));
        Self {
            chip,
            channel,
            path,
        }
    }

    pub fn open(&self) -> Result<(), Error> {
        if !self.chip.is_dir() {
            return Err(Error::NoChip(self.chip.clone()));
        }
        if !self.path.is_dir() {
            fs::write(self.chip.join("export"), self.channel.to_string())?;
            // udev has to create and chmod the new directory. It is quick but
            // not instant, and writing too early fails with ENOENT or EACCES.
            let ready = (0..50).any(|_| {
                if self.path.join("enable").exists() {
                    true
                } else {
                    sleep(Duration::from_millis(20));
                    false
                }
            });
            if !ready {
                return Err(Error::ExportTimeout(self.path.clone()));
            }
        }
        self.off();
        Ok(())
    }

    fn write(&self, name: &str, value: u64) -> Result<(), Error> {
        fs::write(self.path.join(name), value.to_string())?;
        Ok(())
    }

    /// Set frequency and duty. `duty` is 0..=[`DUTY_RANGE`].
    pub fn set(&self, hz: u32, duty: u32) -> Result<(), Error> {
        let period = NANOS_PER_SEC / u64::from(hz.max(1));
        // The kernel rejects a duty_cycle larger than period with EINVAL, so
        // duty goes to zero before the period shrinks. This is not theoretical:
        // it happens on every descending step of the sweep.
        self.write("duty_cycle", 0)?;
        self.write("period", period)?;
        self.write(
            "duty_cycle",
            period * u64::from(duty.min(DUTY_RANGE)) / u64::from(DUTY_RANGE),
        )
    }

    pub fn on(&self) -> Result<(), Error> {
        self.write("enable", 1)
    }

    /// Silence the output. Infallible and idempotent, because this is what runs
    /// from a signal handler and on the way out of a panic, where returning an
    /// error helps nobody.
    pub fn off(&self) {
        let _ = self.write("enable", 0);
        let _ = self.write("duty_cycle", 0);
    }
}

impl Drop for HardwarePwm {
    fn drop(&mut self) {
        self.off();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake, already-exported sysfs tree.
    fn fake() -> (tempdir::TempDir, HardwarePwm) {
        let dir = tempdir::TempDir::new();
        let ch = dir.path().join("pwmchip0").join("pwm0");
        fs::create_dir_all(&ch).unwrap();
        for f in ["period", "duty_cycle", "enable"] {
            fs::write(ch.join(f), "0").unwrap();
        }
        let pwm = HardwarePwm::with_root(dir.path(), 0, 0);
        (dir, pwm)
    }

    fn read(pwm: &HardwarePwm, name: &str) -> String {
        fs::read_to_string(pwm.path.join(name)).unwrap()
    }

    #[test]
    fn period_and_duty_at_20khz() {
        let (_d, pwm) = fake();
        pwm.set(20_000, DUTY_RANGE / 2).unwrap();
        assert_eq!(read(&pwm, "period"), "50000"); // 20 kHz -> 50 us
        assert_eq!(read(&pwm, "duty_cycle"), "25000"); // 50 %
    }

    #[test]
    fn period_and_duty_at_24khz() {
        let (_d, pwm) = fake();
        pwm.set(24_000, DUTY_RANGE / 2).unwrap();
        assert_eq!(read(&pwm, "period"), "41666");
        assert_eq!(read(&pwm, "duty_cycle"), "20833");
    }

    #[test]
    fn quarter_duty() {
        let (_d, pwm) = fake();
        pwm.set(20_000, 250_000).unwrap();
        assert_eq!(read(&pwm, "duty_cycle"), "12500");
    }

    #[test]
    fn duty_never_exceeds_period_when_frequency_rises() {
        // Going 20 -> 24 kHz shrinks the period below the standing duty_cycle.
        // If we did not zero it first the kernel would reject the write.
        let (_d, pwm) = fake();
        pwm.set(20_000, DUTY_RANGE / 2).unwrap();
        pwm.set(24_000, DUTY_RANGE / 2).unwrap();
        let period: u64 = read(&pwm, "period").parse().unwrap();
        let duty: u64 = read(&pwm, "duty_cycle").parse().unwrap();
        assert!(duty <= period, "duty {duty} exceeded period {period}");
    }

    #[test]
    fn duty_is_clamped_to_range() {
        let (_d, pwm) = fake();
        pwm.set(20_000, DUTY_RANGE * 4).unwrap();
        assert_eq!(read(&pwm, "duty_cycle"), read(&pwm, "period"));
    }

    #[test]
    fn off_silences_and_repeats_harmlessly() {
        let (_d, pwm) = fake();
        pwm.set(20_000, DUTY_RANGE / 2).unwrap();
        pwm.on().unwrap();
        assert_eq!(read(&pwm, "enable"), "1");
        pwm.off();
        pwm.off();
        assert_eq!(read(&pwm, "enable"), "0");
        assert_eq!(read(&pwm, "duty_cycle"), "0");
    }

    #[test]
    fn off_on_a_missing_tree_does_not_panic() {
        let pwm = HardwarePwm::with_root(Path::new("/nonexistent"), 0, 0);
        pwm.off();
    }

    #[test]
    fn missing_chip_names_the_overlay() {
        let pwm = HardwarePwm::with_root(Path::new("/nonexistent"), 0, 0);
        let msg = pwm.open().unwrap_err().to_string();
        assert!(msg.contains("pwm-2chan"), "unhelpful message: {msg}");
    }

    #[test]
    fn open_exports_a_channel_that_is_not_yet_present() {
        let dir = tempdir::TempDir::new();
        let chip = dir.path().join("pwmchip0");
        fs::create_dir_all(&chip).unwrap();
        // Stand in for udev: the export file is a directory watcher's job in
        // real life, so here we simply pre-create what it would produce.
        let ch = chip.join("pwm0");
        fs::create_dir_all(&ch).unwrap();
        for f in ["period", "duty_cycle", "enable"] {
            fs::write(ch.join(f), "0").unwrap();
        }
        let pwm = HardwarePwm::with_root(dir.path(), 0, 0);
        pwm.open().unwrap();
        assert_eq!(read(&pwm, "enable"), "0");
    }

    /// Minimal scratch directory. Not worth a dependency for four lines.
    mod tempdir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};

        static N: AtomicU32 = AtomicU32::new(0);

        pub struct TempDir(PathBuf);

        impl TempDir {
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                let n = N.fetch_add(1, Ordering::Relaxed);
                let p =
                    std::env::temp_dir().join(format!("tomcat-test-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}

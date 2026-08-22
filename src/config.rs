//! Runtime configuration, all of it overridable by `TOMCAT_*` environment
//! variables so the systemd unit and its `.env` carry the tuning and the
//! source stays untouched. Variable names and defaults match the Python
//! original exactly, so an existing `.env` works unchanged.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use crate::pwm::DUTY_RANGE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub pir_gpio: u8,
    pub pwm_chip: u32,
    pub pwm_channel: u32,
    pub f_low_hz: u32,
    pub f_high_hz: u32,
    pub sweep_step_hz: u32,
    pub sweep_tick: Duration,
    /// 0..=1_000_000. 500_000 is 50 %, the loudest a square wave gets.
    pub duty: u32,
    pub burst: Duration,
    pub cooldown: Duration,
    pub quiet_start_h: u32,
    pub quiet_end_h: u32,
    pub db_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pir_gpio: 17,
            pwm_chip: 0,
            pwm_channel: 0,
            f_low_hz: 20_000,
            f_high_hz: 24_000,
            sweep_step_hz: 250,
            sweep_tick: Duration::from_millis(20),
            duty: DUTY_RANGE / 2,
            burst: Duration::from_secs_f64(4.0),
            cooldown: Duration::from_secs_f64(8.0),
            quiet_start_h: 22,
            quiet_end_h: 7,
            db_path: default_db_path(),
        }
    }
}

fn default_db_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/home/pi".into());
    PathBuf::from(home).join("catdeter.sqlite3")
}

/// A malformed value is a mistake worth surfacing, not one to paper over with
/// the default: silently ignoring `TOMCAT_DUTY=half` would leave you tuning a
/// knob that is not connected to anything.
fn parsed<T: std::str::FromStr>(key: &str, fallback: T) -> Result<T, String> {
    match env::var(key) {
        Err(_) => Ok(fallback),
        Ok(raw) => raw
            .trim()
            .parse()
            .map_err(|_| format!("{key}: cannot parse {raw:?}")),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let d = Config::default();
        let cfg = Config {
            pir_gpio: parsed("TOMCAT_PIR_GPIO", d.pir_gpio)?,
            pwm_chip: parsed("TOMCAT_PWM_CHIP", d.pwm_chip)?,
            pwm_channel: parsed("TOMCAT_PWM_CHANNEL", d.pwm_channel)?,
            f_low_hz: parsed("TOMCAT_F_LOW_HZ", d.f_low_hz)?,
            f_high_hz: parsed("TOMCAT_F_HIGH_HZ", d.f_high_hz)?,
            sweep_step_hz: parsed("TOMCAT_SWEEP_STEP", d.sweep_step_hz)?,
            sweep_tick: Duration::from_secs_f64(parsed(
                "TOMCAT_SWEEP_TICK_S",
                d.sweep_tick.as_secs_f64(),
            )?),
            duty: parsed("TOMCAT_DUTY", d.duty)?,
            burst: Duration::from_secs_f64(parsed("TOMCAT_BURST_SECONDS", d.burst.as_secs_f64())?),
            cooldown: Duration::from_secs_f64(parsed(
                "TOMCAT_COOLDOWN_S",
                d.cooldown.as_secs_f64(),
            )?),
            quiet_start_h: parsed("TOMCAT_QUIET_START_H", d.quiet_start_h)?,
            quiet_end_h: parsed("TOMCAT_QUIET_END_H", d.quiet_end_h)?,
            db_path: env::var("TOMCAT_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or(d.db_path),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), String> {
        if self.f_low_hz == 0 || self.f_high_hz <= self.f_low_hz {
            return Err(format!(
                "TOMCAT_F_LOW_HZ/F_HIGH_HZ: need 0 < low < high, got {} and {}",
                self.f_low_hz, self.f_high_hz
            ));
        }
        if self.duty > DUTY_RANGE {
            return Err(format!("TOMCAT_DUTY: {} exceeds {DUTY_RANGE}", self.duty));
        }
        if self.quiet_start_h > 23 || self.quiet_end_h > 23 {
            return Err("TOMCAT_QUIET_START_H/END_H: hours must be 0-23".into());
        }
        Ok(())
    }
}

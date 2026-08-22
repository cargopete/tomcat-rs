//! The full deterrent: PIR-triggered bursts, quiet hours, cool-down, event log.
//!
//! Needs the PIR wired per the build guide, and `dtoverlay=pwm-2chan` for the
//! PWM. Runs as root, because the sysfs PWM nodes are root-owned.
//!
//!     sudo catdeter

use std::process::ExitCode;

#[cfg(target_os = "linux")]
use std::{
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    thread::sleep,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use chrono::{Local, Timelike};
#[cfg(target_os = "linux")]
use tomcat::{Config, HardwarePwm, Sweep, db::EventLog, quiet::in_quiet_hours};

/// How long the PIR needs to settle before its output means anything.
#[cfg(target_os = "linux")]
const PIR_WARMUP: Duration = Duration::from_secs(30);
#[cfg(target_os = "linux")]
const POLL: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn run() -> Result<(), String> {
    Err("catdeter needs Linux GPIO; build and run this on the Pi".into())
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    let cfg = Config::from_env()?;

    let pwm = HardwarePwm::new(cfg.pwm_chip, cfg.pwm_channel);
    pwm.open().map_err(|e| e.to_string())?;

    let pir = rppal::gpio::Gpio::new()
        .map_err(|e| format!("gpio: {e}"))?
        .get(cfg.pir_gpio)
        .map_err(|e| format!("gpio {}: {e}", cfg.pir_gpio))?
        .into_input_pulldown();

    let log = EventLog::open(&cfg.db_path).map_err(|e| format!("db: {e}"))?;

    // See tone_sweep.rs: a stop that leaves the peripheral running is a stop
    // that silently fails.
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(sig, Arc::clone(&stop))
            .map_err(|e| format!("signal handler: {e}"))?;
    }

    println!("warming up PIR for {} s ...", PIR_WARMUP.as_secs());
    let warm = Instant::now();
    while warm.elapsed() < PIR_WARMUP && !stop.load(Ordering::SeqCst) {
        sleep(POLL);
    }
    println!("armed.");

    let mut last_fire: Option<Instant> = None;

    while !stop.load(Ordering::SeqCst) {
        if pir.is_high() {
            let hour = Local::now().hour();
            if in_quiet_hours(hour, cfg.quiet_start_h, cfg.quiet_end_h) {
                let _ = log.log(false, "quiet-hours");
            } else if last_fire.is_some_and(|t| t.elapsed() < cfg.cooldown) {
                let _ = log.log(false, "cooldown");
            } else {
                let _ = log.log(true, "");
                burst(&pwm, &cfg, &stop).map_err(|e| e.to_string())?;
                last_fire = Some(Instant::now());
            }
        }
        sleep(POLL);
    }

    pwm.off();
    println!("stopped");
    Ok(())
}

/// One ultrasonic burst. Watches the stop flag so a `systemctl stop` mid-burst
/// takes effect immediately rather than after up to four more seconds of noise.
#[cfg(target_os = "linux")]
fn burst(pwm: &HardwarePwm, cfg: &Config, stop: &AtomicBool) -> Result<(), tomcat::pwm::Error> {
    let mut sweep = Sweep::new(cfg.f_low_hz, cfg.f_high_hz, cfg.sweep_step_hz);
    let end = Instant::now() + cfg.burst;
    pwm.set(sweep.current(), cfg.duty)?;
    pwm.on()?;
    for hz in &mut sweep {
        if Instant::now() >= end || stop.load(Ordering::SeqCst) {
            break;
        }
        pwm.set(hz, cfg.duty)?;
        sleep(cfg.sweep_tick);
    }
    pwm.off();
    Ok(())
}

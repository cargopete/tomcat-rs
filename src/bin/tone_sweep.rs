//! Sweep 20 -> 24 kHz on GPIO18 continuously.
//!
//! No sensor involved: this is the switched-by-hand configuration. Verify with
//! a phone spectrum analyser within ~50 cm of the horn, or with the lock-in rig
//! in the Python repo's `tools/`. You will not hear anything, which is the
//! entire point and also the most annoying thing about testing it.
//!
//!     sudo tone-sweep

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;

use tomcat::{Config, HardwarePwm, Sweep};

fn main() -> ExitCode {
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
            return ExitCode::FAILURE;
        }
    };

    let pwm = HardwarePwm::new(cfg.pwm_chip, cfg.pwm_channel);
    if let Err(e) = pwm.open() {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    // SIGTERM is what `systemctl stop` sends. Without handling it the process
    // dies and the PWM peripheral carries on driving the pin quite happily on
    // its own, so the horn keeps sounding after you have told it to stop. That
    // is a failure you cannot hear, which is the worst kind for this device.
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if let Err(e) = signal_hook::flag::register(sig, Arc::clone(&stop)) {
            eprintln!("could not install signal handler: {e}");
            return ExitCode::FAILURE;
        }
    }

    println!(
        "sweeping {}-{} kHz on GPIO18 at {:.0}% duty",
        cfg.f_low_hz / 1000,
        cfg.f_high_hz / 1000,
        f64::from(cfg.duty) / f64::from(tomcat::pwm::DUTY_RANGE) * 100.0
    );

    let mut sweep = Sweep::new(cfg.f_low_hz, cfg.f_high_hz, cfg.sweep_step_hz);
    if let Err(e) = pwm.set(sweep.current(), cfg.duty).and_then(|()| pwm.on()) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    for hz in &mut sweep {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if let Err(e) = pwm.set(hz, cfg.duty) {
            eprintln!("{e}");
            pwm.off();
            return ExitCode::FAILURE;
        }
        sleep(cfg.sweep_tick);
    }

    // Drop would do this too, but being explicit about the thing that matters
    // most is worth a line.
    pwm.off();
    println!("stopped");
    ExitCode::SUCCESS
}

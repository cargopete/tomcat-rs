//! TomCat: a CATWatch-style PIR-triggered ultrasonic cat deterrent.
//!
//! A Rust port of <https://github.com/cargopete/tomcat>. The hardware is a
//! Raspberry Pi driving GPIO18 through an IRLZ44N logic-level MOSFET into a
//! piezo horn on a 12 V rail. See that repository's `docs/BUILD.md` for the
//! circuit, and `docs/MEASUREMENTS.md` for what has actually been measured
//! coming out of it, which is less than you might hope.
//!
//! Two binaries:
//!
//! - `tone-sweep` sweeps continuously. No sensor needed, switched by hand.
//! - `catdeter` waits on a PIR, honours quiet hours and a cool-down, and logs
//!   every detection to SQLite.

pub mod config;
pub mod db;
pub mod pwm;
pub mod quiet;
pub mod sweep;

pub use config::Config;
pub use pwm::HardwarePwm;
pub use sweep::Sweep;

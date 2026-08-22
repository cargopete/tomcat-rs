# tomcat-rs

A Rust port of [cargopete/tomcat](https://github.com/cargopete/tomcat): a
CATWatch-style **PIR-triggered ultrasonic cat deterrent** for a Raspberry Pi.

A PIR sees motion, the Pi's hardware PWM sweeps 20–24 kHz on GPIO18, and an
**IRLZ44N** logic-level MOSFET switches a piezo horn against a 12 V rail.
Inaudible to humans, reportedly unwelcome to cats.

The electronics, wiring, bill of materials and dog-safety guidance all live in
the Python repository's [`docs/BUILD.md`](https://github.com/cargopete/tomcat/blob/main/docs/BUILD.md).
This repository is the software only.

## Two binaries

| Binary | What it does |
|---|---|
| `tone-sweep` | Sweeps continuously. No sensor. Switched by hand. |
| `catdeter` | PIR-triggered bursts, quiet hours, cool-down, SQLite event log. |

`tone-sweep` is the one to start with: it needs nothing beyond the drive
circuit, and it is what the acoustic measurements in the Python repo were taken
with.

## Why sysfs and not pigpio

`pigpio` was **dropped from Debian trixie**, the base of current Raspberry Pi
OS. The client pieces are still packaged but the `pigpiod` daemon they talk to
is not, so a program built on it starts cleanly, connects to nothing, and never
makes a sound. There is no error worth the name.

So `src/pwm.rs` drives the SoC's PWM peripheral through the kernel's
`/sys/class/pwm` interface directly. No daemon, no `/dev/mem`, and no
dependency: it writes four files. That also means the crate compiles and its
tests run on any machine, not just a Pi, which is why `cargo test` is worth
running on your laptop.

`rppal` is used for the PIR input only, and is gated to `target_os = "linux"`.

## Prerequisites on the Pi

```bash
# Enable hardware PWM0 on GPIO18, and stop the onboard audio driver claiming
# the same peripheral. Both take effect on reboot.
sudo sh -c 'printf "\ndtoverlay=pwm-2chan\n" >> /boot/firmware/config.txt'
sudo sed -i 's/^dtparam=audio=on/dtparam=audio=off/' /boot/firmware/config.txt
sudo reboot
```

After the reboot `ls /sys/class/pwm/` must show `pwmchip0`. If it does not, the
overlay did not load and nothing below will work.

## Build

On the Pi:

```bash
cargo build --release
sudo install -m755 target/release/tone-sweep target/release/catdeter /usr/local/bin/
```

Cross-compiling from a workstation:

```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

You will need an `aarch64-linux-gnu-gcc` linker. `rusqlite` is vendored with
the `bundled` feature so there is no SQLite to install on the target.

## Run

Continuous, switched by hand:

```bash
sudo install -m644 systemd/tomcat-tone.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl start tomcat-tone     # horn on
sudo systemctl stop  tomcat-tone     # horn off
```

The unit is not enabled at boot on purpose: it is a switch, not a daemon. Add
`sudo systemctl enable tomcat-tone` if you want it back after a power cut.

Sensor-driven, once the PIR is wired:

```bash
sudo install -m644 systemd/catdeter.service /etc/systemd/system/
sudo systemctl enable --now catdeter
journalctl -u catdeter -f
```

Both need root, because the sysfs PWM nodes are root-owned. A udev rule
granting the `gpio` group write access would let them drop privileges; that has
not been written yet.

## Tuning

Every setting reads from a `TOMCAT_*` environment variable and falls back to
the literal in the source, so the systemd unit and its `.env` carry the tuning.
The names and defaults match the Python original exactly, so an existing `.env`
works unchanged. See [`.env.example`](.env.example).

A malformed value is a hard error rather than a silent fallback: quietly
ignoring `TOMCAT_DUTY=half` would leave you turning a knob connected to
nothing.

## Two details worth knowing

**`duty_cycle` may never exceed `period`.** The kernel rejects it with
`EINVAL`, and that is not a theoretical concern: it happens on every descending
step of the sweep. `HardwarePwm::set` zeroes the duty before shrinking the
period, and there is a test pinning it.

**`SIGTERM` must silence the output.** It is what `systemctl stop` sends, and
without handling it the process dies while the PWM peripheral carries on
driving the pin. The horn keeps sounding after you have told it to stop, and
since you cannot hear it, nothing tells you. The Python original shipped with
exactly that bug. Both binaries here handle it, and `HardwarePwm` silences on
`Drop` as well.

## Status

Ported and tested, **not yet run on hardware**. 27 tests pass on the host,
`clippy -D warnings` is clean, and the PWM logic is covered against a fake
sysfs tree. What has not happened is a single burst out of a real horn driven
by this code rather than by the Python.

What the Python build did establish, and which carries over unchanged because
the hardware is identical: the horn measurably radiates across **19–22 kHz**
(blind trials, p = 0.00002; see
[`docs/MEASUREMENTS.md`](https://github.com/cargopete/tomcat/blob/main/docs/MEASUREMENTS.md)).
What remains unestablished is how loud that is, and whether any cat has ever
cared.

## Licence

MIT.

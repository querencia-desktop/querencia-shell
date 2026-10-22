//! Querencia Lautstärke: the audio system (PulseAudio protocol, also
//! PipeWire through `pipewire-pulse`) for the panel's volume applet and the
//! mixer window `querencia-lautstaerke`.
//!
//! - [`model`]: plain data and rules (percent, levels, tooltip), tested
//!   without GTK or a server;
//! - [`pulse`]: libpulse, opened at run time, events instead of polling;
//! - [`service`]: one shared connection per process on the GTK main thread;
//! - [`icons`]: the own SVG icons.

pub mod i18n;
pub mod icons;
pub mod model;
pub mod pulse;
pub mod service;

/// Desktop ID and application ID of the mixer.
pub const MIXER_APP_ID: &str = "org.querencia.Lautstaerke";
/// Program name of the mixer.
pub const MIXER_PROGRAM: &str = "querencia-lautstaerke";

/// Writes one log line to stderr in a single `write` call, so the lines of
/// the panel and of the mixer it started (same stderr) never interleave.
pub fn log_line(line: std::fmt::Arguments<'_>) {
    use std::io::Write;
    let mut text = line.to_string();
    text.push('\n');
    let _ = std::io::stderr().lock().write_all(text.as_bytes());
}

/// `eprintln!` through [`log_line`].
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::log_line(format_args!($($arg)*))
    };
}

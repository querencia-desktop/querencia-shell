//! Querencia Netzwerk: the state of the network for the panel's network
//! applet, after the GNOME 2 NetworkManager applet.
//!
//! - [`model`]: plain data and rules (icon, tooltip, texts), tested without
//!   GTK or D-Bus;
//! - [`nm`]: NetworkManager over D-Bus, can connect and switch WLAN;
//!   [`agent`]: its secret agent (WLAN keys);
//! - [`kernel`], [`fallback`]: read only, without NetworkManager (kernel,
//!   systemd-networkd, systemd-resolved);
//! - [`service`]: one shared service per process on the GTK main thread,
//!   switching between the two when NetworkManager comes or goes;
//! - [`icons`]: the own SVG icons.

pub mod agent;
pub mod fallback;
pub mod i18n;
pub mod icons;
pub mod kernel;
pub mod model;
pub mod nm;
pub mod props;
pub mod service;

/// Writes one log line to stderr in a single `write` call (the panel and
/// the programs it starts share stderr).
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

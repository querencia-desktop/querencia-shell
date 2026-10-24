//! Querencia Dateiarbeit: the file work Querencia Desktop and the file
//! manager Querencia Dateien share, without a display (GIO and GLib only).
//!
//! - [`uri`]: URI lists and the clipboard formats of file managers;
//! - [`namen`]: names for copies and new objects, checks of file names,
//!   the visible name of starters;
//! - [`typ`]: types by name, icons for files, sizes;
//! - [`laufwerke`]: volumes (sorting into four types, mounting, unmounting,
//!   ejecting, programs that still use a volume);
//! - [`oeffnen`]: what opening a file means (program, starter, folder);
//! - [`papierkorb`]: trash count without polling, restoring, emptying;
//! - [`ueberwachung`]: folder monitors with debounce, waiting for a folder
//!   that does not exist yet;
//! - [`auftraege`]: copy, move, link, trash, delete, restore as jobs with
//!   progress, cancel and conflict questions;
//! - [`netz`]: network places (one entry per device) and the SFTP host key
//!   (MIT, from the Strata fork Querencia Files).
//!
//! Rules every operation keeps (docs/dateiarbeit.md): nothing is overwritten
//! without an answer; a replacement is built completely first and then put
//! in place atomically; a move across file systems deletes the original only
//! after the whole copy; links are never followed.

pub mod auftraege;
pub mod i18n;
pub mod laufwerke;
pub mod namen;
pub mod netz;
pub mod oeffnen;
pub mod papierkorb;
pub mod typ;
pub mod ueberwachung;
pub mod uri;

mod atomar;

/// Writes one log line to stderr in a single `write` call (the programs
/// using the crate and the programs they start share stderr).
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

//! The shared thumbnail cache of the freedesktop.org Thumbnail Managing
//! Standard (0.9): `$XDG_CACHE_HOME/thumbnails/{normal,large}/<md5>.png`,
//! where `<md5>` is the MD5 of the file's URI in lower-case hex; every PNG
//! carries `Thumb::URI` and `Thumb::MTime` and is only valid while both
//! still match the file. Failed attempts go to
//! `thumbnails/fail/querencia-dateien/` as a PNG with the same two tags,
//! so a file that cannot be thumbnailed is not tried again until it
//! changes. Querencia Files reads and writes the same `large` folder
//! (M9, B §4).
//!
//! Reading treats the cache as untrusted: no symlinks, FIFOs or other
//! special files, a size bound before reading and a dimension bound before
//! decoding. The checks of [`lies_begrenzt`], [`png_masse`] and
//! [`thumb_tags`] follow `src/ui/thumbnail_cache.rs` of Querencia Files
//! (Strata fork, MIT, `crates/querencia-dateiarbeit/LICENSES/MIT-Strata.txt`,
//! Copyright (c) 2026 LGSE Ltd.), rewritten without `rustix`.

use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gtk4::glib;

/// The two size buckets of the standard: the long edge of a thumbnail is
/// at most 128 (normal) or 256 px (large).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Eimer {
    Normal,
    Gross,
}

impl Eimer {
    pub fn kante(self) -> u32 {
        match self {
            Self::Normal => 128,
            Self::Gross => 256,
        }
    }

    pub fn ordner(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Gross => "large",
        }
    }

    /// The bucket a picture of `px` (long edge) is made from.
    pub fn fuer(px: u32) -> Self {
        if px <= 128 { Self::Normal } else { Self::Gross }
    }
}

/// Our folder below `fail/` (the standard: one per program).
pub const FEHL_ORDNER: &str = "querencia-dateien";

/// A cache PNG beyond this is a miss without being read (a 256 px PNG with
/// alpha is below 300 KB).
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Thumbnails of other programs may be larger than 256 px (some write
/// 512 px into `large`); anything beyond is rejected before decoding.
const MAX_KANTE: u32 = 1024;

/// The cache root (`$XDG_CACHE_HOME/thumbnails`, as GLib finds it).
pub fn wurzel() -> PathBuf {
    glib::user_cache_dir().join("thumbnails")
}

/// The file name of a URI's thumbnail: MD5 of the URI, lower-case hex.
pub fn dateiname(uri: &str) -> String {
    let hex = glib::compute_checksum_for_string(glib::ChecksumType::Md5, uri)
        .map(|s| s.to_string())
        .unwrap_or_default();
    format!("{hex}.png")
}

/// Where the thumbnail of `uri` in `eimer` is.
pub fn pfad(wurzel: &Path, eimer: Eimer, uri: &str) -> PathBuf {
    wurzel.join(eimer.ordner()).join(dateiname(uri))
}

/// Where a failed attempt for `uri` is noted.
pub fn fehl_pfad(wurzel: &Path, uri: &str) -> PathBuf {
    wurzel.join("fail").join(FEHL_ORDNER).join(dateiname(uri))
}

/// Whether `pfad` lies in the cache itself (the standard: never make
/// thumbnails of thumbnails).
pub fn im_cache(wurzel: &Path, pfad: &Path) -> bool {
    pfad.starts_with(wurzel)
}

/// A valid cache PNG of `uri` with modification time `mtime` (seconds), as
/// bytes; `None` when missing, stale, foreign or suspicious.
pub fn lesen(datei: &Path, uri: &str, mtime: i64) -> Option<Vec<u8>> {
    let bytes = lies_begrenzt(datei)?;
    let (b, h) = png_masse(&bytes)?;
    if b == 0 || h == 0 || b > MAX_KANTE || h > MAX_KANTE {
        return None;
    }
    let (u, m) = thumb_tags(&bytes);
    (u.as_deref() == Some(uri) && m.as_deref().and_then(mtime_wert) == Some(mtime)).then_some(bytes)
}

/// `Thumb::MTime` as written by GNOME (seconds) or with a fraction.
fn mtime_wert(text: &str) -> Option<i64> {
    let ganz = text.split('.').next()?;
    ganz.trim().parse().ok()
}

/// Whether a failure for `uri` at `mtime` is noted.
pub fn fehlgeschlagen(wurzel: &Path, uri: &str, mtime: i64) -> bool {
    lesen(&fehl_pfad(wurzel, uri), uri, mtime).is_some()
}

// O_NOFOLLOW differs between architectures; O_NONBLOCK keeps a FIFO from
// blocking the open.
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
const O_NOFOLLOW: i32 = 0o100000;
#[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
const O_NOFOLLOW: i32 = 0o400000;
const O_NONBLOCK: i32 = 0o4000;

/// Reads a regular file of at most [`MAX_BYTES`] without following a
/// symlink.
fn lies_begrenzt(pfad: &Path) -> Option<Vec<u8>> {
    lies_regulaer(pfad, MAX_BYTES)
}

/// Reads `pfad` only if it is a regular file of at most `max` bytes:
/// opened with `O_NOFOLLOW` (a symlink is an error, never followed) and
/// `O_NONBLOCK` (a FIFO does not block the open), then checked with fstat
/// on the open file, so it cannot be swapped in between. For the cache and
/// for what a thumbnailer left in its job folder.
pub fn lies_regulaer(pfad: &Path, max: u64) -> Option<Vec<u8>> {
    let datei = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW | O_NONBLOCK)
        .open(pfad)
        .ok()?;
    let meta = datei.metadata().ok()?;
    if !meta.is_file() || meta.len() > max {
        return None;
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    datei.take(max + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= max).then_some(bytes)
}

const SIGNATUR: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The chunks of a PNG: (type, data, start of the chunk, end after the CRC).
/// Stops at the first malformed chunk; never panics.
fn chunks(png: &[u8]) -> Vec<([u8; 4], &[u8], usize, usize)> {
    let mut out = Vec::new();
    if png.get(..SIGNATUR.len()) != Some(SIGNATUR) {
        return out;
    }
    let mut pos = SIGNATUR.len();
    while let Some(kopf) = png.get(pos..pos + 8) {
        let laenge = u32::from_be_bytes([kopf[0], kopf[1], kopf[2], kopf[3]]) as usize;
        let art = [kopf[4], kopf[5], kopf[6], kopf[7]];
        let Some(ende) = pos
            .checked_add(8)
            .and_then(|p| p.checked_add(laenge))
            .and_then(|p| p.checked_add(4))
        else {
            break;
        };
        let Some(daten) = png.get(pos + 8..pos + 8 + laenge) else {
            break;
        };
        if ende > png.len() {
            break;
        }
        out.push((art, daten, pos, ende));
        pos = ende;
        if &art == b"IEND" {
            break;
        }
    }
    out
}

/// Width and height from the IHDR chunk.
pub fn png_masse(png: &[u8]) -> Option<(u32, u32)> {
    let (art, daten, _, _) = *chunks(png).first()?;
    if &art != b"IHDR" || daten.len() < 13 {
        return None;
    }
    let b = u32::from_be_bytes(daten[0..4].try_into().ok()?);
    let h = u32::from_be_bytes(daten[4..8].try_into().ok()?);
    Some((b, h))
}

/// `Thumb::URI` and `Thumb::MTime` of a PNG.
pub fn thumb_tags(png: &[u8]) -> (Option<String>, Option<String>) {
    let mut uri = None;
    let mut mtime = None;
    for (art, daten, _, _) in chunks(png) {
        if &art != b"tEXt" {
            continue;
        }
        let Some(null) = daten.iter().position(|b| *b == 0) else {
            continue;
        };
        let (wort, wert) = (&daten[..null], &daten[null + 1..]);
        let wert = || std::str::from_utf8(wert).ok().map(str::to_owned);
        match wort {
            b"Thumb::URI" => uri = wert(),
            b"Thumb::MTime" => mtime = wert(),
            _ => {}
        }
    }
    (uri, mtime)
}

/// CRC-32 (ISO 3309) of PNG chunks.
fn crc32(teile: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for teil in teile {
        for &b in *teil {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    0xedb8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn text_chunk(wort: &str, wert: &str) -> Vec<u8> {
    let mut daten = Vec::with_capacity(wort.len() + 1 + wert.len());
    daten.extend_from_slice(wort.as_bytes());
    daten.push(0);
    daten.extend_from_slice(wert.as_bytes());
    let mut out = Vec::with_capacity(daten.len() + 12);
    out.extend_from_slice(&(daten.len() as u32).to_be_bytes());
    out.extend_from_slice(b"tEXt");
    out.extend_from_slice(&daten);
    out.extend_from_slice(&crc32(&[b"tEXt", &daten]).to_be_bytes());
    out
}

/// `png` with the text chunks `tags` right after IHDR; text chunks of the
/// same keywords already in the file are dropped. `None` if `png` is no
/// PNG.
pub fn mit_tags(png: &[u8], tags: &[(&str, &str)]) -> Option<Vec<u8>> {
    let alle = chunks(png);
    let (art, _, _, ihdr_ende) = *alle.first()?;
    if &art != b"IHDR" || alle.last().map(|c| &c.0) != Some(b"IEND") {
        return None;
    }
    let mut out = Vec::with_capacity(png.len() + 256);
    out.extend_from_slice(&png[..ihdr_ende]);
    for (wort, wert) in tags {
        out.extend_from_slice(&text_chunk(wort, wert));
    }
    for (art, daten, start, ende) in alle.iter().skip(1) {
        let ersetzt = art == b"tEXt"
            && tags.iter().any(|(wort, _)| {
                daten.starts_with(wort.as_bytes()) && daten.get(wort.len()) == Some(&0)
            });
        if !ersetzt {
            out.extend_from_slice(&png[*start..*ende]);
        }
    }
    Some(out)
}

/// The tags the standard asks for (URI, MTime) and recommends (Size,
/// Mimetype, Software).
pub fn tags<'a>(
    uri: &'a str,
    mtime: &'a str,
    groesse: &'a str,
    typ: &'a str,
) -> [(&'static str, &'a str); 5] {
    [
        ("Thumb::URI", uri),
        ("Thumb::MTime", mtime),
        ("Thumb::Size", groesse),
        ("Thumb::Mimetype", typ),
        (
            "Software",
            concat!("Querencia Dateien ", env!("CARGO_PKG_VERSION")),
        ),
    ]
}

static ZAEHLER: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `ziel` as the standard asks: folder 0700, file 0600,
/// a temporary file in the same folder renamed over the old one (readers
/// never see half a PNG).
pub fn schreiben(ziel: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let ordner = ziel
        .parent()
        .ok_or_else(|| std::io::Error::other("Cache-Pfad ohne Ordner"))?;
    std::fs::create_dir_all(ordner)?;
    std::fs::set_permissions(ordner, std::fs::Permissions::from_mode(0o700))?;
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let name = ziel
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = ordner.join(format!(".{name}.querencia-{}-{n}", std::process::id()));
    let ergebnis = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_data().ok();
        std::fs::rename(&tmp, ziel)
    })();
    if ergebnis.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    ergebnis
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1×1 PNG without text chunks (as GTK writes one).
    fn klein() -> Vec<u8> {
        let mut png = SIGNATUR.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        for (art, daten) in [
            (b"IHDR", ihdr),
            // zlib, stored block: filter byte + one RGBA pixel
            (
                b"IDAT",
                vec![
                    0x78, 0x01, 0x01, 0x05, 0x00, 0xfa, 0xff, 0, 0, 0, 0, 0, 0x00, 0x05, 0x00, 0x01,
                ],
            ),
            (b"IEND", Vec::new()),
        ] {
            png.extend_from_slice(&(daten.len() as u32).to_be_bytes());
            png.extend_from_slice(art);
            png.extend_from_slice(&daten);
            png.extend_from_slice(&crc32(&[art, &daten]).to_be_bytes());
        }
        png
    }

    #[test]
    fn file_name_is_md5_of_the_uri() {
        // the example of the standard (§ "Thumbnail Saving")
        assert_eq!(
            dateiname("file:///home/jens/photos/me.png"),
            "c6ee772d9e49320e97ec29a7eb5b1697.png"
        );
        let w = Path::new("/c/thumbnails");
        assert_eq!(
            pfad(w, Eimer::Normal, "file:///a"),
            Path::new("/c/thumbnails/normal").join(dateiname("file:///a"))
        );
        assert!(
            fehl_pfad(w, "file:///a")
                .to_string_lossy()
                .contains("/fail/querencia-dateien/")
        );
        assert_eq!(Eimer::fuer(64), Eimer::Normal);
        assert_eq!(Eimer::fuer(128), Eimer::Normal);
        assert_eq!(Eimer::fuer(129), Eimer::Gross);
        assert!(im_cache(w, Path::new("/c/thumbnails/normal/x.png")));
        assert!(!im_cache(w, Path::new("/c/bilder/x.png")));
    }

    #[test]
    fn crc_matches_the_png_specification() {
        assert_eq!(crc32(&[b"IEND"]), 0xae42_6082);
        assert_eq!(crc32(&[b"123456789"]), 0xcbf4_3926);
    }

    #[test]
    fn tags_are_written_after_ihdr_and_read_back() {
        let png = klein();
        assert_eq!(png_masse(&png), Some((1, 1)));
        assert_eq!(thumb_tags(&png), (None, None));
        let t = tags("file:///x%20y.png", "1700000000", "42", "image/png");
        let mit = mit_tags(&png, &t).unwrap();
        assert_eq!(png_masse(&mit), Some((1, 1)));
        assert_eq!(
            thumb_tags(&mit),
            (
                Some("file:///x%20y.png".to_owned()),
                Some("1700000000".to_owned())
            )
        );
        // writing again replaces the tags instead of adding a second set
        let t2 = tags("file:///x%20y.png", "1800000000", "42", "image/png");
        let zwei = mit_tags(&mit, &t2).unwrap();
        assert_eq!(thumb_tags(&zwei).1.as_deref(), Some("1800000000"));
        assert_eq!(zwei.len(), mit.len());
        // every chunk still has a valid CRC
        for (art, daten, start, ende) in chunks(&zwei) {
            let crc = u32::from_be_bytes(zwei[ende - 4..ende].try_into().unwrap());
            assert_eq!(crc, crc32(&[&art, daten]), "CRC von {art:?} bei {start}");
        }
        assert!(mit_tags(b"kein png", &t).is_none());
    }

    #[test]
    fn hostile_pngs_never_panic() {
        let png = klein();
        for n in 0..png.len() {
            let _ = chunks(&png[..n]);
            let _ = thumb_tags(&png[..n]);
            let _ = png_masse(&png[..n]);
        }
        let mut riesig = png.clone();
        riesig[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(png_masse(&riesig), None);
    }

    #[test]
    fn reading_checks_uri_mtime_and_file_kind() {
        let d = std::env::temp_dir().join(format!("qd-miniaturen-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let ziel = d.join("normal").join(dateiname("file:///a.png"));
        let png = mit_tags(&klein(), &tags("file:///a.png", "100", "1", "image/png")).unwrap();
        schreiben(&ziel, &png).unwrap();
        let modus = std::fs::metadata(&ziel).unwrap().permissions().mode() & 0o777;
        assert_eq!(modus, 0o600);
        let modus = std::fs::metadata(d.join("normal"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(modus, 0o700);
        assert!(lesen(&ziel, "file:///a.png", 100).is_some());
        assert!(lesen(&ziel, "file:///a.png", 101).is_none(), "veraltet");
        assert!(lesen(&ziel, "file:///b.png", 100).is_none(), "fremd");
        // a symlink in the cache is a miss
        let link = d.join("normal").join("link.png");
        std::os::unix::fs::symlink(&ziel, &link).unwrap();
        assert!(lesen(&link, "file:///a.png", 100).is_none());
        // a failure note is a PNG with the same tags
        let wurzel = d.clone();
        schreiben(&fehl_pfad(&wurzel, "file:///a.png"), &png).unwrap();
        assert!(fehlgeschlagen(&wurzel, "file:///a.png", 100));
        assert!(!fehlgeschlagen(&wurzel, "file:///a.png", 200));
        assert_eq!(mtime_wert("1700000000.5"), Some(1_700_000_000));
        let _ = std::fs::remove_dir_all(&d);
    }
}

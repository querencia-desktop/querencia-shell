//! Making a thumbnail for the cache (worker threads only): pictures GTK
//! decodes itself (PNG, JPEG, TIFF) in the process, other pictures through
//! GdkPixbuf, everything else (and pictures both fail on) through the
//! installed thumbnailers of the freedesktop.org scheme
//! (`$XDG_DATA_HOME/thumbnailers`, `$XDG_DATA_DIRS/thumbnailers`,
//! `*.thumbnailer` with `TryExec`, `Exec` and `MimeType`). PDF and other
//! documents get a thumbnail only if such a thumbnailer is installed
//! (e.g. one of Querencia Dokumente or Poppler's); none is not an error.
//!
//! The result is the PNG for the cache with the tags of the standard
//! (`cache::mit_tags`).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{gdk, gdk_pixbuf, gio, glib};

use super::bild::Bild;
use super::cache::{self, Eimer};

/// What GTK's own loaders decode (no extra process, no sandbox).
const GTK_TYPEN: [&str; 4] = ["image/png", "image/jpeg", "image/tiff", "image/x-tiff"];
/// A thumbnailer that takes longer is stopped (a broken file must not
/// hold a worker).
const ZEITLIMIT: Duration = Duration::from_secs(20);
/// Output of a thumbnailer beyond this is not read.
const MAX_AUSGABE: u64 = 8 * 1024 * 1024;

/// One `.thumbnailer` file.
#[derive(Clone, Debug, PartialEq)]
pub struct Thumbnailer {
    pub name: String,
    pub exec: String,
    pub typen: Vec<String>,
}

/// Why no thumbnail was made.
#[derive(Debug, PartialEq)]
pub enum Fehler {
    /// Nothing here can make one for this type (no note in `fail/`).
    KeinWeg,
    /// The job was withdrawn while running.
    Abgebrochen,
    /// Tried and failed (noted in `fail/`, tried again when the file
    /// changes).
    Fehlgeschlagen(String),
}

/// The file to make a thumbnail of.
#[derive(Clone, Debug)]
pub struct Quelle {
    pub uri: String,
    /// Local path (native files); `None` reads through GIO.
    pub pfad: Option<PathBuf>,
    pub typ: String,
    pub mtime: i64,
    pub groesse: u64,
}

/// Reads one `.thumbnailer` file (key file, group `Thumbnailer Entry`).
pub fn thumbnailer_lesen(name: &str, text: &str) -> Option<Thumbnailer> {
    let mut gruppe = false;
    let (mut exec, mut try_exec, mut typen) = (None, None, Vec::new());
    for zeile in text.lines().map(str::trim) {
        if zeile.starts_with('[') {
            gruppe = zeile == "[Thumbnailer Entry]";
            continue;
        }
        if !gruppe || zeile.starts_with('#') {
            continue;
        }
        let Some((k, v)) = zeile.split_once('=') else {
            continue;
        };
        match k.trim() {
            "Exec" => exec = Some(v.trim().to_owned()),
            "TryExec" => try_exec = Some(v.trim().to_owned()),
            "MimeType" => {
                typen = v
                    .split(';')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            _ => {}
        }
    }
    let exec = exec?;
    let programm = try_exec.or_else(|| argumente(&exec).into_iter().next())?;
    if !ausfuehrbar(&programm) || typen.is_empty() {
        return None;
    }
    Some(Thumbnailer {
        name: name.to_owned(),
        exec,
        typen,
    })
}

/// Whether `programm` (absolute or in `PATH`) can be run.
fn ausfuehrbar(programm: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let laeuft = |p: &Path| {
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if programm.contains('/') {
        return laeuft(Path::new(programm));
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|d| laeuft(&d.join(programm))))
}

/// The installed thumbnailers, the user's first (a file of the same name
/// in a later folder is hidden by an earlier one, as GNOME does).
pub fn alle() -> &'static [Thumbnailer] {
    static ALLE: OnceLock<Vec<Thumbnailer>> = OnceLock::new();
    ALLE.get_or_init(|| {
        let mut ordner = vec![glib::user_data_dir()];
        ordner.extend(glib::system_data_dirs());
        let mut gesehen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for d in ordner {
            let Ok(liste) = std::fs::read_dir(d.join("thumbnailers")) else {
                continue;
            };
            let mut namen: Vec<PathBuf> = liste
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "thumbnailer"))
                .collect();
            namen.sort();
            for p in namen {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !gesehen.insert(name.clone()) {
                    continue;
                }
                if let Some(t) = std::fs::read_to_string(&p)
                    .ok()
                    .and_then(|text| thumbnailer_lesen(&name, &text))
                {
                    out.push(t);
                }
            }
        }
        out
    })
}

/// The thumbnailer for `typ`: one that names it, else one that names a
/// type `typ` is a kind of.
pub fn thumbnailer_fuer<'a>(liste: &'a [Thumbnailer], typ: &str) -> Option<&'a Thumbnailer> {
    liste
        .iter()
        .find(|t| t.typen.iter().any(|m| m == typ))
        .or_else(|| {
            liste.iter().find(|t| {
                t.typen
                    .iter()
                    .any(|m| gio::content_type_is_a(typ, m.as_str()))
            })
        })
}

/// The MIME types GdkPixbuf can load.
fn pixbuf_typen() -> &'static [String] {
    static TYPEN: OnceLock<Vec<String>> = OnceLock::new();
    TYPEN.get_or_init(|| {
        gdk_pixbuf::Pixbuf::formats()
            .iter()
            .flat_map(|f| f.mime_types())
            .map(|t| t.to_string())
            .collect()
    })
}

fn gtk_laedt(typ: &str) -> bool {
    GTK_TYPEN.contains(&typ)
}

fn pixbuf_laedt(typ: &str) -> bool {
    typ.starts_with("image/") && pixbuf_typen().iter().any(|t| t == typ)
}

/// Whether anything here can make a thumbnail of `typ` (asked on the main
/// thread before a job is queued).
pub fn moeglich(typ: &str) -> bool {
    gtk_laedt(typ) || pixbuf_laedt(typ) || thumbnailer_fuer(alle(), typ).is_some()
}

/// The arguments of an `Exec` line (whitespace separates, double quotes
/// group, backslash escapes inside quotes).
pub fn argumente(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut jetzt = String::new();
    let mut in_quote = false;
    let mut hat = false;
    let mut zeichen = exec.chars();
    while let Some(c) = zeichen.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                hat = true;
            }
            '\\' if in_quote => {
                if let Some(n) = zeichen.next() {
                    jetzt.push(n);
                }
            }
            c if c.is_whitespace() && !in_quote => {
                if hat || !jetzt.is_empty() {
                    out.push(std::mem::take(&mut jetzt));
                    hat = false;
                }
            }
            c => jetzt.push(c),
        }
    }
    if hat || !jetzt.is_empty() {
        out.push(jetzt);
    }
    out
}

/// The command line of `exec` for one file: `%u` URI, `%i` path, `%o`
/// output, `%s` size, `%%` a percent sign. `None` if `%i` is asked for a
/// file without a path.
pub fn befehl(
    exec: &str,
    uri: &str,
    pfad: Option<&Path>,
    ausgabe: &Path,
    kante: u32,
) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for arg in argumente(exec) {
        let mut neu = String::new();
        let mut z = arg.chars();
        while let Some(c) = z.next() {
            if c != '%' {
                neu.push(c);
                continue;
            }
            match z.next() {
                Some('u') => neu.push_str(uri),
                Some('i') => neu.push_str(&pfad?.to_string_lossy()),
                Some('o') => neu.push_str(&ausgabe.to_string_lossy()),
                Some('s') => neu.push_str(&kante.to_string()),
                Some('%') => neu.push('%'),
                _ => {}
            }
        }
        out.push(neu);
    }
    (!out.is_empty()).then_some(out)
}

/// The bytes of the file (local or through GIO).
fn inhalt(q: &Quelle) -> Result<Vec<u8>, Fehler> {
    match &q.pfad {
        Some(p) => std::fs::read(p).map_err(|e| Fehler::Fehlgeschlagen(e.to_string())),
        None => gio::File::for_uri(&q.uri)
            .load_contents(gio::Cancellable::NONE)
            .map(|(b, _)| b.to_vec())
            .map_err(|e| Fehler::Fehlgeschlagen(e.to_string())),
    }
}

fn ueber_gtk(q: &Quelle, kante: u32) -> Result<Bild, Fehler> {
    let bytes = glib::Bytes::from_owned(inhalt(q)?);
    let textur =
        gdk::Texture::from_bytes(&bytes).map_err(|e| Fehler::Fehlgeschlagen(e.to_string()))?;
    let bild = Bild::aus_textur(&textur)
        .ok_or_else(|| Fehler::Fehlgeschlagen("leeres Bild".to_owned()))?;
    drop(textur);
    Ok(bild.verkleinert(kante))
}

fn ueber_pixbuf(q: &Quelle, kante: u32) -> Result<Bild, Fehler> {
    let k = kante as i32;
    let pixbuf = match &q.pfad {
        Some(p) => gdk_pixbuf::Pixbuf::from_file_at_scale(p, k, k, true),
        None => {
            let bytes = glib::Bytes::from_owned(inhalt(q)?);
            let strom = gio::MemoryInputStream::from_bytes(&bytes);
            gdk_pixbuf::Pixbuf::from_stream_at_scale(&strom, k, k, true, gio::Cancellable::NONE)
        }
    }
    .map_err(|e| Fehler::Fehlgeschlagen(e.to_string()))?;
    let (b, h) = (pixbuf.width(), pixbuf.height());
    if b <= 0 || h <= 0 {
        return Err(Fehler::Fehlgeschlagen("leeres Bild".to_owned()));
    }
    let daten = pixbuf.read_pixel_bytes();
    let bild = Bild::aus_rgba(
        b as u32,
        h as u32,
        pixbuf.n_channels() as u32,
        pixbuf.rowstride() as usize,
        &daten,
    );
    Ok(bild.verkleinert(kante))
}

static NUMMER: AtomicU64 = AtomicU64::new(0);

/// A private folder for thumbnailer output (0700, below the temporary
/// folder, one per process).
fn ausgabe_ordner() -> Option<&'static Path> {
    static ORDNER: OnceLock<Option<PathBuf>> = OnceLock::new();
    ORDNER
        .get_or_init(|| {
            use std::os::unix::fs::DirBuilderExt;
            let d = std::env::temp_dir().join(format!(
                "querencia-dateien-miniaturen-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::DirBuilder::new().mode(0o700).create(&d).ok()?;
            Some(d)
        })
        .as_deref()
}

/// Removes the output folder (at exit).
pub fn aufraeumen() {
    if let Some(d) = ausgabe_ordner() {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn ueber_thumbnailer(
    q: &Quelle,
    t: &Thumbnailer,
    kante: u32,
    abbruch: &AtomicBool,
) -> Result<Vec<u8>, Fehler> {
    let ordner = ausgabe_ordner()
        .ok_or_else(|| Fehler::Fehlgeschlagen("kein temporärer Ordner".to_owned()))?;
    let ausgabe = ordner.join(format!("{}.png", NUMMER.fetch_add(1, Ordering::Relaxed)));
    let args =
        befehl(&t.exec, &q.uri, q.pfad.as_deref(), &ausgabe, kante).ok_or(Fehler::KeinWeg)?;
    let start = Instant::now();
    let mut kind = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Fehler::Fehlgeschlagen(format!("{}: {e}", t.name)))?;
    let status = loop {
        if let Ok(Some(s)) = kind.try_wait() {
            break Some(s);
        }
        if abbruch.load(Ordering::Relaxed) || start.elapsed() > ZEITLIMIT {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let lesen = || -> Option<Vec<u8>> {
        let f = std::fs::File::open(&ausgabe).ok()?;
        let mut v = Vec::new();
        f.take(MAX_AUSGABE).read_to_end(&mut v).ok()?;
        Some(v)
    };
    let bytes = status.filter(|s| s.success()).and_then(|_| lesen());
    let _ = std::fs::remove_file(&ausgabe);
    if abbruch.load(Ordering::Relaxed) {
        return Err(Fehler::Abgebrochen);
    }
    let Some(bytes) = bytes else {
        return Err(Fehler::Fehlgeschlagen(match status {
            None => format!("{}: Zeitlimit", t.name),
            Some(s) => format!("{}: {s}", t.name),
        }));
    };
    // a PNG of the right size is kept as the thumbnailer wrote it; others
    // (too big, another format) are decoded and written anew
    if cache::png_masse(&bytes).is_some_and(|(b, h)| b.max(h) <= kante && b > 0 && h > 0) {
        return Ok(bytes);
    }
    let textur = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes))
        .map_err(|e| Fehler::Fehlgeschlagen(format!("{}: {e}", t.name)))?;
    let bild = Bild::aus_textur(&textur)
        .ok_or_else(|| Fehler::Fehlgeschlagen("leeres Bild".to_owned()))?;
    Ok(bild.verkleinert(kante).png())
}

/// How a thumbnail of `typ` is made (for the log and the checks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weg {
    Gtk,
    Pixbuf,
    Thumbnailer,
}

/// Makes the PNG for `eimer` (with tags) of `q`; the way taken is
/// returned for the statistics.
pub fn erzeugen(q: &Quelle, eimer: Eimer, abbruch: &AtomicBool) -> Result<(Vec<u8>, Weg), Fehler> {
    let kante = eimer.kante();
    let mut fehler = Fehler::KeinWeg;
    let mut ergebnis: Option<(Vec<u8>, Weg)> = None;
    if gtk_laedt(&q.typ) {
        match ueber_gtk(q, kante) {
            Ok(b) => ergebnis = Some((b.png(), Weg::Gtk)),
            Err(e) => fehler = e,
        }
    }
    if ergebnis.is_none() && pixbuf_laedt(&q.typ) {
        match ueber_pixbuf(q, kante) {
            Ok(b) => ergebnis = Some((b.png(), Weg::Pixbuf)),
            Err(e) => fehler = e,
        }
    }
    if ergebnis.is_none()
        && let Some(t) = thumbnailer_fuer(alle(), &q.typ)
    {
        match ueber_thumbnailer(q, t, kante, abbruch) {
            Ok(b) => ergebnis = Some((b, Weg::Thumbnailer)),
            Err(e) => fehler = e,
        }
    }
    let Some((png, weg)) = ergebnis else {
        return Err(fehler);
    };
    let mtime = q.mtime.to_string();
    let groesse = q.groesse.to_string();
    let tags = cache::tags(&q.uri, &mtime, &groesse, &q.typ);
    cache::mit_tags(&png, &tags)
        .map(|p| (p, weg))
        .ok_or_else(|| Fehler::Fehlgeschlagen("kein PNG".to_owned()))
}

/// The note of a failure: a 1×1 PNG with the tags.
pub fn fehl_png(q: &Quelle) -> Option<Vec<u8>> {
    let png = Bild::leer(1, 1).png();
    let mtime = q.mtime.to_string();
    let groesse = q.groesse.to_string();
    cache::mit_tags(&png, &cache::tags(&q.uri, &mtime, &groesse, &q.typ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_lines_are_split_and_filled_in() {
        assert_eq!(
            argumente(r#"/usr/bin/x -s %s "%u" %o"#),
            ["/usr/bin/x", "-s", "%s", "%u", "%o"]
        );
        assert_eq!(argumente(r#"a "b c" "" d\e"#), ["a", "b c", "", r"d\e"]);
        let b = befehl(
            "glycin-thumbnailer --input %u --output %o --size %s 100%%",
            "file:///a%20b.jpg",
            Some(Path::new("/a b.jpg")),
            Path::new("/tmp/1.png"),
            128,
        )
        .unwrap();
        assert_eq!(
            b,
            [
                "glycin-thumbnailer",
                "--input",
                "file:///a%20b.jpg",
                "--output",
                "/tmp/1.png",
                "--size",
                "128",
                "100%"
            ]
        );
        assert_eq!(
            befehl("x %i %o", "sftp://h/a", None, Path::new("/o"), 128),
            None,
            "%i braucht einen Pfad"
        );
    }

    #[test]
    fn thumbnailer_files_need_exec_types_and_a_program() {
        let text = "[Thumbnailer Entry]\nTryExec=/bin/sh\nExec=/bin/sh -c x %u %o\n\
                    MimeType=application/pdf;application/x-pdf;\n";
        let t = thumbnailer_lesen("p.thumbnailer", text).unwrap();
        assert_eq!(t.typen, ["application/pdf", "application/x-pdf"]);
        assert!(
            thumbnailer_lesen(
                "q",
                "[Thumbnailer Entry]\nTryExec=/gibt/es/nicht\nExec=x\nMimeType=a/b;\n"
            )
            .is_none()
        );
        assert!(thumbnailer_lesen("q", "[Andere]\nExec=/bin/sh\nMimeType=a/b;\n").is_none());
        assert!(thumbnailer_lesen("q", "[Thumbnailer Entry]\nExec=/bin/sh\n").is_none());
        // without TryExec the program of Exec counts
        assert!(
            thumbnailer_lesen("q", "[Thumbnailer Entry]\nExec=sh %u\nMimeType=a/b;\n").is_some()
        );
        let liste = vec![t];
        assert!(thumbnailer_fuer(&liste, "application/pdf").is_some());
        assert!(thumbnailer_fuer(&liste, "text/plain").is_none());
    }

    #[test]
    fn gtk_types_are_decoded_in_process() {
        assert!(gtk_laedt("image/jpeg"));
        assert!(gtk_laedt("image/png"));
        assert!(!gtk_laedt("image/webp"));
        assert!(!gtk_laedt("application/pdf"));
    }
}

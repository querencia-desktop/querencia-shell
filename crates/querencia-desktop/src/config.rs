//! Desktop background configuration (HMP-08): `~/.config/querencia/desktop.ini`
//! (XDG config dir), a GLib key file like the panel's `panel.ini`:
//!
//! ```ini
//! [Background]
//! # QE-01: color = the colour only, no image at all (not even the
//! # built-in one); missing or image = an image. Written only for color.
//! Type=color
//! # image; empty or missing: the built-in image (tiefblau, else Querencia Blau)
//! File=/home/user/Bilder/berge.jpg
//! # zoom (fill the screen, crop, never distort) | scaled (whole image,
//! # bars in Color) | stretched (fill, distorted, as GNOME 2 offered it)
//! # | centered (original size in the middle, QE-01) | tiled (original
//! # size, repeated from the top left, QE-01)
//! Mode=zoom
//! # behind the image and while it loads; also the whole desktop if the
//! # image cannot be read
//! Color=#1c4e96
//! ```
//!
//! Missing file, unknown keys or bad values never stop the desktop: every
//! value falls back to its default, and the reason is logged once.

use std::path::{Path, PathBuf};

/// How the image fills a monitor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Fill the whole screen, keep the aspect ratio, crop what is too much.
    #[default]
    Zoom,
    /// Show the whole image, keep the aspect ratio, bars in the colour.
    Scaled,
    /// Fill the whole screen, ignore the aspect ratio.
    Stretched,
    /// QE-01: original size in the middle, the rest in the colour.
    Centered,
    /// QE-01: original size, repeated from the top left.
    Tiled,
}

/// QE-01: an image or the colour only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Image,
    Color,
}

impl Mode {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "zoom" => Some(Self::Zoom),
            "scaled" => Some(Self::Scaled),
            "stretched" => Some(Self::Stretched),
            "centered" => Some(Self::Centered),
            "tiled" => Some(Self::Tiled),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Zoom => "zoom",
            Self::Scaled => "scaled",
            Self::Stretched => "stretched",
            Self::Centered => "centered",
            Self::Tiled => "tiled",
        }
    }
}

/// Default colour: the blue at the bottom of "Querencia Blau".
pub const DEFAULT_COLOR: (u8, u8, u8) = (0x1c, 0x4e, 0x96);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    /// QE-01: `Color` shows the colour only; `file` is kept, not loaded.
    pub kind: Kind,
    /// `None`: the built-in image.
    pub file: Option<PathBuf>,
    pub mode: Mode,
    pub color: (u8, u8, u8),
}

impl Default for Background {
    fn default() -> Self {
        Self {
            kind: Kind::default(),
            file: None,
            mode: Mode::default(),
            color: DEFAULT_COLOR,
        }
    }
}

/// `#rrggbb` (or `rrggbb`) to RGB.
pub fn parse_color(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
}

/// The `[Background]` group of a key file's text. Unknown or bad values
/// fall back to the defaults; `problems` lists what was ignored.
pub fn parse(text: &str) -> (Background, Vec<String>) {
    let mut background = Background::default();
    let mut problems = Vec::new();
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_group = line == "[Background]";
            continue;
        }
        if !in_group {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!("Zeile ohne „=“: {line}"));
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Type" => match value.to_ascii_lowercase().as_str() {
                "image" => background.kind = Kind::Image,
                "color" => background.kind = Kind::Color,
                _ => problems.push(format!("unbekannter Typ „{value}“, image")),
            },
            "File" if value.is_empty() => background.file = None,
            "File" => background.file = Some(expand_home(value)),
            "Mode" => match Mode::parse(value) {
                Some(mode) => background.mode = mode,
                None => problems.push(format!("unbekannter Modus „{value}“, zoom")),
            },
            "Color" => match parse_color(value) {
                Some(color) => background.color = color,
                None => problems.push(format!("ungültige Farbe „{value}“")),
            },
            other => problems.push(format!("unbekannter Schlüssel „{other}“")),
        }
    }
    (background, problems)
}

/// QE-04 (QD-042): which icons the desktop shows and how large; groups
/// `[Icons]` and `[Volumes]` of `desktop.ini`, written by Querencia
/// Einstellungen ("Symbole der Arbeitsfläche"). Format and the rule that
/// sorts a mount into a type: `docs/qe-04-symbole-der-arbeitsflaeche.md`
/// (copy of the Einstellungen specification `desktop-ini-arbeitsflaeche.md`).
///
/// ```ini
/// [Icons]
/// Computer=true
/// Home=true
/// Trash=true
/// # 32 | 48 | 72 | 96
/// Size=48
///
/// [Volumes]
/// Removable=true
/// Optical=true
/// Network=true
/// Internal=false
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconSettings {
    pub computer: bool,
    pub home: bool,
    pub trash: bool,
    /// Icon size in pixels, one of [`ICON_SIZES`].
    pub size: i32,
    pub removable: bool,
    pub optical: bool,
    pub network: bool,
    pub internal: bool,
}

/// The sizes Querencia Einstellungen offers (Nautilus' 66/100/150/200 %).
pub const ICON_SIZES: [i32; 4] = [32, 48, 72, 96];

impl Default for IconSettings {
    /// Without `desktop.ini` – the **only** place of these values on the
    /// desktop side; the same as `Arbeitsflaeche::default()` in Querencia
    /// Einstellungen (decided 19.10.2026, as on GNOME 2): the three special
    /// icons, 48 px, removable, optical and network volumes, no internal ones.
    fn default() -> Self {
        Self {
            computer: true,
            home: true,
            trash: true,
            size: 48,
            removable: true,
            optical: true,
            network: true,
            internal: false,
        }
    }
}

/// `true`/`false`, `1`/`0`, `yes`/`no`, in any case.
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// The `[Icons]` and `[Volumes]` groups; like [`parse`], bad or unknown
/// values fall back to the default and are listed in `problems`.
pub fn parse_icons(text: &str) -> (IconSettings, Vec<String>) {
    let mut settings = IconSettings::default();
    let mut problems = Vec::new();
    let mut group: Option<&str> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            group = match name {
                "Icons" => Some("Icons"),
                "Volumes" => Some("Volumes"),
                _ => None,
            };
            continue;
        }
        let Some(group) = group else { continue };
        let Some((key, value)) = line.split_once('=') else {
            problems.push(format!("[{group}] Zeile ohne „=“: {line}"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if group == "Icons" && key == "Size" {
            match value.parse::<i32>().ok().filter(|v| ICON_SIZES.contains(v)) {
                Some(size) => settings.size = size,
                None => problems.push(format!("[Icons] ungültige Größe „{value}“, 48")),
            }
            continue;
        }
        let target = match (group, key) {
            ("Icons", "Computer") => &mut settings.computer,
            ("Icons", "Home") => &mut settings.home,
            ("Icons", "Trash") => &mut settings.trash,
            ("Volumes", "Removable") => &mut settings.removable,
            ("Volumes", "Optical") => &mut settings.optical,
            ("Volumes", "Network") => &mut settings.network,
            ("Volumes", "Internal") => &mut settings.internal,
            _ => {
                problems.push(format!("[{group}] unbekannter Schlüssel „{key}“"));
                continue;
            }
        };
        match parse_bool(value) {
            Some(b) => *target = b,
            None => problems.push(format!("[{group}] ungültiger Wert „{value}“ für {key}")),
        }
    }
    (settings, problems)
}

/// QE-04: `text` with the group `name` replaced by `block` (which starts
/// with `[name]` and ends with a newline); everything else stays byte for
/// byte. A group's block runs from its header to its last key line, so
/// comments right before the next header stay with that one. Several
/// blocks of the group become one, at the first; a missing group is
/// appended after an empty line. The same rule as `ini.rs` in Querencia
/// Einstellungen.
pub fn replace_group(text: &str, name: &str, block: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let header = |line: &str| {
        let line = line.trim();
        line.strip_prefix('[')
            .and_then(|l| l.strip_suffix(']'))
            .map(str::to_owned)
    };
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if header(lines[i]).as_deref() == Some(name) {
            let (start, mut end) = (i, i + 1);
            i += 1;
            while i < lines.len() && header(lines[i]).is_none() {
                let line = lines[i].trim();
                if !line.is_empty() && !line.starts_with('#') {
                    end = i + 1;
                }
                i += 1;
            }
            blocks.push((start, end));
        } else {
            i += 1;
        }
    }
    let Some(&(first, _)) = blocks.first() else {
        if text.trim().is_empty() {
            return block.to_owned();
        }
        let mut out = text.to_owned();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(block);
        return out;
    };
    let mut out = String::with_capacity(text.len() + block.len());
    let mut at = 0;
    for &(start, end) in &blocks {
        out.push_str(&lines[at..start].concat());
        if start == first {
            out.push_str(block);
        }
        at = end;
    }
    let rest = lines[at..].concat();
    if !rest.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&rest);
    out
}

/// The key file text for `background`. Written by the desktop menu's
/// "Hintergrund der Arbeitsfläche ändern" (HMP-09).
/// QE-01: `Type=color` is written only for the colour (an image setting
/// stays byte for byte as HMP-10a wrote it), in the same order as Querencia
/// Einstellungen writes the file.
pub fn render(background: &Background) -> String {
    let (r, g, b) = background.color;
    let kind = match background.kind {
        Kind::Color => "Type=color\n",
        Kind::Image => "",
    };
    format!(
        "[Background]\n{kind}File={}\nMode={}\nColor=#{r:02x}{g:02x}{b:02x}\n",
        background
            .file
            .as_ref()
            .map(|f| f.display().to_string())
            .unwrap_or_default(),
        background.mode.name()
    )
}

fn expand_home(value: &str) -> PathBuf {
    match value.strip_prefix("~/") {
        Some(rest) => gtk4::glib::home_dir().join(rest),
        None => PathBuf::from(value),
    }
}

/// `desktop.ini` in the user's Querencia config directory.
pub fn config_path() -> PathBuf {
    gtk4::glib::user_config_dir()
        .join("querencia")
        .join("desktop.ini")
}

/// Names of the built-in image, first found wins: "tiefblau" (Hintergründe
/// 0.2, package `querencia-hintergruende`, standard since 21.10.2026), then
/// the old "Querencia Blau" while the 0.2 images are not installed.
pub const BUILT_IN: [&str; 2] = ["querencia-tiefblau.png", "querencia-blau.png"];

/// Where the built-in image is looked for: `querencia/hintergruende/` in the
/// XDG data dirs (installed), then the source tree (development build); each
/// name of [`BUILT_IN`] in all of them before the next name.
pub fn built_in_candidates(data_dirs: &[PathBuf], source_tree: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = data_dirs
        .iter()
        .map(|dir| dir.join("querencia").join("hintergruende"))
        .collect();
    dirs.push(source_tree.join("data").join("hintergruende"));
    BUILT_IN
        .iter()
        .flat_map(|name| dirs.iter().map(move |dir| dir.join(name)))
        .collect()
}

/// Rectangle (x, y, width, height) the image is drawn into on a monitor of
/// `out` size, for an image of `img` size (logical pixels). Zoom and scaled
/// keep the aspect ratio (zoom covers and is cropped by the monitor, scaled
/// fits inside, centred); stretched fills exactly; centered (QE-01) keeps
/// the original size in the middle (cropped when larger); tiled (QE-01):
/// the first tile at the top left, repeated by the caller.
pub fn placement(mode: Mode, img: (f64, f64), out: (f64, f64)) -> (f64, f64, f64, f64) {
    let (iw, ih) = img;
    let (ow, oh) = out;
    if iw <= 0.0 || ih <= 0.0 || mode == Mode::Stretched {
        return (0.0, 0.0, ow, oh);
    }
    if mode == Mode::Tiled {
        return (0.0, 0.0, iw, ih);
    }
    let scale = match mode {
        Mode::Zoom => (ow / iw).max(oh / ih),
        Mode::Scaled => (ow / iw).min(oh / ih),
        _ => 1.0,
    };
    let (w, h) = (iw * scale, ih * scale);
    ((ow - w) / 2.0, (oh - h) / 2.0, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_config_is_the_built_in_blue() {
        assert_eq!(parse(""), (Background::default(), vec![]));
        let (bg, problems) = parse("[Background]\nFile=\n");
        assert_eq!(bg.file, None);
        assert!(problems.is_empty());
    }

    #[test]
    fn values_are_read_and_bad_ones_fall_back() {
        let (bg, problems) = parse(
            "# Kommentar\n[Other]\nMode=scaled\n[Background]\nFile=/tmp/a.png\nMode=Scaled\nColor=#102030\n",
        );
        assert_eq!(bg.file.as_deref(), Some(Path::new("/tmp/a.png")));
        assert_eq!(bg.mode, Mode::Scaled);
        assert_eq!(bg.color, (0x10, 0x20, 0x30));
        assert!(problems.is_empty(), "{problems:?}");

        let (bg, problems) = parse("[Background]\nMode=kachel\nColor=blau\nFoo=1\nkaputt\n");
        assert_eq!(bg.mode, Mode::Zoom);
        assert_eq!(bg.color, DEFAULT_COLOR);
        assert_eq!(problems.len(), 4, "{problems:?}");
    }

    #[test]
    fn keys_of_other_groups_are_ignored() {
        let (bg, problems) = parse(
            "[Background]\nMode=scaled\n[Icons]\nMode=stretched\nColor=#000000\nFile=/x.png\n",
        );
        assert_eq!(bg.mode, Mode::Scaled);
        assert_eq!(bg.color, DEFAULT_COLOR);
        assert_eq!(bg.file, None);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn home_is_expanded_and_config_round_trips() {
        let (bg, _) = parse("[Background]\nFile=~/Bilder/x.jpg\n");
        assert_eq!(bg.file, Some(gtk4::glib::home_dir().join("Bilder/x.jpg")));
        let original = Background {
            kind: Kind::Image,
            file: Some(PathBuf::from("/srv/bild.png")),
            mode: Mode::Stretched,
            color: (1, 2, 255),
        };
        assert_eq!(parse(&render(&original)).0, original);
        assert_eq!(
            parse(&render(&Background::default())).0,
            Background::default()
        );
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#1c4e96"), Some((0x1c, 0x4e, 0x96)));
        assert_eq!(parse_color("FFFFFF"), Some((255, 255, 255)));
        for bad in ["", "#123", "#12345g", "rot", "#1234567"] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
    }

    /// The four resolutions of the task and the test output, with the
    /// 16:9 built-in image: nothing is distorted, the screen is covered.
    #[test]
    fn zoom_covers_every_resolution_without_distortion() {
        let img = (2560.0, 1440.0);
        for out in [
            (1920.0, 1080.0),
            (1600.0, 900.0),
            (1366.0, 768.0),
            (1280.0, 720.0),
            (1280.0, 1024.0),
            (1080.0, 1920.0),
        ] {
            let (x, y, w, h) = placement(Mode::Zoom, img, out);
            assert!((w / h - img.0 / img.1).abs() < 1e-9, "verzerrt bei {out:?}");
            assert!(x <= 0.0 && y <= 0.0, "nicht bedeckt bei {out:?}");
            assert!(
                x + w >= out.0 - 1e-9 && y + h >= out.1 - 1e-9,
                "nicht bedeckt bei {out:?}"
            );
            assert!((x + w / 2.0 - out.0 / 2.0).abs() < 1e-9, "nicht zentriert");
        }
        // 16:9 on 16:9: exactly the screen
        assert_eq!(
            placement(Mode::Zoom, img, (1920.0, 1080.0)),
            (0.0, 0.0, 1920.0, 1080.0)
        );
        // 1366x768 is not exactly 16:9: a few pixels are cropped
        let (x, _, w, _) = placement(Mode::Zoom, img, (1366.0, 768.0));
        assert!(x <= 0.0 && w >= 1366.0);
    }

    #[test]
    fn scaled_fits_inside_and_stretched_fills() {
        let (x, y, w, h) = placement(Mode::Scaled, (2560.0, 1440.0), (1280.0, 1024.0));
        assert_eq!((x, w), (0.0, 1280.0));
        assert!((h - 720.0).abs() < 1e-9 && (y - 152.0).abs() < 1e-9);
        assert_eq!(
            placement(Mode::Stretched, (100.0, 50.0), (1280.0, 1024.0)),
            (0.0, 0.0, 1280.0, 1024.0)
        );
        // an unreadable image (0x0): the colour fills the screen
        assert_eq!(
            placement(Mode::Zoom, (0.0, 0.0), (800.0, 600.0)),
            (0.0, 0.0, 800.0, 600.0)
        );
    }

    #[test]
    fn built_in_image_is_found_installed_or_in_the_source_tree() {
        let list = built_in_candidates(&[PathBuf::from("/usr/share")], Path::new("/src/q"));
        assert_eq!(
            list,
            [
                PathBuf::from("/usr/share/querencia/hintergruende/querencia-tiefblau.png"),
                PathBuf::from("/src/q/data/hintergruende/querencia-tiefblau.png"),
                PathBuf::from("/usr/share/querencia/hintergruende/querencia-blau.png"),
                PathBuf::from("/src/q/data/hintergruende/querencia-blau.png"),
            ]
        );
    }

    /// QE-01 A1: `Type=color` is read and written; an image setting stays
    /// byte for byte as HMP-10a (and Querencia Einstellungen) write it.
    #[test]
    fn the_colour_only_type_round_trips() {
        let image = Background {
            kind: Kind::Image,
            file: Some(PathBuf::from("/srv/bild.png")),
            mode: Mode::Zoom,
            color: (0x20, 0x4a, 0x87),
        };
        assert_eq!(
            render(&image),
            "[Background]\nFile=/srv/bild.png\nMode=zoom\nColor=#204a87\n",
            "wie HMP-10a, ohne Type"
        );
        let color = Background {
            kind: Kind::Color,
            ..image.clone()
        };
        assert_eq!(
            render(&color),
            "[Background]\nType=color\nFile=/srv/bild.png\nMode=zoom\nColor=#204a87\n",
            "wie Querencia Einstellungen schreibt"
        );
        assert_eq!(parse(&render(&color)), (color, vec![]));
        let (bg, problems) = parse("[Background]\nType=Color\n");
        assert_eq!(bg.kind, Kind::Color);
        assert!(problems.is_empty());
        let (bg, problems) = parse("[Background]\nType=muster\n");
        assert_eq!(bg.kind, Kind::Image);
        assert_eq!(problems, ["unbekannter Typ „muster“, image"]);
    }

    /// QE-04: defaults as decided, values read, bad ones reported.
    #[test]
    fn icon_settings_are_read_with_defaults() {
        let (settings, problems) = parse_icons("");
        assert_eq!(settings, IconSettings::default());
        assert!(problems.is_empty());
        let d = IconSettings::default();
        assert!(d.computer && d.home && d.trash && d.size == 48);
        assert!(d.removable && d.optical && d.network && !d.internal);
        let (settings, problems) = parse_icons(
            "[Background]\nTrash=false\n[Icons]\nComputer=false\nTrash=0\nSize=72\n\n\
             [Volumes]\nNetwork=NO\nInternal=yes\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings,
            IconSettings {
                computer: false,
                trash: false,
                size: 72,
                network: false,
                internal: true,
                ..IconSettings::default()
            }
        );
        let (settings, problems) =
            parse_icons("[Icons]\nSize=50\nHome=vielleicht\nFoo=1\n[Volumes]\nkaputt\n");
        assert_eq!(settings, IconSettings::default());
        assert_eq!(problems.len(), 4, "{problems:?}");
        // the background parser does not see the new groups
        let (background, problems) =
            parse("[Background]\nMode=scaled\n[Icons]\nSize=96\n[Volumes]\nNetwork=false\n");
        assert_eq!(background.mode, Mode::Scaled);
        assert!(problems.is_empty());
    }

    /// QE-04: the HMP-09 fallback writes only `[Background]`.
    #[test]
    fn replacing_a_group_keeps_the_others() {
        let other = "# Hand\n[Background]\nFile=\nMode=zoom\nColor=#1c4e96\n\n[Icons]\nSize=96\n\n[Volumes]\nNetwork=false\n";
        let new = replace_group(
            other,
            "Background",
            &render(&Background {
                file: Some(PathBuf::from("/srv/b.png")),
                ..Background::default()
            }),
        );
        assert_eq!(
            new,
            "# Hand\n[Background]\nFile=/srv/b.png\nMode=zoom\nColor=#1c4e96\n\n[Icons]\nSize=96\n\n[Volumes]\nNetwork=false\n"
        );
        assert_eq!(
            replace_group("", "Background", "[Background]\nFile=\n"),
            "[Background]\nFile=\n"
        );
        assert_eq!(
            replace_group("[Icons]\nSize=32", "Volumes", "[Volumes]\nNetwork=true\n"),
            "[Icons]\nSize=32\n\n[Volumes]\nNetwork=true\n"
        );
        assert_eq!(
            replace_group("[A]\nx=1\n[A]\ny=2\n[B]\nz=3\n", "A", "[A]\nn=0\n"),
            "[A]\nn=0\n[B]\nz=3\n"
        );
    }

    /// QE-01 A2: centered and tiled, with the values of Querencia
    /// Einstellungen (`modell.rs`, `platzierung_haelt_proportionen`).
    #[test]
    fn centered_and_tiled_keep_the_original_size() {
        for (text, mode) in [("centered", Mode::Centered), ("tiled", Mode::Tiled)] {
            let (bg, problems) = parse(&format!("[Background]\nMode={text}\n"));
            assert_eq!((bg.mode, problems.len()), (mode, 0));
            assert_eq!(mode.name(), text);
            let back = Background {
                mode,
                ..Background::default()
            };
            assert_eq!(parse(&render(&back)).0, back);
        }
        let (img, out) = ((1600.0, 1200.0), (1920.0, 1080.0));
        assert_eq!(
            placement(Mode::Centered, img, out),
            (160.0, -60.0, 1600.0, 1200.0)
        );
        assert_eq!(
            placement(Mode::Centered, (200.0, 150.0), (1280.0, 720.0)),
            (540.0, 285.0, 200.0, 150.0)
        );
        assert_eq!(
            placement(Mode::Tiled, (500.0, 400.0), out),
            (0.0, 0.0, 500.0, 400.0)
        );
        // an unreadable image: the colour everywhere
        assert_eq!(
            placement(Mode::Tiled, (0.0, 0.0), out),
            (0.0, 0.0, 1920.0, 1080.0)
        );
        // the old modes are unchanged
        let (x, y, w, h) = placement(Mode::Scaled, img, out);
        assert!((h - 1080.0).abs() < 1e-9 && (w - 1440.0).abs() < 1e-9);
        assert!((x - 240.0).abs() < 1e-9 && y.abs() < 1e-9);
    }
}

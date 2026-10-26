//! What one StatusNotifierItem says about itself, read from its properties
//! (`org.kde.StatusNotifierItem`, `Properties.GetAll`), and how the
//! notification area shows it: which icon, which tooltip, hidden or
//! highlighted. Pure, tested without a display or bus.
//!
//! Specification: freedesktop.org StatusNotifierItem (draft 0.4, the KDE
//! interface names are the ones in use).

use std::path::PathBuf;

use gtk4::glib;

/// The interface every program offers.
pub const INTERFACE: &str = "org.kde.StatusNotifierItem";
/// The freedesktop name of the same interface, offered by a few programs
/// instead; read only if the KDE one gives nothing.
pub const INTERFACE_FREEDESKTOP: &str = "org.freedesktop.StatusNotifierItem";
/// Object path of an item that registered with its bus name only.
pub const DEFAULT_PATH: &str = "/StatusNotifierItem";
/// Largest pixmap side taken; bigger ones are left out (a program cannot
/// make the panel hold an arbitrary image).
const MAX_PIXMAP_SIDE: i32 = 1024;

/// Property `Status`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// Not shown (the specification lets a host hide it).
    Passive,
    #[default]
    Active,
    /// Shown with its attention icon and highlighted.
    NeedsAttention,
}

impl Status {
    /// The value of the property; anything unknown counts as active.
    pub fn parse(text: &str) -> Status {
        match text {
            "Passive" => Status::Passive,
            "NeedsAttention" => Status::NeedsAttention,
            _ => Status::Active,
        }
    }
}

/// One image of an `a(iiay)` property: ARGB32, network byte order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixmap {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

impl Pixmap {
    /// The pixels as RGBA (straight alpha), row after row, as gdk-pixbuf
    /// takes them.
    pub fn rgba(&self) -> Vec<u8> {
        self.data
            .chunks_exact(4)
            .flat_map(|p| [p[1], p[2], p[3], p[0]])
            .collect()
    }
}

/// The pixmaps of an `a(iiay)` value; images with a wrong size, no pixels
/// or more than [`MAX_PIXMAP_SIDE`] are left out.
pub fn pixmaps(value: &glib::Variant) -> Vec<Pixmap> {
    let Some(list) = value.get::<Vec<(i32, i32, Vec<u8>)>>() else {
        return Vec::new();
    };
    list.into_iter()
        .filter(|(w, h, data)| {
            (1..=MAX_PIXMAP_SIDE).contains(w)
                && (1..=MAX_PIXMAP_SIDE).contains(h)
                && data.len() == (*w as usize) * (*h as usize) * 4
        })
        .map(|(width, height, data)| Pixmap {
            width,
            height,
            data,
        })
        .collect()
}

/// The pixmap to draw at `size` pixels: the smallest that is at least as
/// big, else the biggest.
pub fn best_pixmap(pixmaps: &[Pixmap], size: i32) -> Option<&Pixmap> {
    let side = |p: &Pixmap| p.width.max(p.height);
    pixmaps
        .iter()
        .filter(|p| side(p) >= size)
        .min_by_key(|p| side(p))
        .or_else(|| pixmaps.iter().max_by_key(|p| side(p)))
}

/// Property `ToolTip`: `(s icon, a(iiay) pixmaps, s title, s text)`; the
/// icon is not shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tooltip {
    pub title: String,
    pub text: String,
}

/// The properties the notification area uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Eintrag {
    pub id: String,
    pub category: String,
    pub title: String,
    pub status: Status,
    pub icon_name: String,
    pub icon_pixmaps: Vec<Pixmap>,
    pub attention_name: String,
    pub attention_pixmaps: Vec<Pixmap>,
    pub overlay_name: String,
    pub overlay_pixmaps: Vec<Pixmap>,
    pub icon_theme_path: String,
    pub tooltip: Tooltip,
    /// Object path of the item's `com.canonical.dbusmenu`, `None` without.
    pub menu: Option<String>,
    /// The item only has a menu: a left click shows it too.
    pub item_is_menu: bool,
}

fn text(dict: &glib::VariantDict, name: &str) -> Option<String> {
    let value = dict.lookup_value(name, None)?;
    value.str().map(str::to_owned)
}

/// The tooltip of a `ToolTip` value; `None` for one of another type.
pub fn tooltip(value: &glib::Variant) -> Option<Tooltip> {
    if value.type_().as_str() != "(sa(iiay)ss)" {
        return None;
    }
    let part = |i| value.child_value(i).str().unwrap_or_default().to_owned();
    Some(Tooltip {
        title: part(2),
        text: part(3),
    })
}

impl Eintrag {
    /// Takes the properties of `werte` (`a{sv}` of `GetAll`); unknown names
    /// and values of a wrong type are left out, as missing ones.
    pub fn from_properties(werte: &glib::Variant) -> Option<Eintrag> {
        let dict = werte.get::<glib::VariantDict>()?;
        let pixmap_of = |name: &str| {
            dict.lookup_value(name, None)
                .map(|v| pixmaps(&v))
                .unwrap_or_default()
        };
        let menu = text(&dict, "Menu").filter(|path| !path.is_empty() && path != "/");
        Some(Eintrag {
            id: text(&dict, "Id").unwrap_or_default(),
            category: text(&dict, "Category").unwrap_or_default(),
            title: text(&dict, "Title").unwrap_or_default(),
            status: Status::parse(&text(&dict, "Status").unwrap_or_default()),
            icon_name: text(&dict, "IconName").unwrap_or_default(),
            icon_pixmaps: pixmap_of("IconPixmap"),
            attention_name: text(&dict, "AttentionIconName").unwrap_or_default(),
            attention_pixmaps: pixmap_of("AttentionIconPixmap"),
            overlay_name: text(&dict, "OverlayIconName").unwrap_or_default(),
            overlay_pixmaps: pixmap_of("OverlayIconPixmap"),
            icon_theme_path: text(&dict, "IconThemePath").unwrap_or_default(),
            tooltip: dict
                .lookup_value("ToolTip", None)
                .and_then(|v| tooltip(&v))
                .unwrap_or_default(),
            menu,
            item_is_menu: dict
                .lookup::<bool>("ItemIsMenu")
                .ok()
                .flatten()
                .unwrap_or(false),
        })
    }

    /// Whether the item is shown at all.
    pub fn sichtbar(&self) -> bool {
        self.status != Status::Passive
    }

    /// The icon to show, best first: the attention icon while the item
    /// needs attention (if it has one), else the normal one; a name before
    /// the pixmaps, as the specification asks. The caller takes the first
    /// it can draw (a name the theme lacks is skipped).
    pub fn symbol(&self) -> Vec<Quelle> {
        let attention = self.status == Status::NeedsAttention
            && (!self.attention_name.is_empty() || !self.attention_pixmaps.is_empty());
        let (name, pixmaps) = if attention {
            (&self.attention_name, &self.attention_pixmaps)
        } else {
            (&self.icon_name, &self.icon_pixmaps)
        };
        let mut liste = quellen(name, pixmaps);
        if attention {
            // an attention icon the theme lacks: the normal one is better
            // than nothing
            liste.extend(quellen(&self.icon_name, &self.icon_pixmaps));
        }
        liste
    }

    /// The small icon drawn over the corner of the main one.
    pub fn overlay(&self) -> Vec<Quelle> {
        quellen(&self.overlay_name, &self.overlay_pixmaps)
    }

    /// The tooltip text: the tooltip's title and text (without markup),
    /// else the title, else the ID.
    pub fn tooltip_text(&self) -> String {
        let title = self.tooltip.title.trim();
        let body = ohne_markup(&self.tooltip.text);
        let body = body.trim();
        match (title.is_empty(), body.is_empty()) {
            (false, false) => format!("{title}\n{body}"),
            (false, true) => title.to_owned(),
            (true, false) => body.to_owned(),
            (true, true) if !self.title.trim().is_empty() => self.title.trim().to_owned(),
            _ => self.id.clone(),
        }
    }
}

fn quellen(name: &str, pixmaps: &[Pixmap]) -> Vec<Quelle> {
    let mut quellen = Vec::new();
    if name.starts_with('/') {
        quellen.push(Quelle::Datei(PathBuf::from(name)));
    } else if !name.is_empty() {
        quellen.push(Quelle::Name(name.to_owned()));
    }
    if !pixmaps.is_empty() {
        quellen.push(Quelle::Pixmaps(pixmaps.to_vec()));
    }
    quellen
}

/// Where an icon comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Quelle {
    /// A name of the icon theme (or of the item's `IconThemePath`).
    Name(String),
    /// An image file named by its absolute path (some programs put one in
    /// `IconName`).
    Datei(PathBuf),
    Pixmaps(Vec<Pixmap>),
}

/// `text` without the markup the specification allows in a tooltip (a
/// subset of HTML): tags left out, `<br>` as a line break, the five XML
/// entities and numeric ones decoded.
pub fn ohne_markup(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            // a lone "<" is text
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end]
            .trim()
            .trim_end_matches('/')
            .trim()
            .to_ascii_lowercase();
        if tag == "br" || tag == "p" || tag == "/p" {
            out.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    entities(&out)
}

fn entities(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let decoded = after.find(';').filter(|end| *end <= 10).and_then(|end| {
            let name = &after[..end];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|ch| (ch, end))
        });
        match decoded {
            Some((ch, end)) => {
                out.push(ch);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The bus name and object path of an item from its ID in the watcher
/// (`RegisteredStatusNotifierItems`): `<bus name><path>` as the KDE watcher
/// forms it, or a bus name alone (then the path is [`DEFAULT_PATH`]).
pub fn adresse(id: &str) -> Option<(String, String)> {
    let (bus, path) = match id.find('/') {
        _ if id.is_empty() => return None,
        Some(0) => return None,
        Some(i) => (&id[..i], id[i..].to_owned()),
        None => (id, DEFAULT_PATH.to_owned()),
    };
    if !gtk4::gio::dbus_is_name(bus) || !glib::Variant::is_object_path(&path) {
        return None;
    }
    Some((bus.to_owned(), path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::prelude::*;

    fn dict(eintraege: &[(&str, glib::Variant)]) -> glib::Variant {
        let dict = glib::VariantDict::new(None);
        for (name, wert) in eintraege {
            dict.insert_value(name, wert);
        }
        dict.end()
    }

    fn pixmap(w: i32, h: i32) -> (i32, i32, Vec<u8>) {
        (w, h, vec![0xff; (w * h * 4) as usize])
    }

    #[test]
    fn properties_are_read_and_wrong_types_left_out() {
        let tooltip = (
            "symbol",
            vec![pixmap(2, 2)],
            "Webex",
            "<b>Verfügbar</b><br/>3 &amp; 4",
        )
            .to_variant();
        let werte = dict(&[
            ("Id", "webex".to_variant()),
            ("Category", "ApplicationStatus".to_variant()),
            ("Title", "Webex".to_variant()),
            ("Status", "NeedsAttention".to_variant()),
            ("IconName", "webex-tray".to_variant()),
            (
                "IconPixmap",
                vec![pixmap(16, 16), pixmap(3, 2)].to_variant(),
            ),
            ("AttentionIconName", 7i32.to_variant()),
            (
                "Menu",
                glib::Variant::parse(None, "objectpath '/MenuBar'").unwrap(),
            ),
            ("ItemIsMenu", true.to_variant()),
            ("ToolTip", tooltip),
            ("IconThemePath", "/tmp/symbole".to_variant()),
        ]);
        let e = Eintrag::from_properties(&werte).unwrap();
        assert_eq!(e.id, "webex");
        assert_eq!(e.status, Status::NeedsAttention);
        assert_eq!(e.icon_name, "webex-tray");
        assert_eq!(e.icon_pixmaps.len(), 2);
        assert_eq!(e.attention_name, "", "falscher Typ bleibt leer");
        assert_eq!(e.menu.as_deref(), Some("/MenuBar"));
        assert!(e.item_is_menu);
        assert_eq!(e.icon_theme_path, "/tmp/symbole");
        assert_eq!(e.tooltip_text(), "Webex\nVerfügbar\n3 & 4");
        assert!(Eintrag::from_properties(&7u32.to_variant()).is_none());
    }

    #[test]
    fn a_menu_path_of_slash_or_nothing_is_no_menu() {
        for wert in ["/", ""] {
            let werte = dict(&[(
                "Menu",
                glib::Variant::parse(
                    None,
                    &format!("objectpath '{}'", if wert.is_empty() { "/" } else { wert }),
                )
                .unwrap(),
            )]);
            assert_eq!(Eintrag::from_properties(&werte).unwrap().menu, None);
        }
        let werte = dict(&[("Menu", "".to_variant())]);
        assert_eq!(Eintrag::from_properties(&werte).unwrap().menu, None);
        let ohne = Eintrag::from_properties(&dict(&[])).unwrap();
        assert_eq!(ohne.menu, None);
        assert!(!ohne.item_is_menu);
        assert_eq!(ohne.status, Status::Active);
    }

    #[test]
    fn status_passive_hides_and_unknown_shows() {
        assert_eq!(Status::parse("Passive"), Status::Passive);
        assert_eq!(Status::parse("Active"), Status::Active);
        assert_eq!(Status::parse("NeedsAttention"), Status::NeedsAttention);
        assert_eq!(Status::parse("irgendwas"), Status::Active);
        let mut e = Eintrag::default();
        assert!(e.sichtbar());
        e.status = Status::Passive;
        assert!(!e.sichtbar());
    }

    #[test]
    fn bad_pixmaps_are_left_out() {
        let falsch = (4i32, 4i32, vec![0u8; 10]);
        let leer = (0i32, 0i32, Vec::<u8>::new());
        let riesig = (2000i32, 1i32, vec![0u8; 8000]);
        let gut = pixmap(2, 3);
        let wert = vec![falsch, leer, riesig, gut.clone()].to_variant();
        let p = pixmaps(&wert);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].width, p[0].height), (2, 3));
        assert!(pixmaps(&"kein Bild".to_variant()).is_empty());
    }

    #[test]
    fn the_best_pixmap_is_the_smallest_big_enough_else_the_biggest() {
        let p = |w| Pixmap {
            width: w,
            height: w,
            data: Vec::new(),
        };
        let list = [p(16), p(64), p(22), p(32)];
        assert_eq!(best_pixmap(&list, 20).unwrap().width, 22);
        assert_eq!(best_pixmap(&list, 24).unwrap().width, 32);
        assert_eq!(best_pixmap(&list, 16).unwrap().width, 16);
        assert_eq!(best_pixmap(&list, 100).unwrap().width, 64);
        assert!(best_pixmap(&[], 16).is_none());
    }

    #[test]
    fn argb_becomes_rgba() {
        let p = Pixmap {
            width: 1,
            height: 2,
            data: vec![0x80, 1, 2, 3, 0xff, 9, 8, 7],
        };
        assert_eq!(p.rgba(), [1, 2, 3, 0x80, 9, 8, 7, 0xff]);
    }

    #[test]
    fn icon_sources_name_first_then_pixmaps_attention_while_needed() {
        let px = Pixmap {
            width: 1,
            height: 1,
            data: vec![0; 4],
        };
        let mut e = Eintrag {
            icon_name: "normal".into(),
            icon_pixmaps: vec![px.clone()],
            attention_name: "achtung".into(),
            ..Eintrag::default()
        };
        assert_eq!(
            e.symbol(),
            [
                Quelle::Name("normal".into()),
                Quelle::Pixmaps(vec![px.clone()])
            ]
        );
        e.status = Status::NeedsAttention;
        assert_eq!(
            e.symbol(),
            [
                Quelle::Name("achtung".into()),
                Quelle::Name("normal".into()),
                Quelle::Pixmaps(vec![px.clone()])
            ]
        );
        // needs attention without an attention icon: the normal one
        e.attention_name.clear();
        assert_eq!(e.symbol()[0], Quelle::Name("normal".into()));
        // an absolute path is a file
        e.icon_name = "/opt/app/tray.png".into();
        assert_eq!(e.symbol()[0], Quelle::Datei("/opt/app/tray.png".into()));
        assert!(Eintrag::default().symbol().is_empty());
        e.overlay_name = "emblem".into();
        assert_eq!(e.overlay(), [Quelle::Name("emblem".into())]);
    }

    #[test]
    fn tooltip_falls_back_to_title_then_id() {
        let mut e = Eintrag {
            id: "app-id".into(),
            ..Eintrag::default()
        };
        assert_eq!(e.tooltip_text(), "app-id");
        e.title = " Titel ".into();
        assert_eq!(e.tooltip_text(), "Titel");
        e.tooltip.text = "nur Text".into();
        assert_eq!(e.tooltip_text(), "nur Text");
        e.tooltip.title = "Kopf".into();
        assert_eq!(e.tooltip_text(), "Kopf\nnur Text");
        assert!(tooltip(&"falsch".to_variant()).is_none());
    }

    #[test]
    fn markup_is_removed() {
        assert_eq!(
            ohne_markup("<b>fett</b> und <i>schräg</i>"),
            "fett und schräg"
        );
        assert_eq!(ohne_markup("a<br>b<BR/>c"), "a\nb\nc");
        assert_eq!(
            ohne_markup("1 &lt; 2 &amp;&amp; 3 &gt; 2"),
            "1 < 2 && 3 > 2"
        );
        assert_eq!(ohne_markup("&#65;&#x42;&quot;&apos;"), "AB\"'");
        assert_eq!(ohne_markup("Tom & Jerry; a < b"), "Tom & Jerry; a < b");
        assert_eq!(ohne_markup("&unbekannt;"), "&unbekannt;");
    }

    #[test]
    fn item_addresses_from_watcher_ids() {
        assert_eq!(
            adresse(":1.42/org/ayatana/NotificationItem/nm"),
            Some((":1.42".into(), "/org/ayatana/NotificationItem/nm".into()))
        );
        assert_eq!(
            adresse("org.kde.StatusNotifierItem-12-1/StatusNotifierItem"),
            Some((
                "org.kde.StatusNotifierItem-12-1".into(),
                "/StatusNotifierItem".into()
            ))
        );
        assert_eq!(
            adresse(":1.7"),
            Some((":1.7".into(), "/StatusNotifierItem".into()))
        );
        for falsch in ["", "/nur/pfad", "kein name/x", ":1.7/a//b", ":1.7/a/"] {
            assert_eq!(adresse(falsch), None, "{falsch:?}");
        }
    }
}

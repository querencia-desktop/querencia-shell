//! "Informationen über Querencia" in the "System" menu, the counterpart of
//! GNOME 2's "Über GNOME": a small window with logo, name, release, the
//! panel's own version, a short description, the link to the web site and
//! the license. An ordinary toplevel window, no libadwaita, no header bar;
//! labwc draws the title bar.
//!
//! The release ("Alpha 2.1") comes from the file the `querencia-desktop`
//! package installs, [`RELEASE_FILE`] in the format of os-release(5)
//! (`VERSION`, `HOME_URL`), else from [`VERSION_FILE`] (its first line).
//! Without the package (single packages, the development machine) neither
//! exists and the window names only the panel's version.

use std::path::Path;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation, Window, glib};

use crate::i18n::{gettext, gettext_f};
use crate::{add_dialog, links, logo, session};

/// Size of the logo at the top of the window.
const LOGO_SIZE: i32 = 64;
const PRODUCT: &str = "Querencia Desktop";
/// Installed by the `querencia-desktop` package; fixed paths, no override.
pub const RELEASE_FILE: &str = "/usr/share/querencia/release";
pub const VERSION_FILE: &str = "/usr/share/querencia/version";

/// What the window shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct About {
    pub product: String,
    /// The release of Querencia Desktop, `None` without the package.
    pub release: Option<String>,
    /// "Panel 0.2.2", from `Cargo.toml`.
    pub panel: String,
    pub description: String,
    pub home_url: String,
    pub license: String,
}

/// The release and web site from the package's files below `root` (`/` on
/// a running system): [`RELEASE_FILE`], else [`VERSION_FILE`], else none.
pub fn about_from(root: &Path) -> About {
    let read = |file: &str| std::fs::read_to_string(root.join(file.trim_start_matches('/'))).ok();
    let fields = read(RELEASE_FILE).map(|text| os_release(&text));
    let field = |key: &str| {
        fields
            .as_ref()
            .and_then(|fields| fields.iter().find(|(k, _)| k == key))
            .map(|(_, value)| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let release = match &fields {
        Some(_) => field("VERSION"),
        None => read(VERSION_FILE).and_then(|text| {
            text.lines()
                .next()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
        }),
    };
    // Only a web address; anything else would be handed to xdg-open.
    let home_url = field("HOME_URL")
        .filter(|url| url.starts_with("https://") && !url.contains(char::is_whitespace))
        .unwrap_or_else(|| links::HOME_URL.to_string());
    About {
        product: PRODUCT.to_string(),
        release,
        panel: gettext_f("Panel {version}", &[("version", env!("CARGO_PKG_VERSION"))]),
        description: gettext("A classic desktop in the style of GNOME 2 for Wayland and labwc"),
        home_url,
        license: gettext_f(
            "License: {license}",
            &[("license", env!("CARGO_PKG_LICENSE"))],
        ),
    }
}

/// [`about_from`] of this system.
pub fn about() -> About {
    about_from(Path::new("/"))
}

/// The `KEY=value` lines of an os-release(5) file, in order: empty lines
/// and comments skipped, values unquoted with the escapes `\"`, `\\`, `` \` ``
/// and `\$`; lines without `=` or with an invalid key are left out.
pub fn os_release(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .map(|(key, value)| (key.to_string(), unquote(value)))
        .collect()
}

fn unquote(value: &str) -> String {
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
    let Some(quote) = quote else {
        return value.to_string();
    };
    let inner = &value[1..];
    let inner = inner.strip_suffix(quote).unwrap_or(inner);
    if quote == '\'' {
        return inner.to_string();
    }
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(next @ ('"' | '\\' | '`' | '$')) => out.push(next),
                Some(next) => {
                    out.push('\\');
                    out.push(next);
                }
                None => out.push('\\'),
            },
            c => out.push(c),
        }
    }
    out
}

/// Raises the window in `slot` or opens a new one there.
pub fn present(slot: &glib::WeakRef<Window>) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = Window::builder()
        .title(gettext("About Querencia"))
        .resizable(false)
        .build();
    window.set_application(session::panel_application().as_ref());
    let about = about();

    // The official Querencia logo (C2), as large as the theme's
    // distribution logo `start-here` was here before.
    let logo = logo::image_of_size(LOGO_SIZE);
    let name = Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(&about.product)
        ))
        .use_markup(true)
        .build();
    let content = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(18)
        .margin_bottom(12)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&logo);
    content.append(&name);
    let text = |text: &str| {
        Label::builder()
            .label(text)
            .wrap(true)
            .max_width_chars(40)
            .justify(gtk4::Justification::Center)
            .build()
    };
    if let Some(release) = &about.release {
        content.append(&text(release));
    }
    content.append(&text(&about.panel));
    content.append(&text(&about.description));
    content.append(&link(&about.home_url));
    content.append(&text(&about.license));
    let close = add_dialog::stock_button(&gettext("_Close"), "window-close");
    close.set_halign(Align::End);
    close.set_margin_top(12);
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    content.append(&close);
    window.set_child(Some(&content));
    session::close_on_escape(&window);
    GtkWindowExt::set_focus(&window, Some(&close));
    slot.set(Some(&window));
    window.present();
}

/// The web site as a link that opens with `xdg-open`, labelled with the
/// address without its scheme ("querencialinux.org").
fn link(url: &str) -> Label {
    let shown = url.trim_start_matches("https://").trim_end_matches('/');
    let label = Label::builder()
        .label(format!(
            "<a href=\"{}\">{}</a>",
            glib::markup_escape_text(url),
            glib::markup_escape_text(shown)
        ))
        .use_markup(true)
        .build();
    label.connect_activate_link(|_, uri| {
        links::open(uri);
        glib::Propagation::Stop
    });
    label
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A scratch root with `files` (path below the root, contents).
    fn root(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("querencia-about-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let file = dir.join(path.trim_start_matches('/'));
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The file of Alpha 2.1.
    const ALPHA_21: &str = "NAME=\"Querencia Desktop\"\n\
        VERSION=\"Alpha 2.1\"\n\
        VERSION_ID=\"0.1.0~alpha2.1\"\n\
        HOME_URL=\"https://querencialinux.org\"\n\
        BUG_REPORT_URL=\"https://github.com/querencia-desktop/querencia/issues\"\n\
        DOCUMENTATION_URL=\"https://querencialinux.org/de/docs/\"\n";

    #[test]
    fn without_the_package_only_the_panel_version() {
        let dir = root("ohne", &[]);
        let about = about_from(&dir);
        assert_eq!(about.product, "Querencia Desktop");
        assert_eq!(about.release, None);
        assert_eq!(about.panel, format!("Panel {}", env!("CARGO_PKG_VERSION")));
        assert!(
            about.description.contains("GNOME 2"),
            "{}",
            about.description
        );
        assert_eq!(about.home_url, "https://querencialinux.org");
        // without a catalog in the test: the English msgid
        assert_eq!(about.license, "License: GPL-3.0-or-later");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_release_file_names_the_release() {
        let dir = root(
            "release",
            &[(RELEASE_FILE, ALPHA_21), (VERSION_FILE, "anders\n")],
        );
        let about = about_from(&dir);
        assert_eq!(about.release.as_deref(), Some("Alpha 2.1"));
        assert_eq!(about.panel, format!("Panel {}", env!("CARGO_PKG_VERSION")));
        assert_eq!(about.home_url, "https://querencialinux.org");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_release_the_first_line_of_version() {
        let dir = root("version", &[(VERSION_FILE, "  Alpha 2.1 \nzweite Zeile\n")]);
        assert_eq!(about_from(&dir).release.as_deref(), Some("Alpha 2.1"));
        let dir2 = root("version-leer", &[(VERSION_FILE, "\n")]);
        assert_eq!(about_from(&dir2).release, None);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(dir2);
    }

    #[test]
    fn a_release_file_without_version_shows_none() {
        // The release file wins even when it lacks VERSION: it is the
        // package's statement, the version file is only the fallback.
        let dir = root(
            "ohne-version",
            &[
                (RELEASE_FILE, "NAME=\"Querencia Desktop\"\n"),
                (VERSION_FILE, "Alpha 2.1\n"),
            ],
        );
        assert_eq!(about_from(&dir).release, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_a_web_address_becomes_the_link() {
        for (value, expected) in [
            (
                "\"https://querencialinux.org/\"",
                "https://querencialinux.org/",
            ),
            ("\"file:///etc/passwd\"", "https://querencialinux.org"),
            (
                "\"http://querencialinux.org\"",
                "https://querencialinux.org",
            ),
            ("\"https://a b\"", "https://querencialinux.org"),
            ("\"\"", "https://querencialinux.org"),
        ] {
            let dir = root("url", &[(RELEASE_FILE, &format!("HOME_URL={value}\n"))]);
            assert_eq!(about_from(&dir).home_url, expected, "{value}");
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn os_release_syntax() {
        let fields = os_release(
            "# Kommentar\n\n\
             NAME=\"Querencia Desktop\"\n\
             VERSION=Alpha\n\
             ESC=\"a\\\"b\\\\c\\`d\\$e\\nf\"\n\
             SINGLE='x y'\n\
             kein Gleichheitszeichen\n\
             BAD KEY=\"x\"\n\
             =leer\n\
             LEER=\n",
        );
        assert_eq!(
            fields,
            [
                ("NAME", "Querencia Desktop"),
                ("VERSION", "Alpha"),
                ("ESC", "a\"b\\c`d$e\\nf"),
                ("SINGLE", "x y"),
                ("LEER", ""),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string()))
        );
    }

    #[test]
    fn about_shows_no_development_paths() {
        let dir = root("pfade", &[(RELEASE_FILE, ALPHA_21)]);
        let about = about_from(&dir);
        for text in [
            &about.product,
            about.release.as_deref().unwrap_or_default(),
            &about.panel,
            &about.description,
            &about.home_url,
            &about.license,
        ] {
            assert!(!text.contains("/home"), "{text}");
            assert!(!text.contains("opt/"), "{text}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

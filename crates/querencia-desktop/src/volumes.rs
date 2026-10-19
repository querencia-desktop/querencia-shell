//! Mounted removable media on the desktop (QE-03), as on GNOME 2.
//!
//! Source is GIO's volume monitor. A mount gets an icon when it is a
//! physical volume the user can take away or unmount (`shown`): not
//! shadowed, with a GIO volume that is not of class `network`, rooted at a
//! local `file://` path outside the system tree, and either on a removable
//! drive or ejectable or unmountable by the user. Mounts without a volume
//! (GVfs network shares, the document portal, archives) and system mounts
//! never appear.
//!
//! Volume icons are never remembered: their places live only in memory
//! (`Places`), are forgotten when the volume goes away, and never reach
//! the state file `desktop-symbole`.

use std::path::{Path, PathBuf};

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::layout::Place;

/// What GIO tells about a mount, without GIO.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub name: String,
    pub root_uri: String,
    pub shadowed: bool,
    pub has_volume: bool,
    /// The volume's identifier `class` (`device`, `loop`, `network`).
    pub volume_class: Option<String>,
    pub unix_device: Option<String>,
    pub drive: Option<DriveFacts>,
    pub can_unmount: bool,
    pub can_eject: bool,
    /// The themed icon names of mount, volume and drive.
    pub icon_names: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriveFacts {
    pub removable: bool,
    pub media_removable: bool,
    pub can_eject: bool,
}

/// The picture of a volume icon (Querencia's own device icons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    Removable,
    Harddisk,
    Optical,
    Encrypted,
}

/// What the icon menu offers to take the volume away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    Eject,
    Unmount,
}

/// A volume icon: the mount by its root, and how it looks and goes away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub uri: String,
    pub picture: Picture,
    pub removal: Option<Removal>,
}

impl Volume {
    pub fn from_facts(facts: &Facts) -> Self {
        Self {
            uri: facts.root_uri.clone(),
            picture: picture(facts),
            removal: removal(facts),
        }
    }
}

/// System locations: never shown, whatever GIO says about them.
fn is_system_path(path: &Path) -> bool {
    const TREES: [&str; 11] = [
        "/boot", "/dev", "/efi", "/etc", "/opt", "/proc", "/snap", "/srv", "/sys", "/usr", "/var",
    ];
    if path.starts_with("/run") {
        return !path.starts_with("/run/media");
    }
    path == Path::new("/")
        || path == Path::new("/home")
        || path == Path::new("/root")
        || path == Path::new("/tmp")
        || TREES.iter().any(|tree| path.starts_with(tree))
}

fn local_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    glib::Uri::unescape_string(path, None::<&str>).map(|p| PathBuf::from(p.as_str()))
}

/// Whether the mount gets a desktop icon (see the module documentation).
pub fn shown(facts: &Facts) -> bool {
    if facts.shadowed || !facts.has_volume || facts.volume_class.as_deref() == Some("network") {
        return false;
    }
    let Some(path) = local_path(&facts.root_uri) else {
        return false;
    };
    if is_system_path(&path) {
        return false;
    }
    let removable_drive = facts
        .drive
        .is_some_and(|d| d.removable || d.media_removable || d.can_eject);
    removable_drive || facts.can_eject || facts.can_unmount
}

/// Encrypted (a device-mapper volume or GIO's lock emblem), optical,
/// removable (by icon name or drive), else a hard disk.
pub fn picture(facts: &Facts) -> Picture {
    let has = |part: &str| facts.icon_names.iter().any(|n| n.contains(part));
    let mapped = facts
        .unix_device
        .as_deref()
        .is_some_and(|d| d.starts_with("/dev/dm-"));
    if mapped || has("changes-prevent") || has("changes-allow") {
        Picture::Encrypted
    } else if has("optical") || has("cdrom") {
        Picture::Optical
    } else if [
        "removable",
        "usb",
        "flash",
        "media-memory",
        "floppy",
        "multimedia-player",
        "phone",
    ]
    .iter()
    .any(|part| has(part))
        || facts
            .drive
            .is_some_and(|d| d.removable || d.media_removable)
    {
        Picture::Removable
    } else {
        Picture::Harddisk
    }
}

/// "Auswerfen" when GIO can eject the mount, else "Aushängen" when it can
/// unmount it, else nothing.
pub fn removal(facts: &Facts) -> Option<Removal> {
    if facts.can_eject {
        Some(Removal::Eject)
    } else if facts.can_unmount {
        Some(Removal::Unmount)
    } else {
        None
    }
}

impl Picture {
    /// For the log.
    pub fn name(self) -> &'static str {
        match self {
            Picture::Removable => "Wechseldatenträger",
            Picture::Harddisk => "Festplatte",
            Picture::Optical => "optisches Medium",
            Picture::Encrypted => "verschlüsselt",
        }
    }
}

impl Removal {
    pub fn label(self) -> &'static str {
        match self {
            Removal::Eject => "Auswerfen",
            Removal::Unmount => "Aushängen",
        }
    }

    pub fn action(self) -> &'static str {
        match self {
            Removal::Eject => "desktop.eject",
            Removal::Unmount => "desktop.unmount",
        }
    }
}

/// The message for a failed unmount or eject: (title, detail), or `None`
/// when nothing is to be said (cancelled, or GIO's mount operation already
/// asked the user).
pub fn failure(
    kind: Option<gio::IOErrorEnum>,
    removal: Removal,
    name: &str,
    message: &str,
) -> Option<(String, String)> {
    let title = match removal {
        Removal::Eject => format!("„{name}“ konnte nicht ausgeworfen werden."),
        Removal::Unmount => format!("„{name}“ konnte nicht ausgehängt werden."),
    };
    let detail = match kind {
        Some(gio::IOErrorEnum::Cancelled | gio::IOErrorEnum::FailedHandled) => return None,
        Some(gio::IOErrorEnum::Busy) => "Der Datenträger wird noch benutzt. Schließen Sie alle \
             Programme und Fenster, die Dateien auf diesem Datenträger geöffnet haben, und \
             versuchen Sie es erneut."
            .to_owned(),
        Some(gio::IOErrorEnum::PermissionDenied) => {
            "Sie haben keine Berechtigung für diesen Datenträger.".to_owned()
        }
        Some(gio::IOErrorEnum::NotMounted | gio::IOErrorEnum::NotFound) => {
            "Der Datenträger ist nicht mehr eingehängt.".to_owned()
        }
        _ => message.to_owned(),
    };
    Some((title, detail))
}

/// Programs with their working directory or an open file below `root`,
/// as GVfs looks for them (only this user's are readable): (PID, name).
pub fn users_of(proc_dir: &Path, root: &Path) -> Vec<(u32, String)> {
    let Ok(entries) = std::fs::read_dir(proc_dir) else {
        return Vec::new();
    };
    let inside = |link: std::io::Result<PathBuf>| link.is_ok_and(|p| p.starts_with(root));
    let mut found: Vec<(u32, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<u32>().ok()?;
            let dir = entry.path();
            let uses = inside(std::fs::read_link(dir.join("cwd")))
                || std::fs::read_dir(dir.join("fd")).is_ok_and(|fds| {
                    fds.filter_map(Result::ok)
                        .any(|fd| inside(std::fs::read_link(fd.path())))
                });
            uses.then(|| {
                let name = std::fs::read_to_string(dir.join("comm")).unwrap_or_default();
                (pid, name.trim().to_owned())
            })
        })
        .collect();
    found.sort();
    found
}

/// GIO asks what to do while programs still use the volume: (message,
/// detail, buttons). Button 0 cancels, button 1 takes it away anyway.
pub fn busy_question(
    removal: Removal,
    name: &str,
    users: &[(u32, String)],
) -> (String, String, [&'static str; 2]) {
    let mut detail = if users.is_empty() {
        "Ein Programm hat noch Dateien auf dem Datenträger geöffnet.".to_owned()
    } else {
        let mut text =
            "Diese Programme haben noch Dateien auf dem Datenträger geöffnet:".to_owned();
        for (pid, program) in users.iter().take(5) {
            text.push_str(&format!("\n• {program} (Prozess {pid})"));
        }
        if users.len() > 5 {
            text.push_str(&format!("\n… und {} weitere", users.len() - 5));
        }
        text
    };
    let (verb, anyway) = match removal {
        Removal::Eject => ("werfen Sie ihn trotzdem aus", "Trotzdem auswerfen"),
        Removal::Unmount => ("hängen Sie ihn trotzdem aus", "Trotzdem aushängen"),
    };
    detail.push_str(&format!(
        "\n\nSchließen Sie die Programme, oder {verb}. Ungesicherte Daten \
         dieser Programme können dabei verloren gehen."
    ));
    (
        format!("Der Datenträger „{name}“ wird noch benutzt."),
        detail,
        ["Abbrechen", anyway],
    )
}

/// Places of volume icons for this session only, by icon key.
#[derive(Debug, Default)]
pub struct Places(Vec<(String, Place)>);

impl Places {
    pub fn get(&self, key: &str) -> Option<Place> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, p)| *p)
    }

    pub fn set(&mut self, key: &str, place: Place) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = place,
            None => self.0.push((key.to_owned(), place)),
        }
    }

    /// Forgets the places of volumes that are gone.
    pub fn keep_only(&mut self, present: &[&str]) {
        self.0.retain(|(k, _)| present.contains(&k.as_str()));
    }
}

// GIO side: reading the volume monitor.

fn icon_names(icon: Option<gio::Icon>, names: &mut Vec<String>) {
    let Some(icon) = icon else { return };
    if let Some(themed) = icon.downcast_ref::<gio::ThemedIcon>() {
        names.extend(themed.names().iter().map(|n| n.to_string()));
    } else if let Some(emblemed) = icon.downcast_ref::<gio::EmblemedIcon>() {
        icon_names(Some(emblemed.icon()), names);
        for emblem in emblemed.emblems() {
            icon_names(Some(emblem.icon()), names);
        }
    }
}

pub fn facts(mount: &gio::Mount) -> Facts {
    let volume = mount.volume();
    let drive = mount
        .drive()
        .or_else(|| volume.as_ref().and_then(|v| v.drive()));
    let mut names = Vec::new();
    icon_names(Some(mount.icon()), &mut names);
    icon_names(volume.as_ref().map(|v| v.icon()), &mut names);
    icon_names(drive.as_ref().map(|d| d.icon()), &mut names);
    Facts {
        name: mount.name().to_string(),
        root_uri: mount.root().uri().to_string(),
        shadowed: mount.is_shadowed(),
        has_volume: volume.is_some(),
        volume_class: volume
            .as_ref()
            .and_then(|v| v.identifier(gio::VOLUME_IDENTIFIER_KIND_CLASS))
            .map(|c| c.to_string()),
        unix_device: volume
            .as_ref()
            .and_then(|v| v.identifier(gio::VOLUME_IDENTIFIER_KIND_UNIX_DEVICE))
            .map(|d| d.to_string()),
        drive: drive.as_ref().map(|d| DriveFacts {
            removable: d.is_removable(),
            media_removable: d.is_media_removable(),
            can_eject: d.can_eject(),
        }),
        can_unmount: mount.can_unmount(),
        can_eject: mount.can_eject(),
        icon_names: names,
    }
}

/// The mounts that get an icon, by name.
pub fn mounted(monitor: &gio::VolumeMonitor) -> Vec<Facts> {
    let mut list: Vec<Facts> = monitor.mounts().iter().map(facts).filter(shown).collect();
    list.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.root_uri.cmp(&b.root_uri))
    });
    list
}

pub fn find_mount(monitor: &gio::VolumeMonitor, uri: &str) -> Option<gio::Mount> {
    monitor.mounts().into_iter().find(|m| m.root().uri() == uri)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Anchor, Axis};

    fn usb_stick() -> Facts {
        Facts {
            name: "QDTEST".into(),
            root_uri: "file:///run/media/u/QDTEST".into(),
            has_volume: true,
            volume_class: Some("device".into()),
            unix_device: Some("/dev/sdb1".into()),
            drive: Some(DriveFacts {
                removable: true,
                media_removable: false,
                can_eject: true,
            }),
            can_unmount: true,
            can_eject: true,
            icon_names: vec!["drive-removable-media-usb".into()],
            ..Facts::default()
        }
    }

    /// A loop image set up by the user: a volume without a drive.
    fn loop_image() -> Facts {
        Facts {
            name: "QDLOOP".into(),
            root_uri: "file:///run/media/u/QDLOOP".into(),
            has_volume: true,
            volume_class: Some("loop".into()),
            unix_device: Some("/dev/loop8".into()),
            can_unmount: true,
            icon_names: vec!["drive-removable-media".into()],
            ..Facts::default()
        }
    }

    #[test]
    fn removable_and_user_unmountable_volumes_are_shown() {
        assert!(shown(&usb_stick()));
        assert!(shown(&loop_image()));
        let mut card = usb_stick();
        card.can_eject = false;
        card.can_unmount = false;
        assert!(shown(&card), "a removable drive alone is enough");
        let mut internal = loop_image();
        internal.volume_class = Some("device".into());
        internal.root_uri = "file:///media/u/Daten".into();
        assert!(shown(&internal), "a disk the user mounted and may unmount");
        internal.can_unmount = false;
        assert!(!shown(&internal), "neither removable nor unmountable");
        let mut escaped = usb_stick();
        escaped.root_uri = "file:///run/media/u/Mein%20Stick".into();
        assert!(shown(&escaped));
    }

    #[test]
    fn network_portal_shadowed_and_system_mounts_are_not_shown() {
        let mut share = loop_image();
        share.volume_class = Some("network".into());
        assert!(!shown(&share));
        let mut sftp = usb_stick();
        sftp.root_uri = "sftp://server/home".into();
        assert!(!shown(&sftp));
        let mut portal = loop_image();
        portal.has_volume = false;
        portal.root_uri = "file:///run/user/1001/doc".into();
        assert!(!shown(&portal));
        let mut fuse = usb_stick();
        fuse.root_uri = "file:///run/user/1001/gvfs/smb-share:server=s,share=x".into();
        assert!(!shown(&fuse));
        let mut shadowed = usb_stick();
        shadowed.shadowed = true;
        assert!(!shown(&shadowed));
        for root in [
            "file:///",
            "file:///boot/efi",
            "file:///home",
            "file:///var/lib/x",
        ] {
            let mut system = usb_stick();
            system.root_uri = root.into();
            assert!(!shown(&system), "{root}");
        }
        let mut in_home = loop_image();
        in_home.root_uri = "file:///home/u/mnt/karte".into();
        assert!(
            shown(&in_home),
            "a mount below the home folder is the user's"
        );
    }

    #[test]
    fn the_picture_follows_device_and_icon_names() {
        assert_eq!(picture(&usb_stick()), Picture::Removable);
        assert_eq!(picture(&loop_image()), Picture::Removable);
        let with = |names: &[&str], removable: bool| {
            let mut f = loop_image();
            f.icon_names = names.iter().map(|n| (*n).to_owned()).collect();
            f.drive = Some(DriveFacts {
                removable,
                ..DriveFacts::default()
            });
            picture(&f)
        };
        assert_eq!(
            with(&["media-optical-cd-rom", "drive-optical"], true),
            Picture::Optical
        );
        assert_eq!(with(&["drive-harddisk-usb"], true), Picture::Removable);
        assert_eq!(with(&["media-flash-sd"], false), Picture::Removable);
        assert_eq!(with(&["drive-harddisk"], false), Picture::Harddisk);
        assert_eq!(with(&["drive-harddisk"], true), Picture::Removable);
        assert_eq!(
            with(&["drive-removable-media", "changes-prevent-symbolic"], true),
            Picture::Encrypted
        );
        let mut luks = usb_stick();
        luks.unix_device = Some("/dev/dm-0".into());
        assert_eq!(picture(&luks), Picture::Encrypted);
    }

    #[test]
    fn eject_wins_over_unmount() {
        assert_eq!(removal(&usb_stick()), Some(Removal::Eject));
        assert_eq!(removal(&loop_image()), Some(Removal::Unmount));
        let mut fixed = loop_image();
        fixed.can_unmount = false;
        assert_eq!(removal(&fixed), None);
        assert_eq!(Removal::Eject.label(), "Auswerfen");
        assert_eq!(Removal::Unmount.action(), "desktop.unmount");
        assert_eq!(
            Volume::from_facts(&usb_stick()),
            Volume {
                uri: "file:///run/media/u/QDTEST".into(),
                picture: Picture::Removable,
                removal: Some(Removal::Eject),
            }
        );
    }

    #[test]
    fn failures_are_reported_unless_cancelled_or_handled() {
        let (title, detail) = failure(
            Some(gio::IOErrorEnum::Busy),
            Removal::Unmount,
            "QDTEST",
            "target is busy",
        )
        .expect("busy is reported");
        assert_eq!(title, "„QDTEST“ konnte nicht ausgehängt werden.");
        assert!(detail.starts_with("Der Datenträger wird noch benutzt."));
        let (title, detail) = failure(
            Some(gio::IOErrorEnum::Failed),
            Removal::Eject,
            "cidata",
            "kaputt",
        )
        .expect("x");
        assert_eq!(title, "„cidata“ konnte nicht ausgeworfen werden.");
        assert_eq!(detail, "kaputt");
        assert_eq!(
            failure(
                Some(gio::IOErrorEnum::FailedHandled),
                Removal::Unmount,
                "x",
                ""
            ),
            None
        );
        assert_eq!(
            failure(Some(gio::IOErrorEnum::Cancelled), Removal::Eject, "x", ""),
            None
        );
        assert!(failure(None, Removal::Unmount, "x", "D-Bus").is_some());
    }

    #[test]
    fn programs_using_a_volume_are_found_by_cwd_and_open_files() {
        let base = std::env::temp_dir().join(format!("qe03-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (proc_dir, media) = (base.join("proc"), base.join("media/QDTEST"));
        std::fs::create_dir_all(media.join("Ordner")).expect("fixture");
        let process = |pid: &str, name: &str, cwd: &Path, files: &[PathBuf]| {
            let dir = proc_dir.join(pid);
            std::fs::create_dir_all(dir.join("fd")).expect("fixture");
            std::fs::write(dir.join("comm"), format!("{name}\n")).expect("fixture");
            std::os::unix::fs::symlink(cwd, dir.join("cwd")).expect("fixture");
            for (n, file) in files.iter().enumerate() {
                std::os::unix::fs::symlink(file, dir.join("fd").join(n.to_string()))
                    .expect("fixture");
            }
        };
        process("40", "sleep", &media.join("Ordner"), &[]);
        process(
            "7",
            "editor",
            &base,
            &[base.join("x"), media.join("Brief.txt")],
        );
        process("9", "anderes", &base, &[base.join("media/QDTESTCHEN/a")]);
        std::fs::create_dir_all(proc_dir.join("self")).expect("fixture");
        assert_eq!(
            users_of(&proc_dir, &media),
            [(7, "editor".to_owned()), (40, "sleep".to_owned())]
        );
        assert!(users_of(&base.join("fehlt"), &media).is_empty());
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn the_busy_question_names_the_programs() {
        let (message, detail, buttons) =
            busy_question(Removal::Unmount, "QDTEST", &[(40, "sleep".to_owned())]);
        assert_eq!(message, "Der Datenträger „QDTEST“ wird noch benutzt.");
        assert!(detail.contains("• sleep (Prozess 40)"), "{detail}");
        assert!(detail.contains("hängen Sie ihn trotzdem aus"));
        assert_eq!(buttons, ["Abbrechen", "Trotzdem aushängen"]);
        let many: Vec<(u32, String)> = (1..=7).map(|n| (n, format!("p{n}"))).collect();
        let (_, detail, buttons) = busy_question(Removal::Eject, "cidata", &many);
        assert!(
            detail.contains("p5") && !detail.contains("p6") && detail.contains("… und 2 weitere")
        );
        assert_eq!(buttons[1], "Trotzdem auswerfen");
        let (_, detail, _) = busy_question(Removal::Unmount, "x", &[]);
        assert!(detail.starts_with("Ein Programm hat noch Dateien"));
    }

    #[test]
    fn session_places_are_forgotten_with_their_volume() {
        let place = |offset| Place {
            x: Axis {
                anchor: Anchor::End,
                offset,
            },
            y: Axis {
                anchor: Anchor::Start,
                offset: 8,
            },
        };
        let mut places = Places::default();
        places.set("laufwerk:a", place(8));
        places.set("laufwerk:b", place(20));
        places.set("laufwerk:a", place(30));
        assert_eq!(places.get("laufwerk:a"), Some(place(30)));
        places.keep_only(&["laufwerk:b"]);
        assert_eq!(places.get("laufwerk:a"), None);
        assert_eq!(places.get("laufwerk:b"), Some(place(20)));
    }
}

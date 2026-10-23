//! Volumes, as GNOME 2 showed and handled them (Desktop QE-03, QE-04).
//!
//! Source is GIO's volume monitor. [`classify`] sorts every mount into one
//! of four types (in this order): network shares (`smb:`, `sftp:` … – GVfs
//! mounts without a GIO volume, too), optical media, removable volumes (a
//! removable or ejectable drive, phones and cameras) and internal volumes
//! the user mounted and may unmount. Shadowed mounts, system paths, the
//! document portal, GVfs' FUSE directories, archives and other virtual
//! places never get a type. Which types a program shows is its own choice
//! (the desktop: `desktop.ini` `[Volumes]`).
//!
//! New against the desktop: [`alle`] lists volumes that are not mounted yet,
//! [`einhaengen`] mounts them, [`entfernen`] unmounts or ejects. Questions
//! (password, "still in use") go through a [`Rueckfrage`]; the GTK side
//! (`gtk::MountOperation`) belongs to the programs, not to this crate.

use std::path::{Path, PathBuf};

use gio::prelude::*;

use crate::i18n::{gettext, gettext_f, ngettext_f};

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
    /// A volume that is not mounted: whether GIO can mount it (`root_uri`
    /// is then empty).
    pub can_mount: bool,
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
    /// QE-04: a network share (the theme's `folder-remote`).
    Network,
}

/// What the icon menu offers to take the volume away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    Eject,
    Unmount,
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
    glib::Uri::unescape_string(path, None::<&str>).map(|p: glib::GString| PathBuf::from(p.as_str()))
}

/// QE-04: the four types of `desktop.ini` `[Volumes]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeType {
    Removable,
    Optical,
    Network,
    Internal,
}

impl VolumeType {
    /// For the log, with the key of `desktop.ini`.
    pub fn name(self) -> &'static str {
        match self {
            VolumeType::Removable => "Wechseldatenträger (Removable)",
            VolumeType::Optical => "optisches Medium (Optical)",
            VolumeType::Network => "Netzlaufwerk (Network)",
            VolumeType::Internal => "eingebautes Laufwerk (Internal)",
        }
    }
}

/// Network shares by their URI scheme (GVfs backends).
const NETWORK_SCHEMES: [&str; 9] = [
    "smb", "sftp", "ssh", "ftp", "ftps", "dav", "davs", "nfs", "afp",
];
/// Phones, cameras and players GVfs mounts without a local path.
const DEVICE_SCHEMES: [&str; 3] = ["mtp", "gphoto2", "afc"];

fn scheme(uri: &str) -> &str {
    uri.split_once(':').map_or("", |(scheme, _)| scheme)
}

fn is_network(facts: &Facts) -> bool {
    NETWORK_SCHEMES.contains(&scheme(&facts.root_uri))
        || facts.volume_class.as_deref() == Some("network")
}

/// QE-04: the type of a mount, `None` for mounts that never get an icon
/// (rules in this order: network, optical, removable, internal).
pub fn classify(facts: &Facts) -> Option<VolumeType> {
    if facts.shadowed {
        return None;
    }
    if is_network(facts) {
        return Some(VolumeType::Network);
    }
    if DEVICE_SCHEMES.contains(&scheme(&facts.root_uri)) {
        return Some(VolumeType::Removable);
    }
    // archives, trash:, recent:, burn:, computer:, network:, admin: …
    let path = local_path(&facts.root_uri)?;
    // the document portal and GVfs' FUSE directory have no volume and lie
    // below /run/user; neither do other FUSE mounts of programs
    if is_system_path(&path) || !facts.has_volume {
        return None;
    }
    let picture = picture(facts);
    if picture == Picture::Optical {
        return Some(VolumeType::Optical);
    }
    let removable_drive = facts
        .drive
        .is_some_and(|d| d.removable || d.media_removable || d.can_eject);
    // QE-04a: an encrypted volume counts by its device, and that includes
    // the icon names of a drive-less device (a LUKS loop image is removable
    // like the same image without encryption)
    if removable_drive || facts.can_eject || removable_by_name(facts) {
        return Some(VolumeType::Removable);
    }
    facts.can_unmount.then_some(VolumeType::Internal)
}

/// The type of a volume that is not mounted yet (it has no root to look
/// at): network, optical, removable as for mounts; otherwise internal when
/// GIO may mount it. `None` for volumes nobody can mount.
pub fn classify_unmounted(facts: &Facts) -> Option<VolumeType> {
    if is_network(facts) {
        return Some(VolumeType::Network);
    }
    if !facts.can_mount {
        return None;
    }
    if picture(facts) == Picture::Optical {
        return Some(VolumeType::Optical);
    }
    let removable_drive = facts
        .drive
        .is_some_and(|d| d.removable || d.media_removable || d.can_eject);
    if removable_drive || facts.can_eject || removable_by_name(facts) {
        return Some(VolumeType::Removable);
    }
    Some(VolumeType::Internal)
}

/// GIO's icon names say the medium can be taken away (stick, card,
/// player, phone, removable drive).
fn removable_by_name(facts: &Facts) -> bool {
    const PARTS: [&str; 7] = [
        "removable",
        "usb",
        "flash",
        "media-memory",
        "floppy",
        "multimedia-player",
        "phone",
    ];
    facts
        .icon_names
        .iter()
        .any(|name| PARTS.iter().any(|part| name.contains(part)))
}

/// A network share (QE-04), encrypted (a device-mapper volume or GIO's
/// lock emblem), optical, removable (by icon name or drive), else a hard disk.
pub fn picture(facts: &Facts) -> Picture {
    if is_network(facts) {
        return Picture::Network;
    }
    let has = |part: &str| facts.icon_names.iter().any(|n| n.contains(part));
    let mapped = facts
        .unix_device
        .as_deref()
        .is_some_and(|d| d.starts_with("/dev/dm-"));
    if mapped || has("changes-prevent") || has("changes-allow") {
        Picture::Encrypted
    } else if has("optical") || has("cdrom") {
        Picture::Optical
    } else if removable_by_name(facts)
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
            Picture::Network => "Netzlaufwerk",
        }
    }
}

impl Removal {
    /// For the log.
    pub fn label(self) -> &'static str {
        match self {
            Removal::Eject => "Auswerfen",
            Removal::Unmount => "Aushängen",
        }
    }

    /// The entry of the icon menu.
    pub fn menu_label(self) -> String {
        match self {
            Removal::Eject => gettext("Eject"),
            Removal::Unmount => gettext("Unmount"),
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
        Removal::Eject => gettext_f("“{name}” could not be ejected.", &[("name", name)]),
        Removal::Unmount => gettext_f("“{name}” could not be unmounted.", &[("name", name)]),
    };
    let detail = match kind {
        Some(gio::IOErrorEnum::Cancelled | gio::IOErrorEnum::FailedHandled) => return None,
        Some(gio::IOErrorEnum::Busy) => gettext(
            "The volume is still in use. Close all programs and windows that have files \
             on this volume open, and try again.",
        ),
        Some(gio::IOErrorEnum::PermissionDenied) => {
            gettext("You do not have the permissions for this volume.")
        }
        Some(gio::IOErrorEnum::NotMounted | gio::IOErrorEnum::NotFound) => {
            gettext("The volume is no longer mounted.")
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
) -> (String, String, [String; 2]) {
    let mut detail = if users.is_empty() {
        gettext("A program still has files on the volume open.")
    } else {
        let mut text = gettext("These programs still have files on the volume open:");
        for (pid, program) in users.iter().take(5) {
            text.push('\n');
            text.push_str(&gettext_f(
                "• {program} (process {pid})",
                &[("program", program), ("pid", &pid.to_string())],
            ));
        }
        if users.len() > 5 {
            let more = u32::try_from(users.len() - 5).unwrap_or(u32::MAX);
            text.push('\n');
            text.push_str(&ngettext_f("… and {n} more", "… and {n} more", more, &[]));
        }
        text
    };
    let (advice, anyway) = match removal {
        Removal::Eject => (
            gettext(
                "Close the programs, or eject the volume anyway. Unsaved data of these \
                 programs may be lost.",
            ),
            gettext("Eject Anyway"),
        ),
        Removal::Unmount => (
            gettext(
                "Close the programs, or unmount the volume anyway. Unsaved data of these \
                 programs may be lost.",
            ),
            gettext("Unmount Anyway"),
        ),
    };
    detail.push_str("\n\n");
    detail.push_str(&advice);
    (
        gettext_f("The volume “{name}” is still in use.", &[("name", name)]),
        detail,
        [gettext("Cancel"), anyway],
    )
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
        can_mount: false,
    }
}

pub fn find_mount(monitor: &gio::VolumeMonitor, uri: &str) -> Option<gio::Mount> {
    monitor.mounts().into_iter().find(|m| m.root().uri() == uri)
}

/// QE-03: Querencia's own device icons, built in (no icon theme has them);
/// QE-04: network shares with the theme's remote folder, as Nautilus 2.30.
pub fn volume_icon(picture: Picture) -> gio::Icon {
    let svg: &'static [u8] = match picture {
        Picture::Network => {
            return gio::ThemedIcon::from_names(&["folder-remote", "network-server", "folder"])
                .upcast();
        }
        Picture::Removable => include_bytes!("../data/icons/strata-querencia-drive-removable.svg"),
        Picture::Harddisk => include_bytes!("../data/icons/strata-querencia-drive-harddisk.svg"),
        Picture::Optical => include_bytes!("../data/icons/strata-querencia-media-optical.svg"),
        Picture::Encrypted => include_bytes!("../data/icons/strata-querencia-drive-encrypted.svg"),
    };
    gio::BytesIcon::new(&glib::Bytes::from_static(svg)).upcast()
}

/// What GIO tells about a volume that is not mounted.
pub fn volume_facts(volume: &gio::Volume) -> Facts {
    let drive = volume.drive();
    let mut names = Vec::new();
    icon_names(Some(volume.icon()), &mut names);
    icon_names(drive.as_ref().map(|d| d.icon()), &mut names);
    Facts {
        name: volume.name().to_string(),
        root_uri: String::new(),
        has_volume: true,
        volume_class: volume
            .identifier(gio::VOLUME_IDENTIFIER_KIND_CLASS)
            .map(|c| c.to_string()),
        unix_device: volume
            .identifier(gio::VOLUME_IDENTIFIER_KIND_UNIX_DEVICE)
            .map(|d| d.to_string()),
        drive: drive.as_ref().map(|d| DriveFacts {
            removable: d.is_removable(),
            media_removable: d.is_media_removable(),
            can_eject: d.can_eject(),
        }),
        can_eject: volume.can_eject(),
        can_mount: volume.can_mount(),
        icon_names: names,
        ..Facts::default()
    }
}

/// The GIO object behind a [`Laufwerk`].
#[derive(Clone, Debug)]
pub enum Quelle {
    /// Mounted: unmount or eject it, open its root.
    Mount(gio::Mount),
    /// Not mounted: mount it first ([`einhaengen`]).
    Volume(gio::Volume),
}

/// One volume for a side bar, the places menu or "Rechner".
#[derive(Clone, Debug)]
pub struct Laufwerk {
    pub name: String,
    /// The root of a mounted volume; `None` before it is mounted.
    pub uri: Option<String>,
    pub typ: VolumeType,
    pub picture: Picture,
    pub mounted: bool,
    /// How a mounted volume goes away; `None` when it cannot.
    pub removal: Option<Removal>,
    pub icon: gio::Icon,
    pub quelle: Quelle,
}

/// Every volume a person would see, mounted or not, by name: mounts with a
/// type ([`classify`]), and volumes GIO can mount that are not mounted yet
/// ([`classify_unmounted`]). A mounted volume appears once (as its mount).
pub fn alle(monitor: &gio::VolumeMonitor) -> Vec<Laufwerk> {
    let mut list: Vec<Laufwerk> = monitor
        .mounts()
        .into_iter()
        .filter_map(|mount| {
            let facts = facts(&mount);
            let typ = classify(&facts)?;
            let picture = picture(&facts);
            Some(Laufwerk {
                name: facts.name.clone(),
                uri: Some(facts.root_uri.clone()),
                typ,
                picture,
                mounted: true,
                removal: removal(&facts),
                icon: volume_icon(picture),
                quelle: Quelle::Mount(mount),
            })
        })
        .collect();
    list.extend(
        monitor
            .volumes()
            .into_iter()
            .filter(|volume| volume.get_mount().is_none())
            .filter_map(|volume| {
                let facts = volume_facts(&volume);
                let typ = classify_unmounted(&facts)?;
                let picture = picture(&facts);
                Some(Laufwerk {
                    name: facts.name.clone(),
                    uri: None,
                    typ,
                    picture,
                    mounted: false,
                    removal: None,
                    icon: volume_icon(picture),
                    quelle: Quelle::Volume(volume),
                })
            }),
    );
    list.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.uri.cmp(&b.uri))
            .then_with(|| b.mounted.cmp(&a.mounted))
    });
    list
}

/// Where questions while mounting or taking a volume away go: passwords,
/// "still in use", which choice. Programs with a display give GTK's
/// `gtk::MountOperation` (its own dialogs) with their handlers attached
/// (the desktop's German "still in use" question, [`busy_question`]);
/// without one, nothing can be asked.
pub trait Rueckfrage {
    fn mount_operation(&self) -> Option<gio::MountOperation>;
}

/// No questions: GIO gets no mount operation and fails where it would ask.
pub struct OhneRueckfrage;

impl Rueckfrage for OhneRueckfrage {
    fn mount_operation(&self) -> Option<gio::MountOperation> {
        None
    }
}

/// Mounts a volume that is not mounted (new against the desktop, which only
/// showed mounted ones) and gives its mount.
pub async fn einhaengen(
    volume: &gio::Volume,
    rueckfrage: &dyn Rueckfrage,
) -> Result<gio::Mount, glib::Error> {
    let operation = rueckfrage.mount_operation();
    volume
        .mount_future(gio::MountMountFlags::NONE, operation.as_ref())
        .await?;
    volume.get_mount().ok_or_else(|| {
        glib::Error::new(
            gio::IOErrorEnum::NotMounted,
            &gettext("The volume was mounted, but it has no mount point."),
        )
    })
}

/// "Aushängen" or "Auswerfen" through GIO.
pub async fn entfernen(
    mount: &gio::Mount,
    how: Removal,
    rueckfrage: &dyn Rueckfrage,
) -> Result<(), glib::Error> {
    let operation = rueckfrage.mount_operation();
    match how {
        Removal::Eject => {
            mount
                .eject_with_operation_future(gio::MountUnmountFlags::NONE, operation.as_ref())
                .await
        }
        Removal::Unmount => {
            mount
                .unmount_with_operation_future(gio::MountUnmountFlags::NONE, operation.as_ref())
                .await
        }
    }
}

/// The message for a failed mount: (title, detail), or `None` when nothing
/// is to be said (cancelled, or the mount operation already asked).
pub fn mount_failure(
    kind: Option<gio::IOErrorEnum>,
    name: &str,
    message: &str,
) -> Option<(String, String)> {
    let title = gettext_f("“{name}” could not be mounted.", &[("name", name)]);
    let detail = match kind {
        Some(gio::IOErrorEnum::Cancelled | gio::IOErrorEnum::FailedHandled) => return None,
        Some(gio::IOErrorEnum::PermissionDenied) => {
            gettext("You do not have the permissions for this volume.")
        }
        Some(gio::IOErrorEnum::AlreadyMounted) => gettext("The volume is already mounted."),
        _ => message.to_owned(),
    };
    Some((title, detail))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The desktop with every type switched on (its `desktop.ini`).
    fn shown(facts: &Facts) -> bool {
        classify(facts).is_some()
    }

    /// QE-03's rules still hold with every type switched on.
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
        internal.icon_names = vec!["drive-harddisk".into()];
        assert!(shown(&internal), "a disk the user mounted and may unmount");
        internal.can_unmount = false;
        assert!(!shown(&internal), "neither removable nor unmountable");
        let mut escaped = usb_stick();
        escaped.root_uri = "file:///run/media/u/Mein%20Stick".into();
        assert!(shown(&escaped));
    }

    #[test]
    fn portal_shadowed_virtual_and_system_mounts_are_never_shown() {
        let mut portal = loop_image();
        portal.has_volume = false;
        portal.root_uri = "file:///run/user/1001/doc".into();
        assert_eq!(classify(&portal), None);
        let mut fuse = usb_stick();
        fuse.root_uri = "file:///run/user/1001/gvfs/smb-share:server=s,share=x".into();
        assert_eq!(classify(&fuse), None);
        let mut shadowed = usb_stick();
        shadowed.shadowed = true;
        assert_eq!(classify(&shadowed), None);
        let mut shadowed_share = nas_test();
        shadowed_share.shadowed = true;
        assert_eq!(classify(&shadowed_share), None);
        for root in [
            "archive://x",
            "trash:///",
            "recent:///",
            "burn:///",
            "computer:///",
        ] {
            let mut f = loop_image();
            f.root_uri = root.into();
            assert_eq!(classify(&f), None, "{root}");
        }
        let mut fuse_in_home = loop_image();
        fuse_in_home.has_volume = false;
        fuse_in_home.root_uri = "file:///home/u/projekt/gvfs".into();
        assert_eq!(classify(&fuse_in_home), None, "FUSE mounts of programs");
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

    /// The mounts of the VM entwicklungs-vm on 19.10.2026 (`gio mount -li`).
    fn cidata() -> Facts {
        Facts {
            name: "cidata".into(),
            root_uri: "file:///run/media/user/cidata".into(),
            has_volume: true,
            volume_class: Some("device".into()),
            unix_device: Some("/dev/sr0".into()),
            drive: Some(DriveFacts {
                removable: true,
                media_removable: true,
                can_eject: true,
            }),
            can_unmount: true,
            can_eject: true,
            icon_names: vec![
                "media-optical-cd-rom".into(),
                "media-optical".into(),
                "drive-optical".into(),
            ],
            ..Facts::default()
        }
    }

    fn nas_test() -> Facts {
        Facts {
            name: "user on nas-test.local".into(),
            root_uri: "smb://nas-test.local/user/".into(),
            can_unmount: true,
            icon_names: vec!["folder-remote".into(), "folder".into()],
            ..Facts::default()
        }
    }

    #[test]
    fn the_four_types() {
        assert_eq!(classify(&usb_stick()), Some(VolumeType::Removable));
        assert_eq!(classify(&loop_image()), Some(VolumeType::Removable));
        assert_eq!(classify(&cidata()), Some(VolumeType::Optical));
        assert_eq!(classify(&nas_test()), Some(VolumeType::Network));
        let mut sftp = nas_test();
        sftp.root_uri = "sftp://server/home".into();
        assert_eq!(classify(&sftp), Some(VolumeType::Network));
        let mut nfs_volume = loop_image();
        nfs_volume.volume_class = Some("network".into());
        assert_eq!(classify(&nfs_volume), Some(VolumeType::Network));
        let mut phone = nas_test();
        phone.root_uri = "mtp://Telefon/".into();
        assert_eq!(classify(&phone), Some(VolumeType::Removable));
        let mut disk = loop_image();
        disk.volume_class = Some("device".into());
        disk.unix_device = Some("/dev/sda3".into());
        disk.drive = Some(DriveFacts::default());
        disk.icon_names = vec!["drive-harddisk".into()];
        disk.root_uri = "file:///run/media/u/Daten".into();
        assert_eq!(classify(&disk), Some(VolumeType::Internal));
        // encrypted: by its drive
        let mut luks_disk = disk.clone();
        luks_disk.unix_device = Some("/dev/dm-0".into());
        assert_eq!(classify(&luks_disk), Some(VolumeType::Internal));
        let mut luks_stick = usb_stick();
        luks_stick.unix_device = Some("/dev/dm-1".into());
        assert_eq!(classify(&luks_stick), Some(VolumeType::Removable));
        assert_eq!(picture(&nas_test()), Picture::Network);
        assert_eq!(removal(&nas_test()), Some(Removal::Unmount));
    }

    /// QE-04a (G2-X, QE-03 D13 with QE-04): an unlocked LUKS loop image has
    /// no drive and the picture "encrypted"; its icon names make it removable,
    /// as the same image without encryption, and it shows with the defaults.
    #[test]
    fn an_encrypted_loop_image_is_removable_like_a_plain_one() {
        let mut geheim = loop_image();
        geheim.name = "QDGEHEIM".into();
        geheim.root_uri = "file:///run/media/u/QDGEHEIM".into();
        geheim.unix_device = Some("/dev/dm-0".into());
        geheim.volume_class = Some("device".into());
        geheim.icon_names = vec![
            "drive-removable-media".into(),
            "changes-allow-symbolic".into(),
        ];
        assert_eq!(picture(&geheim), Picture::Encrypted);
        assert_eq!(classify(&geheim), Some(VolumeType::Removable));
        assert!(shown(&geheim));
        // an internal encrypted partition stays internal (hidden by default)
        let mut intern = geheim.clone();
        intern.icon_names = vec!["drive-harddisk".into(), "changes-allow-symbolic".into()];
        intern.drive = Some(DriveFacts::default());
        assert_eq!(classify(&intern), Some(VolumeType::Internal));
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
        assert_eq!(title, "“QDTEST” could not be unmounted.");
        assert!(detail.starts_with("The volume is still in use."));
        let (title, detail) = failure(
            Some(gio::IOErrorEnum::Failed),
            Removal::Eject,
            "cidata",
            "kaputt",
        )
        .expect("x");
        assert_eq!(title, "“cidata” could not be ejected.");
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
        assert_eq!(message, "The volume “QDTEST” is still in use.");
        assert!(detail.contains("• sleep (process 40)"), "{detail}");
        assert!(detail.contains("unmount the volume anyway"));
        assert_eq!(buttons, ["Cancel", "Unmount Anyway"]);
        let many: Vec<(u32, String)> = (1..=7).map(|n| (n, format!("p{n}"))).collect();
        let (_, detail, buttons) = busy_question(Removal::Eject, "cidata", &many);
        assert!(detail.contains("p5") && !detail.contains("p6") && detail.contains("… and 2 more"));
        assert_eq!(buttons[1], "Eject Anyway");
        let (_, detail, _) = busy_question(Removal::Unmount, "x", &[]);
        assert!(detail.starts_with("A program still has files"));
    }

    #[test]
    fn unmounted_volumes_get_a_type_when_they_can_be_mounted() {
        let mut stick = usb_stick();
        stick.root_uri = String::new();
        stick.can_mount = true;
        assert_eq!(classify_unmounted(&stick), Some(VolumeType::Removable));
        let mut disk = loop_image();
        disk.root_uri = String::new();
        disk.icon_names = vec!["drive-harddisk".into()];
        disk.can_mount = true;
        assert_eq!(classify_unmounted(&disk), Some(VolumeType::Internal));
        disk.can_mount = false;
        assert_eq!(classify_unmounted(&disk), None);
        let mut cd = cidata();
        cd.root_uri = String::new();
        cd.can_mount = true;
        assert_eq!(classify_unmounted(&cd), Some(VolumeType::Optical));
        let (title, detail) =
            mount_failure(Some(gio::IOErrorEnum::PermissionDenied), "QDTEST", "x").expect("x");
        assert_eq!(title, "“QDTEST” could not be mounted.");
        assert!(detail.starts_with("You do not have"));
        assert_eq!(
            mount_failure(Some(gio::IOErrorEnum::Cancelled), "x", ""),
            None
        );
        assert!(volume_icon(Picture::Removable).is::<gio::BytesIcon>());
        assert!(volume_icon(Picture::Network).is::<gio::ThemedIcon>());
    }
}

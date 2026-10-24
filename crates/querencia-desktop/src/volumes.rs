//! Volumes on the desktop (QE-03, QE-04), as on GNOME 2.
//!
//! What GIO tells about mounts and volumes, their four types, pictures,
//! "Aushängen"/"Auswerfen", mounting and the texts of failures come from
//! `querencia_dateiarbeit::laufwerke` (Desktop 0.3.0; up to 0.2.7 they were
//! here). The desktop keeps what is its own:
//!
//! - which types get an icon is the user's choice in `desktop.ini`
//!   `[Volumes]` (`config::IconSettings`, Querencia Einstellungen „Symbole
//!   der Arbeitsfläche“);
//! - 0.3.0 (E3): removable and optical media that are not mounted get an
//!   icon too; a double click or "Einhängen" mounts them. Internal
//!   partitions that are not mounted never appear (the system's own).
//! - Volume icons are never remembered: their places live only in memory
//!   (`Places`), are forgotten when the volume goes away, and never reach
//!   the state file `desktop-symbole`. A volume keeps its place while it is
//!   mounted (`Volume::slot`).

use gtk4::gio;
use gtk4::prelude::*;
use querencia_dateiarbeit::laufwerke::{self, Quelle};

pub use querencia_dateiarbeit::laufwerke::{Facts, Picture, Removal, VolumeType};

use crate::config::IconSettings;
use crate::layout::Place;

/// A volume icon: how it looks, how it goes away, and where it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// The root of the mount; `None` while the volume is not mounted.
    pub uri: Option<String>,
    /// The icon's identity for this session, the same before and after
    /// mounting: the GIO volume's UUID, device or name, else the root URI.
    pub slot: String,
    pub picture: Picture,
    pub removal: Option<Removal>,
}

/// Whether volumes of type `typ` get an icon with these settings.
pub fn shown_type(typ: VolumeType, settings: &IconSettings) -> bool {
    match typ {
        VolumeType::Removable => settings.removable,
        VolumeType::Optical => settings.optical,
        VolumeType::Network => settings.network,
        VolumeType::Internal => settings.internal,
    }
}

/// Whether the mount gets a desktop icon with these settings.
pub fn shown(facts: &Facts, settings: &IconSettings) -> bool {
    laufwerke::classify(facts).is_some_and(|typ| shown_type(typ, settings))
}

/// E3: a volume that is not mounted gets an icon only as removable or
/// optical medium (a stick, a card, a disc), and only when its type is on.
pub fn offered_unmounted(typ: VolumeType, settings: &IconSettings) -> bool {
    matches!(typ, VolumeType::Removable | VolumeType::Optical) && shown_type(typ, settings)
}

/// The identity of a GIO volume that stays when it is mounted.
pub fn slot_of(volume: &gio::Volume) -> String {
    let id = [
        gio::VOLUME_IDENTIFIER_KIND_UUID,
        gio::VOLUME_IDENTIFIER_KIND_UNIX_DEVICE,
    ]
    .into_iter()
    .find_map(|kind| volume.identifier(kind))
    .unwrap_or_else(|| volume.name());
    format!("volume:{id}")
}

/// The volumes that get an icon with these settings, by name: mounted ones
/// and (E3) removable media that are not mounted.
pub fn listed(monitor: &gio::VolumeMonitor, settings: &IconSettings) -> Vec<(String, Volume)> {
    laufwerke::alle(monitor)
        .into_iter()
        .filter_map(|drive| {
            let wanted = if drive.mounted {
                shown_type(drive.typ, settings)
            } else {
                offered_unmounted(drive.typ, settings)
            };
            if !wanted {
                return None;
            }
            let slot = match &drive.quelle {
                Quelle::Mount(mount) => mount.volume().as_ref().map(slot_of),
                Quelle::Volume(volume) => Some(slot_of(volume)),
            }
            .or_else(|| drive.uri.clone())?;
            Some((
                drive.name,
                Volume {
                    uri: drive.uri,
                    slot,
                    picture: drive.picture,
                    removal: drive.removal,
                },
            ))
        })
        .collect()
}

/// The GIO volume of an icon that is not mounted.
pub fn find_volume(monitor: &gio::VolumeMonitor, slot: &str) -> Option<gio::Volume> {
    monitor
        .volumes()
        .into_iter()
        .find(|volume| slot_of(volume) == slot)
}

/// The action of the icon menu for `removal`.
pub fn action(removal: Removal) -> &'static str {
    match removal {
        Removal::Eject => "desktop.eject",
        Removal::Unmount => "desktop.unmount",
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Anchor, Axis};
    use querencia_dateiarbeit::laufwerke::DriveFacts;

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

    /// The seed image of a test VM: an optical medium.
    fn cidata() -> Facts {
        Facts {
            name: "cidata".into(),
            root_uri: "file:///run/media/u/cidata".into(),
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

    fn nas_share() -> Facts {
        Facts {
            name: "daten auf nas.example".into(),
            root_uri: "smb://nas.example/daten/".into(),
            can_unmount: true,
            icon_names: vec!["folder-remote".into(), "folder".into()],
            ..Facts::default()
        }
    }

    /// QE-04 with the decided defaults: the optical medium stays, the share
    /// appears, an internal partition would go; each switch works alone.
    #[test]
    fn the_settings_choose_the_types() {
        let defaults = IconSettings::default();
        let mut disk = loop_image();
        disk.icon_names = vec!["drive-harddisk".into()];
        disk.drive = Some(DriveFacts::default());
        assert!(shown(&cidata(), &defaults));
        assert!(shown(&nas_share(), &defaults));
        assert!(shown(&usb_stick(), &defaults));
        assert!(!shown(&disk, &defaults));
        for (f, off) in [
            (
                usb_stick(),
                IconSettings {
                    removable: false,
                    ..defaults
                },
            ),
            (
                cidata(),
                IconSettings {
                    optical: false,
                    ..defaults
                },
            ),
            (
                nas_share(),
                IconSettings {
                    network: false,
                    ..defaults
                },
            ),
        ] {
            assert!(!shown(&f, &off), "{}", f.name);
        }
        assert!(shown(
            &disk,
            &IconSettings {
                internal: true,
                ..defaults
            }
        ));
    }

    /// E3: only media that are not mounted get an icon, never partitions
    /// of the system or shares that are not connected; the settings still
    /// decide per type.
    #[test]
    fn unmounted_volumes_get_an_icon_only_as_media() {
        let all = IconSettings {
            internal: true,
            ..IconSettings::default()
        };
        assert!(offered_unmounted(VolumeType::Removable, &all));
        assert!(offered_unmounted(VolumeType::Optical, &all));
        assert!(!offered_unmounted(VolumeType::Internal, &all));
        assert!(!offered_unmounted(VolumeType::Network, &all));
        let no_media = IconSettings {
            removable: false,
            ..all
        };
        assert!(!offered_unmounted(VolumeType::Removable, &no_media));
        assert!(offered_unmounted(VolumeType::Optical, &no_media));
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

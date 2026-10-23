// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 LGSE Ltd. (Strata)
// SPDX-FileCopyrightText: 2026 Querencia Desktop contributors
//
// Adapted from Strata `src/adapters/volume/mounts.rs` (Querencia Files,
// branch `querencia` at 3dfdb25f), MIT License, see
// `LICENSES/MIT-Strata.txt`. Changes: only what restoring needs
// (mount point and file system type of a path, mounts to scan for trash).

//! The mount table from `/proc/self/mountinfo`, read without touching the
//! mounted file systems (an unresponsive network mount cannot block it).

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct MountTable {
    entries: Arc<[(PathBuf, String)]>,
}

impl MountTable {
    pub fn current() -> Self {
        std::fs::read("/proc/self/mountinfo")
            .map(Self::parse)
            .unwrap_or_else(|_| Self::parse(b""))
    }

    pub fn parse(mountinfo: impl AsRef<[u8]>) -> Self {
        let entries = mountinfo
            .as_ref()
            .split(|byte| *byte == b'\n')
            .filter_map(|line| {
                let separator = line.windows(3).position(|bytes| bytes == b" - ")?;
                let (front, back) = (&line[..separator], &line[separator + 3..]);
                let mount_point = front.split(|byte| *byte == b' ').nth(4)?;
                let fs_type = std::str::from_utf8(back.split(|byte| *byte == b' ').next()?).ok()?;
                Some((PathBuf::from(unescape(mount_point)), fs_type.to_owned()))
            })
            .collect::<Vec<_>>();
        Self {
            entries: entries.into(),
        }
    }

    pub fn fs_type_for(&self, path: &Path) -> Option<&str> {
        self.innermost(path).map(|(_, fs_type)| fs_type.as_str())
    }

    pub fn mount_point_for(&self, path: &Path) -> Option<&Path> {
        self.innermost(path)
            .map(|(mount_point, _)| mount_point.as_path())
    }

    /// Excludes virtual or potentially blocking file systems from scans.
    pub fn trash_scan_mounts(&self) -> impl Iterator<Item = &Path> {
        self.entries.iter().filter_map(|(mount_point, fs_type)| {
            is_trash_scan_fs_type(fs_type).then_some(mount_point.as_path())
        })
    }

    fn innermost(&self, path: &Path) -> Option<&(PathBuf, String)> {
        self.entries
            .iter()
            .filter(|(mount_point, _)| path.starts_with(mount_point))
            .max_by_key(|(mount_point, _)| mount_point.as_os_str().len())
    }
}

fn is_trash_scan_fs_type(fs_type: &str) -> bool {
    !is_remote_fs_type(fs_type) && !is_virtual_fs_type(fs_type)
}

fn is_virtual_fs_type(fs_type: &str) -> bool {
    matches!(
        fs_type,
        "proc"
            | "sysfs"
            | "devtmpfs"
            | "tmpfs"
            | "cgroup"
            | "cgroup2"
            | "overlay"
            | "autofs"
            | "securityfs"
            | "debugfs"
            | "tracefs"
            | "ramfs"
            | "hugetlbfs"
            | "mqueue"
            | "bpf"
            | "pstore"
            | "configfs"
            | "fusectl"
            | "rpc_pipefs"
            | "nsfs"
            | "binfmt_misc"
            | "devpts"
            | "efivarfs"
    )
}

/// Includes FUSE because an unavailable userspace daemon can block calls.
pub fn is_remote_fs_type(fs_type: &str) -> bool {
    matches!(
        fs_type,
        "nfs"
            | "nfs4"
            | "cifs"
            | "smb"
            | "smb2"
            | "smb3"
            | "smbfs"
            | "ncpfs"
            | "afs"
            | "afp"
            | "9p"
            | "ceph"
            | "coda"
            | "davfs"
            | "glusterfs"
            | "gfs"
            | "gfs2"
            | "lustre"
            | "ocfs2"
            | "webdav"
            | "vboxsf"
            | "virtiofs"
    ) || fs_type.starts_with("fuse")
}

/// Decodes mountinfo's octal escapes without assuming path bytes are UTF-8.
fn unescape(field: impl AsRef<[u8]>) -> OsString {
    let bytes = field.as_ref();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let octal = (bytes[index] == b'\\' && index + 3 < bytes.len())
            .then(|| &bytes[index + 1..index + 4])
            .filter(|digits| digits.iter().all(|digit| (b'0'..=b'7').contains(digit)))
            .map(|digits| {
                digits
                    .iter()
                    .fold(0u32, |code, digit| code * 8 + u32::from(digit - b'0'))
            });
        match octal.and_then(|code| u8::try_from(code).ok()) {
            Some(byte) => {
                out.push(byte);
                index += 4;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    OsString::from_vec(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
22 1 0:20 / / rw,relatime shared:1 - ext4 /dev/sda2 rw
40 22 0:36 / /mnt/nfs rw,relatime shared:22 - nfs4 server:/export rw,vers=4.2
41 40 8:17 / /mnt/nfs/local rw,relatime shared:23 - ext4 /dev/sdb1 rw
52 22 0:45 / /run/user/1000/gvfs rw,nosuid,nodev,relatime shared:30 - fuse.gvfsd-fuse gvfsd-fuse rw,user_id=1000
60 22 0:50 / /mnt/my\\040share rw,relatime shared:31 - cifs //server/share rw
malformed line without separator
";

    #[test]
    fn innermost_mount_wins_and_escapes_are_decoded() {
        let table = MountTable::parse(SAMPLE);
        let nested = Path::new("/mnt/nfs/local/file");
        assert_eq!(table.fs_type_for(nested), Some("ext4"));
        assert_eq!(
            table.mount_point_for(nested),
            Some(Path::new("/mnt/nfs/local"))
        );
        assert_eq!(table.fs_type_for(Path::new("/mnt/nfs/docs")), Some("nfs4"));
        assert_eq!(
            table.mount_point_for(Path::new("/mnt/nfsdata")),
            Some(Path::new("/"))
        );
        assert_eq!(
            table.mount_point_for(Path::new("/mnt/my share/x")),
            Some(Path::new("/mnt/my share"))
        );
        let scanned: Vec<&Path> = table.trash_scan_mounts().collect();
        assert_eq!(scanned, [Path::new("/"), Path::new("/mnt/nfs/local")]);
    }
}

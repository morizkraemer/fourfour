use std::path::{Path, PathBuf};

use crate::command_error::CommandError;

pub fn list_removable_volumes() -> Result<Vec<String>, String> {
    let mut volumes: Vec<String> = candidate_volumes()?
        .into_iter()
        .filter(|path| is_removable(path) && refuse_system_volume(path).is_ok())
        .filter_map(|path| path.to_str().map(str::to_string))
        .collect();
    volumes.sort();
    Ok(volumes)
}

pub fn require_removable_volume(path: &Path) -> Result<(), CommandError> {
    let canonical = path
        .canonicalize()
        .map_err(|e| CommandError::from_io(&e, format!("Cannot resolve {}: {e}", path.display())).with_path(path).for_stick())?;
    #[cfg(test)]
    if fake_volumes::contains(&canonical) {
        return Ok(());
    }
    refuse_system_volume(&canonical)?;
    let is_listed = candidate_volumes()?
        .iter()
        .filter(|candidate| is_removable(candidate))
        .any(|candidate| candidate.canonicalize().is_ok_and(|c| c == canonical));
    if is_listed {
        Ok(())
    } else {
        Err(CommandError::other(format!("{} is not a removable volume.", path.display())).with_path(path))
    }
}

#[cfg(test)]
pub mod fake_volumes {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static VOLUMES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

    pub fn register(path: &Path) {
        let canonical = path.canonicalize().expect("fake volume must exist");
        VOLUMES.lock().unwrap().push(canonical);
    }

    pub(super) fn contains(canonical: &Path) -> bool {
        VOLUMES.lock().unwrap().iter().any(|v| v == canonical)
    }
}

pub fn refuse_system_volume(path: &Path) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve {}: {e}", path.display()))?;
    if is_system_volume(&canonical) {
        return Err(format!("Refusing to touch the system volume: {}", path.display()));
    }
    Ok(())
}

#[cfg(unix)]
fn is_system_volume(canonical: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    if canonical.starts_with("/System") {
        return true;
    }
    let device = |p: &Path| std::fs::metadata(p).ok().map(|m| m.dev());
    let boot_device = device(Path::new("/"));
    if boot_device.is_none() || device(canonical) == boot_device {
        return true;
    }
    match std::env::var_os("HOME") {
        Some(home) => overlaps_home(canonical, Path::new(&home)),
        None => false,
    }
}

#[cfg(unix)]
fn overlaps_home(canonical: &Path, home: &Path) -> bool {
    let home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    home.starts_with(canonical) || canonical.starts_with(&home)
}

#[cfg(windows)]
fn is_system_volume(canonical: &Path) -> bool {
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let canonical = canonical.to_string_lossy().trim_start_matches(r"\\?\").to_lowercase();
    canonical.starts_with(&system_drive.to_lowercase())
}

#[cfg(target_os = "macos")]
fn candidate_volumes() -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir("/Volumes").map_err(|e| e.to_string())?;
    Ok(entries.flatten().map(|entry| entry.path()).collect())
}

#[cfg(target_os = "macos")]
fn is_removable(path: &Path) -> bool {
    let Ok(output) = std::process::Command::new("diskutil")
        .args(["info", "-plist"])
        .arg(path)
        .output()
    else {
        return false;
    };
    output.status.success() && plist_says_removable(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "windows")]
fn candidate_volumes() -> Result<Vec<PathBuf>, String> {
    Ok((b'A'..=b'Z')
        .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
        .filter(|drive| drive.exists())
        .collect())
}

#[cfg(target_os = "windows")]
fn is_removable(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDriveTypeW(root_path_name: *const u16) -> u32;
    }
    const DRIVE_REMOVABLE: u32 = 2;

    let root: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOVABLE }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn candidate_volumes() -> Result<Vec<PathBuf>, String> {
    let mounts = std::fs::read_to_string("/proc/mounts").map_err(|e| format!("Cannot read /proc/mounts: {e}"))?;
    Ok(user_mount_points(&mounts))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn is_removable(path: &Path) -> bool {
    let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else {
        return false;
    };
    let Some(entry) = parse_mounts(&mounts)
        .into_iter()
        .find(|entry| Path::new(&entry.mount_point) == path)
    else {
        return false;
    };
    linux_volume_is_removable(&entry.filesystem, sysfs_removable_flag(&entry.device))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn sysfs_removable_flag(device: &str) -> Option<bool> {
    let name = Path::new(device).file_name()?;
    let block = Path::new("/sys/class/block").join(name).canonicalize().ok()?;
    [Some(block.as_path()), block.parent()]
        .into_iter()
        .flatten()
        .find_map(|dir| std::fs::read_to_string(dir.join("removable")).ok())
        .map(|flag| flag.trim() == "1")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn mount_device(mount_point: &Path) -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    device_for_mount_point(&mounts, mount_point)
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
fn device_for_mount_point(proc_mounts: &str, mount_point: &Path) -> Option<String> {
    parse_mounts(proc_mounts)
        .into_iter()
        .find(|entry| Path::new(&entry.mount_point) == mount_point)
        .map(|entry| entry.device)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn plist_says_removable(xml: &str) -> bool {
    !plist_is_disk_image(xml)
        && ["Ejectable", "RemovableMediaOrExternalDevice", "RemovableMedia"]
            .iter()
            .any(|key| plist_bool(xml, key) == Some(true))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn plist_is_disk_image(xml: &str) -> bool {
    plist_string(xml, "BusProtocol").as_deref() == Some("Disk Image")
        || plist_string(xml, "VirtualOrPhysical").as_deref() == Some("Virtual")
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn plist_string(xml: &str, key: &str) -> Option<String> {
    let after_key = xml.split(&format!("<key>{key}</key>")).nth(1)?;
    let value = after_key.trim_start().strip_prefix("<string>")?;
    Some(value.split("</string>").next()?.to_string())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn plist_bool(xml: &str, key: &str) -> Option<bool> {
    let after_key = xml.split(&format!("<key>{key}</key>")).nth(1)?;
    let value = after_key.trim_start();
    if value.starts_with("<true/>") {
        Some(true)
    } else if value.starts_with("<false/>") {
        Some(false)
    } else {
        None
    }
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
struct MountEntry {
    device: String,
    mount_point: String,
    filesystem: String,
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
fn parse_mounts(proc_mounts: &str) -> Vec<MountEntry> {
    proc_mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some(MountEntry {
                device: fields.next()?.to_string(),
                mount_point: unescape_mount_point(fields.next()?),
                filesystem: fields.next()?.to_string(),
            })
        })
        .collect()
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
fn linux_volume_is_removable(filesystem: &str, sysfs_removable: Option<bool>) -> bool {
    sysfs_removable == Some(true)
        || ["vfat", "exfat", "hfs", "hfsplus", "ntfs", "ntfs3", "fuseblk"].contains(&filesystem)
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
fn user_mount_points(proc_mounts: &str) -> Vec<PathBuf> {
    parse_mounts(proc_mounts)
        .into_iter()
        .map(|entry| entry.mount_point)
        .filter(|mount| ["/media/", "/run/media/", "/mnt/"].iter().any(|prefix| mount.starts_with(prefix)))
        .map(PathBuf::from)
        .collect()
}

#[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
fn unescape_mount_point(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|digits| bytes[i] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)))
            .and_then(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 8).ok());
        match octal {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_device_behind_a_mount_point() {
        let mounts = "/dev/sda1 /media/dj/MY\\040STICK vfat rw 0 0\n/dev/nvme0n1p2 / ext4 rw 0 0\n";
        assert_eq!(
            device_for_mount_point(mounts, Path::new("/media/dj/MY STICK")).as_deref(),
            Some("/dev/sda1")
        );
        assert_eq!(device_for_mount_point(mounts, Path::new("/media/dj/other")), None);
    }

    const BOOT_PLIST: &str = "<dict><key>Ejectable</key><false/><key>RemovableMedia</key><false/>\
        <key>RemovableMediaOrExternalDevice</key><false/></dict>";
    const STICK_PLIST: &str = "<dict><key>Ejectable</key>\n<true/>\n<key>RemovableMedia</key>\n<true/></dict>";

    #[test]
    fn internal_disk_plist_is_not_removable() {
        assert!(!plist_says_removable(BOOT_PLIST));
    }

    #[test]
    fn ejectable_stick_plist_is_removable() {
        assert!(plist_says_removable(STICK_PLIST));
    }

    #[test]
    fn mounted_disk_image_is_not_removable() {
        let installer = "<dict><key>BusProtocol</key>\n<string>Disk Image</string>\n<key>Ejectable</key>\n<true/>\n\
            <key>RemovableMedia</key>\n<true/>\n<key>RemovableMediaOrExternalDevice</key>\n<true/></dict>";
        assert!(!plist_says_removable(installer));
    }

    #[test]
    fn usb_stick_with_bus_protocol_is_removable() {
        let stick = "<dict><key>BusProtocol</key>\n<string>USB</string>\n<key>Ejectable</key>\n<true/>\n\
            <key>VirtualOrPhysical</key>\n<string>Physical</string></dict>";
        assert!(plist_says_removable(stick));
    }

    #[test]
    fn missing_keys_are_not_removable() {
        assert!(!plist_says_removable("<dict></dict>"));
    }

    #[test]
    fn linux_mounts_keep_only_user_mount_points() {
        let mounts = "/dev/sda1 / ext4 rw 0 0\n\
            /dev/sdb1 /media/merlin/DJ\\040STICK vfat rw 0 0\n\
            /dev/sdc1 /run/media/merlin/USB exfat rw 0 0\n\
            tmpfs /run tmpfs rw 0 0\n\
            /dev/sdd1 /mnt/backup ext4 rw 0 0\n";
        let points = user_mount_points(mounts);
        assert_eq!(
            points,
            vec![
                PathBuf::from("/media/merlin/DJ STICK"),
                PathBuf::from("/run/media/merlin/USB"),
                PathBuf::from("/mnt/backup"),
            ]
        );
    }

    #[test]
    fn removable_flag_wins_and_filesystem_is_the_fallback() {
        assert!(linux_volume_is_removable("ext4", Some(true)));
        assert!(linux_volume_is_removable("vfat", None));
        assert!(linux_volume_is_removable("exfat", Some(false)));
        assert!(!linux_volume_is_removable("ext4", Some(false)));
        assert!(!linux_volume_is_removable("ext4", None));
        assert!(!linux_volume_is_removable("nfs4", None));
        assert!(!linux_volume_is_removable("btrfs", None));
    }

    #[test]
    fn parses_device_and_filesystem_from_proc_mounts() {
        let entries = parse_mounts("/dev/sdb1 /media/merlin/DJ\\040STICK vfat rw 0 0\n");
        assert_eq!(entries[0].device, "/dev/sdb1");
        assert_eq!(entries[0].mount_point, "/media/merlin/DJ STICK");
        assert_eq!(entries[0].filesystem, "vfat");
    }

    #[test]
    fn unlisted_folder_is_not_a_removable_volume() {
        let dir = std::env::temp_dir().join(format!("volumes-test-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = require_removable_volume(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn registered_fake_volume_is_accepted() {
        let dir = std::env::temp_dir().join(format!("volumes-test-fake-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        fake_volumes::register(&dir);
        let result = require_removable_volume(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(result.is_ok());
    }

    #[test]
    #[cfg_attr(windows, ignore = "issue #39: '/' is not a Windows volume root")]
    fn root_is_refused() {
        assert!(refuse_system_volume(Path::new("/")).is_err());
    }

    #[test]
    fn missing_path_is_refused() {
        assert!(refuse_system_volume(Path::new("/definitely/not/here")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn home_directory_is_refused() {
        let home = std::env::var("HOME").unwrap();
        assert!(refuse_system_volume(Path::new(&home)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn folders_inside_or_above_home_overlap_it() {
        let home = Path::new("/Users/merlin");
        assert!(overlaps_home(Path::new("/Users/merlin/Music"), home));
        assert!(overlaps_home(Path::new("/Users"), home));
        assert!(!overlaps_home(Path::new("/Volumes/DJ STICK"), home));
        assert!(!overlaps_home(Path::new("/Users/merlinx"), home));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_root_is_refused() {
        let link = std::env::temp_dir().join(format!("volumes-test-boot-{}", std::process::id()));
        std::os::unix::fs::symlink("/", &link).unwrap();
        let refused = refuse_system_volume(&link).is_err();
        std::fs::remove_file(&link).unwrap();
        assert!(refused);
    }
}

use std::path::{Path, PathBuf};

pub fn list_removable_volumes() -> Result<Vec<String>, String> {
    let mut volumes: Vec<String> = candidate_volumes()?
        .into_iter()
        .filter(|path| is_removable(path) && refuse_system_volume(path).is_ok())
        .filter_map(|path| path.to_str().map(str::to_string))
        .collect();
    volumes.sort();
    Ok(volumes)
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
        Some(home) => Path::new(&home).starts_with(canonical),
        None => false,
    }
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
fn is_removable(_path: &Path) -> bool {
    true
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn candidate_volumes() -> Result<Vec<PathBuf>, String> {
    let mounts = std::fs::read_to_string("/proc/mounts").map_err(|e| format!("Cannot read /proc/mounts: {e}"))?;
    Ok(user_mount_points(&mounts))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn is_removable(_path: &Path) -> bool {
    true
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn plist_says_removable(xml: &str) -> bool {
    ["Ejectable", "RemovableMediaOrExternalDevice", "RemovableMedia"]
        .iter()
        .any(|key| plist_bool(xml, key) == Some(true))
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
fn user_mount_points(proc_mounts: &str) -> Vec<PathBuf> {
    proc_mounts
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(unescape_mount_point)
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
    fn symlink_to_root_is_refused() {
        let link = std::env::temp_dir().join(format!("volumes-test-boot-{}", std::process::id()));
        std::os::unix::fs::symlink("/", &link).unwrap();
        let refused = refuse_system_volume(&link).is_err();
        std::fs::remove_file(&link).unwrap();
        assert!(refused);
    }
}

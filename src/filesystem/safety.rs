use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FSError {
    #[error("Path does not exist: {0}")]
    NotFound(String),
    #[error("Path is a critical system path and cannot be modified: {0}")]
    ForbiddenPath(String),
    #[error("Path is outside allowed user boundaries: {0}")]
    OutOfBounds(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Critical system root paths that MUST NEVER be deleted under any circumstances.
const FORBIDDEN_ROOTS: &[&str] = &[
    "/", "/etc", "/usr", "/bin", "/sbin", "/boot", "/lib", "/lib64", "/lib32", "/root", "/dev",
    "/proc", "/sys", "/run", "/var", "/opt", "/srv", "/home",
];

/// Validates that a path is strictly safe to inspect or clean.
pub fn validate_path_safety(path: &Path) -> Result<PathBuf, FSError> {
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| FSError::OutOfBounds("HOME is not set; refusing deletion".into()))?;
    let home_canonical = home_dir.canonicalize()?;
    // HOME itself may be an alias. No cleanup-specific component may be a symlink.
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let absolute = if let Ok(rest) = absolute.strip_prefix(&home_dir) {
        home_canonical.join(rest)
    } else {
        absolute
    };
    let mut checked = PathBuf::new();
    for component in absolute.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(FSError::ForbiddenPath(path.display().to_string()));
        }
        checked.push(component);
        if std::fs::symlink_metadata(&checked)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    FSError::NotFound(path.display().to_string())
                } else {
                    FSError::Io(error)
                }
            })?
            .file_type()
            .is_symlink()
        {
            return Err(FSError::ForbiddenPath(path.display().to_string()));
        }
    }
    let canonical = absolute.canonicalize()?;

    // 1. Direct roots protection: /, /tmp, /var/tmp, or forbidden roots
    if canonical == Path::new("/")
        || canonical == Path::new("/tmp")
        || canonical == Path::new("/var/tmp")
    {
        return Err(FSError::ForbiddenPath(canonical.display().to_string()));
    }

    for &forbidden in FORBIDDEN_ROOTS {
        let fpath = Path::new(forbidden);
        if canonical == fpath {
            return Err(FSError::ForbiddenPath(canonical.display().to_string()));
        }
    }

    // 2. Check if inside /tmp or /var/tmp (allowed temporary storage)
    let is_inside_tmp =
        canonical.starts_with(Path::new("/tmp")) || canonical.starts_with(Path::new("/var/tmp"));

    // 3. User Home boundary check

    // Exact home directory cannot be modified
    if canonical == home_canonical {
        return Err(FSError::ForbiddenPath(canonical.display().to_string()));
    }

    let is_inside_home = canonical.starts_with(&home_canonical);

    // Protected sensitive user directories inside $HOME
    if is_inside_home {
        let protected_user_subdirs = [
            ".ssh",
            ".gnupg",
            ".bashrc",
            ".bash_profile",
            ".zshrc",
            ".profile",
            ".config",
            ".local",
            ".local/share",
            "Documents",
            "Desktop",
        ];
        for sub in &protected_user_subdirs {
            let protected_path = home_canonical.join(sub);
            if canonical == protected_path {
                return Err(FSError::ForbiddenPath(format!(
                    "Protected user location: {}",
                    sub
                )));
            }
        }
    }

    // 4. If not inside allowed /tmp, reject critical system prefixes
    if !is_inside_tmp {
        let forbidden_prefixes = [
            "/etc", "/usr", "/bin", "/sbin", "/boot", "/lib", "/lib64", "/lib32", "/dev", "/proc",
            "/sys", "/run", "/var", "/opt", "/srv", "/root",
        ];
        for &prefix in &forbidden_prefixes {
            let p = Path::new(prefix);
            if canonical.starts_with(p) {
                return Err(FSError::ForbiddenPath(canonical.display().to_string()));
            }
        }
    }

    // 5. Must be strictly inside home or inside allowed tmp
    if !is_inside_home && !is_inside_tmp {
        return Err(FSError::OutOfBounds(canonical.display().to_string()));
    }

    Ok(canonical)
}

/// Safely opens a file or directory in the default desktop file manager.
pub fn open_in_file_manager(path: &Path) -> Result<(), FSError> {
    if !path.exists() {
        return Err(FSError::NotFound(path.display().to_string()));
    }

    let target = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    let status = crate::process::output(
        std::process::Command::new("xdg-open").arg(target),
        std::time::Duration::from_secs(15),
    );

    match status {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::warn!("Failed to launch xdg-open: {}", e);
            Err(FSError::Io(e))
        }
    }
}

/// Safely removes a file or directory after strict path safety validation.
#[allow(dead_code)]
pub fn safe_delete(path: &Path) -> Result<u64, FSError> {
    let report = traverse_target(path, &std::sync::atomic::AtomicBool::new(false), true, true)?;
    if !report.errors.is_empty() {
        return Err(FSError::Io(std::io::Error::other(report.errors.join("; "))));
    }
    Ok(report.bytes)
}

#[derive(Debug, Default)]
pub struct TraversalReport {
    pub bytes: u64,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

// Linux openat2 rejects bind mounts as well as ordinary mounts. Unsupported kernels
// fail closed; st_dev alone cannot enforce this boundary.
pub fn traverse_target(
    path: &Path,
    cancel: &std::sync::atomic::AtomicBool,
    delete: bool,
    remove_root: bool,
) -> Result<TraversalReport, FSError> {
    traverse_with_policy(path, cancel, delete, remove_root, &|boundary, entry, _| {
        mount_allowed(boundary, entry)
    })
}

fn traverse_with_policy(
    path: &Path,
    cancel: &std::sync::atomic::AtomicBool,
    delete: bool,
    remove_root: bool,
    policy: &dyn Fn(u64, u64, &Path) -> bool,
) -> Result<TraversalReport, FSError> {
    use std::ffi::{CStr, CString};
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::sync::atomic::Ordering;

    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }
    fn open(parent: i32, name: &Path, directory: bool, confined: bool) -> io::Result<OwnedFd> {
        let name = CString::new(name.as_os_str().as_bytes())?;
        let how = OpenHow {
            flags: (libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | if directory {
                    libc::O_RDONLY | libc::O_DIRECTORY
                } else {
                    libc::O_PATH
                }) as u64,
            mode: 0,
            resolve: 0x04 | if confined { 0x01 | 0x08 } else { 0 }, // NO_SYMLINKS, NO_XDEV, BENEATH
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                parent,
                name.as_ptr(),
                &how,
                std::mem::size_of::<OpenHow>(),
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
        }
    }
    fn stat(parent: i32, name: &Path) -> io::Result<libc::statx> {
        let name = CString::new(name.as_os_str().as_bytes())?;
        let mut st: libc::statx = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            libc::statx(
                parent,
                name.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW | libc::AT_EMPTY_PATH,
                libc::STATX_BASIC_STATS | libc::STATX_MNT_ID,
                &mut st,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        if st.stx_mask & libc::STATX_MNT_ID == 0 {
            return Err(io::Error::other("mount identity unavailable"));
        }
        Ok(st)
    }
    fn same(a: &libc::statx, b: &libc::statx) -> bool {
        a.stx_ino == b.stx_ino && a.stx_mnt_id == b.stx_mnt_id && a.stx_mode == b.stx_mode
    }
    fn names(fd: i32, cancel: &std::sync::atomic::AtomicBool) -> io::Result<Vec<PathBuf>> {
        let dup = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        if dup < 0 {
            return Err(io::Error::last_os_error());
        }
        let dir = unsafe { libc::fdopendir(dup) };
        if dir.is_null() {
            unsafe {
                libc::close(dup);
            }
            return Err(io::Error::last_os_error());
        }
        let mut result = Vec::new();
        loop {
            if cancel.load(Ordering::Acquire) {
                unsafe {
                    libc::closedir(dir);
                }
                return Ok(result);
            }
            unsafe {
                *libc::__errno_location() = 0;
            }
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                unsafe {
                    libc::closedir(dir);
                }
                if error.raw_os_error() != Some(0) {
                    return Err(error);
                }
                return Ok(result);
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name != b"." && name != b".." {
                result.push(PathBuf::from(std::ffi::OsStr::from_bytes(name)));
            }
        }
    }
    struct Walk<'a> {
        anchor: i32,
        mount: u64,
        cancel: &'a std::sync::atomic::AtomicBool,
        delete: bool,
        report: TraversalReport,
        policy: &'a dyn Fn(u64, u64, &Path) -> bool,
    }
    impl Walk<'_> {
        fn visit(
            &mut self,
            parent: i32,
            name: &Path,
            relative: &Path,
            remove: bool,
            depth: usize,
        ) -> io::Result<()> {
            if self.cancel.load(Ordering::Acquire) {
                self.report.cancelled = true;
                return Ok(());
            }
            if depth > 128 {
                return Err(io::Error::other("cleanup depth limit exceeded"));
            }
            let before = stat(parent, name)?;
            if !(self.policy)(self.mount, before.stx_mnt_id, relative) {
                return Err(io::Error::other("mount boundary refused"));
            }
            let directory = before.stx_mode as u32 & libc::S_IFMT == libc::S_IFDIR;
            if directory {
                let fd = open(parent, name, true, true)?;
                if !same(&before, &stat(fd.as_raw_fd(), Path::new(""))?) {
                    return Err(io::Error::other("directory identity changed"));
                }
                for child in names(fd.as_raw_fd(), self.cancel)? {
                    if self.cancel.load(Ordering::Acquire) {
                        self.report.cancelled = true;
                        break;
                    }
                    let preserve_trash_container = depth == 0
                        && !remove
                        && name == Path::new("Trash")
                        && (child == Path::new("files") || child == Path::new("info"));
                    if let Err(e) = self.visit(
                        fd.as_raw_fd(),
                        &child,
                        &relative.join(&child),
                        !preserve_trash_container,
                        depth + 1,
                    ) {
                        self.report.errors.push(format!(
                            "{}: {}",
                            relative.join(child).display(),
                            e
                        ));
                    }
                }
            }
            if self.cancel.load(Ordering::Acquire) {
                self.report.cancelled = true;
                return Ok(());
            }
            if self.delete && remove {
                // Reopen the parent from the boundary: reject renamed/replaced ancestors.
                // Linux has no inode-conditional unlink; a hostile same-UID rename
                // between this recheck and unlinkat cannot be made atomic here.
                let linked_parent = open(
                    self.anchor,
                    relative
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new(".")),
                    true,
                    true,
                )?;
                if !same(
                    &stat(parent, Path::new(""))?,
                    &stat(linked_parent.as_raw_fd(), Path::new(""))?,
                ) || !same(&before, &stat(parent, name)?)
                {
                    return Err(io::Error::other("cleanup identity changed"));
                }
                let name = CString::new(name.as_os_str().as_bytes())?;
                if unsafe {
                    libc::unlinkat(
                        parent,
                        name.as_ptr(),
                        if directory { libc::AT_REMOVEDIR } else { 0 },
                    )
                } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            // Count only regular files actually unlinked, never scanned estimates or symlink targets.
            if (!self.delete || remove) && before.stx_mode as u32 & libc::S_IFMT == libc::S_IFREG {
                self.report.bytes = self.report.bytes.saturating_add(before.stx_size);
            }
            Ok(())
        }
    }
    let canonical = validate_path_safety(path)?;
    let home = PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?,
    )
    .canonicalize()?;
    let boundary = if canonical.starts_with("/var/tmp") {
        Path::new("/var/tmp")
    } else if canonical.starts_with("/tmp") {
        Path::new("/tmp")
    } else {
        home.as_path()
    };
    let anchor = open(libc::AT_FDCWD, boundary, true, false)?;
    let relative = canonical
        .strip_prefix(boundary)
        .map_err(|_| io::Error::other("invalid boundary"))?;
    let parent = open(
        anchor.as_raw_fd(),
        relative
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
        true,
        true,
    )?;
    let mut walk = Walk {
        anchor: anchor.as_raw_fd(),
        mount: stat(anchor.as_raw_fd(), Path::new(""))?.stx_mnt_id,
        cancel,
        delete,
        report: TraversalReport::default(),
        policy,
    };
    let name = Path::new(
        relative
            .file_name()
            .ok_or_else(|| io::Error::other("invalid cleanup root"))?,
    );
    let directory = stat(parent.as_raw_fd(), name)?.stx_mode as u32 & libc::S_IFMT == libc::S_IFDIR;
    if let Err(e) = walk.visit(
        parent.as_raw_fd(),
        name,
        relative,
        remove_root || !directory,
        0,
    ) {
        walk.report.errors.push(e.to_string());
    }
    Ok(walk.report)
}

fn mount_allowed(boundary: u64, entry: u64) -> bool {
    boundary == entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn simulated_bind_mount_is_skipped_even_with_same_device() {
        use std::sync::atomic::AtomicBool;
        let root = std::env::temp_dir().join(format!("tidy_mount_policy_{}", std::process::id()));
        std::fs::create_dir_all(root.join("mounted")).unwrap();
        std::fs::write(root.join("mounted/keep"), b"keep").unwrap();
        std::fs::write(root.join("delete"), b"123").unwrap();
        assert!(!mount_allowed(17, 18));
        assert!(mount_allowed(17, 17));
        let report = traverse_with_policy(
            &root,
            &AtomicBool::new(false),
            true,
            false,
            &|boundary, entry, path| {
                mount_allowed(
                    boundary,
                    if path.ends_with("mounted") {
                        entry + 1
                    } else {
                        entry
                    },
                )
            },
        )
        .unwrap();
        assert_eq!(report.bytes, 3);
        assert!(!report.errors.is_empty());
        assert!(root.join("mounted/keep").exists());
        assert!(!root.join("delete").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_interrupts_a_single_tree_and_preserves_remaining_files() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let root = std::env::temp_dir().join(format!("tidy_mid_cancel_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..32 {
            std::fs::write(root.join(format!("{i}")), b"1234").unwrap();
        }
        let cancel = AtomicBool::new(false);
        let visits = AtomicUsize::new(0);
        let report = traverse_with_policy(&root, &cancel, true, false, &|a, b, _| {
            if visits.fetch_add(1, Ordering::Relaxed) == 5 {
                cancel.store(true, Ordering::Release);
            }
            mount_allowed(a, b)
        })
        .unwrap();
        assert!(report.cancelled);
        assert!(report.bytes > 0 && report.bytes < 128);
        let remaining = std::fs::read_dir(&root).unwrap().count();
        assert_eq!(report.bytes, (32 - remaining) as u64 * 4);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renamed_open_directory_is_not_cleaned_under_replacement_identity() {
        use std::sync::atomic::AtomicBool;
        let root =
            std::env::temp_dir().join(format!("tidy_rename_identity_{}", std::process::id()));
        let moved = root.with_extension("moved");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("victim"), b"preserve").unwrap();
        let report = traverse_with_policy(
            &root,
            &AtomicBool::new(false),
            true,
            false,
            &|a, b, path| {
                if path.ends_with("victim") {
                    std::fs::rename(&root, &moved).unwrap();
                    std::fs::create_dir(&root).unwrap();
                    std::fs::write(root.join("victim"), b"replacement").unwrap();
                }
                mount_allowed(a, b)
            },
        )
        .unwrap();
        assert_eq!(report.bytes, 0);
        assert!(!report.errors.is_empty());
        assert_eq!(std::fs::read(moved.join("victim")).unwrap(), b"preserve");
        assert_eq!(std::fs::read(root.join("victim")).unwrap(), b"replacement");
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(moved).unwrap();
    }

    #[test]
    fn test_forbidden_paths() {
        assert!(validate_path_safety(Path::new("/etc")).is_err());
        assert!(validate_path_safety(Path::new("/usr")).is_err());
        assert!(validate_path_safety(Path::new("/boot")).is_err());
        assert!(validate_path_safety(Path::new("/")).is_err());
    }

    #[test]
    fn test_safe_temp_deletion() {
        let temp_dir = std::env::temp_dir().join("cleaner_safety_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("test_file.tmp");
        {
            let mut file = File::create(&file_path).unwrap();
            use std::io::Write;
            file.write_all(b"hello temporary world").unwrap();
        }

        assert!(file_path.exists());
        let deleted = safe_delete(&file_path).unwrap();
        assert!(deleted > 0);
        assert!(!file_path.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

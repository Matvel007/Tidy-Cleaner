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
    if !path.exists() {
        return Err(FSError::NotFound(path.display().to_string()));
    }

    let canonical = path.canonicalize().map_err(FSError::Io)?;

    let path_str = canonical.to_string_lossy();

    // 1. Check exact match against forbidden roots
    for &forbidden in FORBIDDEN_ROOTS {
        if path_str == forbidden {
            return Err(FSError::ForbiddenPath(path_str.to_string()));
        }
    }

    // 2. Check if path is a top-level essential system directory
    if path_str.starts_with("/etc")
        || path_str.starts_with("/usr")
        || path_str.starts_with("/bin")
        || path_str.starts_with("/sbin")
        || path_str.starts_with("/boot")
        || path_str.starts_with("/lib")
        || path_str.starts_with("/lib64")
        || path_str.starts_with("/dev")
        || path_str.starts_with("/proc")
        || path_str.starts_with("/sys")
        || path_str.starts_with("/run")
        || path_str.starts_with("/var")
        || path_str.starts_with("/opt")
        || path_str.starts_with("/srv")
        || path_str.starts_with("/root")
    {
        return Err(FSError::ForbiddenPath(path_str.to_string()));
    }

    // 3. User Home boundary check (must be inside user's home or /tmp).
    //    Fail closed: if HOME cannot be determined, refuse to delete.
    let home_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| FSError::OutOfBounds("HOME is not set; refusing deletion".to_string()))?;
    let home_canonical = home_dir.canonicalize().unwrap_or(home_dir);

    // Exact home directory cannot be deleted
    if canonical == home_canonical {
        return Err(FSError::ForbiddenPath(path_str.to_string()));
    }

    let is_inside_home = canonical.starts_with(&home_canonical);
    let is_inside_tmp = canonical.starts_with("/tmp") || canonical.starts_with("/var/tmp");

    if !is_inside_home && !is_inside_tmp {
        return Err(FSError::OutOfBounds(path_str.to_string()));
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

    let status = std::process::Command::new("xdg-open").arg(target).status();

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
    let canonical = validate_path_safety(path)?;

    let mut deleted_bytes = 0u64;

    if canonical.is_file() || canonical.is_symlink() {
        if let Ok(meta) = canonical.symlink_metadata() {
            deleted_bytes = meta.len();
        }
        std::fs::remove_file(&canonical)?;
    } else if canonical.is_dir() {
        // Calculate size before deletion
        for entry in walkdir::WalkDir::new(&canonical)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    deleted_bytes += meta.len();
                }
            }
        }
        std::fs::remove_dir_all(&canonical)?;
    }

    Ok(deleted_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

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

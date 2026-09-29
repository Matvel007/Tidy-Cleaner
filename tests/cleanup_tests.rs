use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tidy_cleaner::cleanup::cleaner::Cleaner;
use tidy_cleaner::cleanup::models::{CleanupCategory, CleanupItem, RiskLevel};
use tidy_cleaner::filesystem::safety::{validate_path_safety, FSError};

#[test]
fn test_validate_path_safety_forbidden_roots() {
    let forbidden_paths = ["/etc", "/usr", "/boot", "/bin", "/root", "/var/log"];
    for p in forbidden_paths {
        let path = Path::new(p);
        if path.exists() {
            let res = validate_path_safety(path);
            assert!(
                matches!(res, Err(FSError::ForbiddenPath(_))),
                "Path {} should be forbidden, got {:?}",
                p,
                res
            );
        }
    }
}

#[test]
fn test_validate_path_safety_protected_user_dirs() {
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(home);
        let sensitive = [".ssh", ".bashrc", ".config", "Documents"];
        for s in sensitive {
            let target = home_path.join(s);
            if target.exists() {
                let res = validate_path_safety(&target);
                assert!(
                    matches!(res, Err(FSError::ForbiddenPath(_))),
                    "User sensitive path {:?} should be forbidden, got {:?}",
                    target,
                    res
                );
            }
        }
    }
}

#[test]
fn test_validate_path_safety_allows_tmp_subpaths() {
    let unique_dir = std::env::temp_dir().join(format!("tidy_safety_test_{}", std::process::id()));
    fs::create_dir_all(&unique_dir).unwrap();

    let res = validate_path_safety(&unique_dir);
    assert!(res.is_ok(), "Expected safe tmp subpath to be allowed: {:?}", res);

    let _ = fs::remove_dir_all(&unique_dir);
}

#[tokio::test]
async fn test_cleaner_symlink_safety_does_not_delete_target() {
    let test_dir = std::env::temp_dir().join(format!("tidy_symlink_test_{}", std::process::id()));
    let cache_dir = test_dir.join("app_cache");
    let outside_dir = test_dir.join("outside_protected");

    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&outside_dir).unwrap();

    // Create important target file outside
    let secret_file = outside_dir.join("secret_keys.txt");
    {
        let mut f = File::create(&secret_file).unwrap();
        f.write_all(b"super secret data").unwrap();
    }

    // Create a regular cache file
    let normal_cache = cache_dir.join("temp.cache");
    {
        let mut f = File::create(&normal_cache).unwrap();
        f.write_all(b"temporary cache content").unwrap();
    }

    // Create symlink inside cache pointing to the outside secret file
    #[cfg(unix)]
    {
        let link_path = cache_dir.join("symlink_to_secret");
        std::os::unix::fs::symlink(&secret_file, &link_path).unwrap();
        assert!(link_path.exists());
    }

    let item = CleanupItem {
        id: "test_cache_item".to_string(),
        rule_id: "test_rule".to_string(),
        name: "Test Cache".to_string(),
        description: "Test Cache Description".to_string(),
        path: cache_dir.clone(),
        size_bytes: 23,
        size_formatted: "23 B".to_string(),
        safety_level: RiskLevel::Safe,
        category: CleanupCategory::ApplicationCache,
        selected: true,
    };

    let cancel = Arc::new(AtomicBool::new(false));
    let summary = Cleaner::run_clean(vec![item], cancel, None, None).await;

    assert_eq!(summary.items_cleaned, 1);
    assert!(summary.errors.is_empty(), "Cleaning errors: {:?}", summary.errors);

    // CRITICAL ASSERTION: The outside secret file MUST STILL EXIST and have intact content!
    assert!(secret_file.exists(), "Target file must not be deleted via symlink traversal!");
    let content = fs::read_to_string(&secret_file).unwrap();
    assert_eq!(content, "super secret data");

    // The normal cache inside cache_dir should be deleted
    assert!(!normal_cache.exists());

    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_parse_journal_size() {
    use tidy_cleaner::cleanup::scanner::Scanner;

    assert_eq!(
        Scanner::parse_journal_size("Archived and active journals take up 16.6M in the file system."),
        (16.6 * 1024.0 * 1024.0) as u64
    );

    assert_eq!(
        Scanner::parse_journal_size("Archived and active journals take up 1.5G in the file system."),
        (1.5 * 1024.0 * 1024.0 * 1024.0) as u64
    );

    assert_eq!(
        Scanner::parse_journal_size("Archived and active journals take up 512K in the file system."),
        512 * 1024
    );

    // Comma decimal separator support
    assert_eq!(
        Scanner::parse_journal_size("Archived and active journals take up 16,6M in the file system."),
        (16.6 * 1024.0 * 1024.0) as u64
    );

    assert_eq!(Scanner::parse_journal_size("Unknown output"), 0);
}

#[test]
fn test_default_rules_registered() {
    use tidy_cleaner::cleanup::rules::RuleRegistry;

    let rules = RuleRegistry::get_default_rules();
    assert!(rules.iter().any(|r| r.id == "flatpak_apps"));
    assert!(rules.iter().any(|r| r.id == "systemd_journal"));
    assert!(rules.iter().any(|r| r.id == "user_trash"));
    assert!(rules.iter().any(|r| r.id == "thumbnails"));
    assert!(rules.iter().any(|r| r.id == "orphaned_packages"));
}

#[test]
fn test_parse_pacman_installed_size() {
    use tidy_cleaner::cleanup::scanner::Scanner;

    let sample_output = r#"
Name            : amf-headers
Version         : 1.4.36-1
Installed Size  : 581.41 KiB

Name            : doxygen
Version         : 1.13.2-1
Installed Size  : 25.06 MiB

Name            : huge-pkg
Version         : 1.0-1
Installed Size  : 1.50 GiB
"#;

    let total = Scanner::parse_pacman_installed_size(sample_output);
    let expected = (581.41 * 1024.0) as u64
        + (25.06 * 1024.0 * 1024.0) as u64
        + (1.50 * 1024.0 * 1024.0 * 1024.0) as u64;

    assert_eq!(total, expected);
}

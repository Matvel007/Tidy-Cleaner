use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tidy_cleaner::cleanup::cleaner::Cleaner;
use tidy_cleaner::cleanup::cleaner::CleanupOutcome;
use tidy_cleaner::cleanup::models::{CleanupCategory, CleanupItem, RiskLevel};
use tidy_cleaner::cleanup::models::{CleanupRule, ScanPhase};
use tidy_cleaner::cleanup::scanner::Scanner;
use tidy_cleaner::filesystem::safety::{validate_path_safety, FSError};

fn fixture(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tidy_cleanup_{}_{}", std::process::id(), name));
    fs::create_dir_all(&path).unwrap();
    path
}

fn cleanup_item(path: PathBuf) -> CleanupItem {
    CleanupItem {
        id: path.display().to_string(),
        rule_id: "regression".into(),
        name: "cleanup.rule.thumbnails".into(),
        description: String::new(),
        path,
        size_bytes: 999999,
        size_formatted: "999999 B".into(),
        safety_level: RiskLevel::Safe,
        category: CleanupCategory::ApplicationCache,
        selected: true,
    }
}

#[tokio::test]
async fn root_and_ancestor_symlinks_are_rejected() {
    let root = fixture("root_links");
    let actual = root.join("actual/cache");
    fs::create_dir_all(&actual).unwrap();
    fs::write(actual.join("recoverable"), b"keep").unwrap();
    std::os::unix::fs::symlink(root.join("actual"), root.join("ancestor")).unwrap();
    std::os::unix::fs::symlink(&actual, root.join("root_link")).unwrap();
    for target in [root.join("ancestor/cache"), root.join("root_link")] {
        assert!(validate_path_safety(&target).is_err());
        let summary = Cleaner::run_clean(
            vec![cleanup_item(target)],
            Arc::new(AtomicBool::new(false)),
            None,
            None,
        )
        .await;
        assert_eq!(summary.outcome, CleanupOutcome::Partial);
        assert!(summary.cleaned_ids.is_empty());
        assert_eq!(summary.bytes_freed, 0);
    }
    assert_eq!(fs::read(actual.join("recoverable")).unwrap(), b"keep");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn canonical_home_and_home_alias_remain_usable() {
    if let Some(actual) = std::env::var_os("TIDY_TEST_HOME_CANONICAL") {
        let actual = PathBuf::from(actual);
        let alias = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert!(validate_path_safety(&actual).is_err());
        assert!(validate_path_safety(&alias).is_err());
        assert_eq!(
            validate_path_safety(&alias.join("cache")).unwrap(),
            actual.join("cache")
        );
        assert_eq!(
            validate_path_safety(&actual.join("cache")).unwrap(),
            actual.join("cache")
        );
        return;
    }
    let root = fixture("home_alias");
    let actual = root.join("real_home");
    fs::create_dir_all(actual.join("cache")).unwrap();
    let alias = root.join("alias_home");
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "canonical_home_and_home_alias_remain_usable",
            "--nocapture",
        ])
        .env("HOME", alias)
        .env("TIDY_TEST_HOME_CANONICAL", actual)
        .status()
        .unwrap();
    assert!(status.success());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn empty_cache_does_not_report_stale_scanned_bytes() {
    let root = fixture("empty");
    fs::write(root.join("zero"), b"").unwrap();
    let summary = Cleaner::run_clean(
        vec![cleanup_item(root.clone())],
        Arc::new(AtomicBool::new(false)),
        None,
        None,
    )
    .await;
    assert_eq!(summary.outcome, CleanupOutcome::Complete);
    assert_eq!(summary.items_cleaned, 1);
    assert_eq!(summary.bytes_freed, 0);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn partial_deletion_keeps_item_unresolved_and_counts_only_successes() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let root = fixture("partial");
    fs::write(root.join("disposable"), b"12345").unwrap();
    let blocked = root.join("blocked");
    fs::create_dir(&blocked).unwrap();
    fs::write(blocked.join("remaining"), b"not removed").unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500)).unwrap();
    let summary = Cleaner::run_clean(
        vec![cleanup_item(root.clone())],
        Arc::new(AtomicBool::new(false)),
        None,
        None,
    )
    .await;
    assert_eq!(summary.outcome, CleanupOutcome::Partial);
    assert_eq!(summary.bytes_freed, 5);
    assert_eq!(summary.items_cleaned, 0);
    assert!(summary.cleaned_ids.is_empty());
    assert!(!summary.errors.is_empty());
    assert!(blocked.join("remaining").exists());
    fs::set_permissions(blocked, fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_never_emits_completed_or_claims_cleaned_items() {
    let root = fixture("cancelled");
    fs::write(root.join("keep"), b"keep").unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let summary = Cleaner::run_clean(
        vec![cleanup_item(root.clone())],
        Arc::new(AtomicBool::new(true)),
        Some(tx),
        None,
    )
    .await;
    assert_eq!(summary.outcome, CleanupOutcome::Cancelled);
    assert!(summary.cleaned_ids.is_empty());
    while let Some(progress) = rx.recv().await {
        assert_ne!(progress.phase, ScanPhase::Completed);
    }
    assert!(root.join("keep").exists());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn jetbrains_scans_only_disposable_per_product_caches() {
    let root = fixture("jetbrains");
    let base = root.join("JetBrains");
    for dir in [
        "IntelliJIdea2025.2/caches",
        "IntelliJIdea2025.2/index",
        "IntelliJIdea2025.2/tmp",
        "IntelliJIdea2025.2/LocalHistory",
        "IntelliJIdea2025.2/recovery",
        "IntelliJIdea2025.2/workspace",
        "unknown2025.2/caches",
        "PyCharm2025.1/caches",
        "PyCharm2025.1/LocalHistory",
    ] {
        fs::create_dir_all(base.join(dir)).unwrap();
        fs::write(base.join(dir).join("data"), b"data").unwrap();
    }
    let rule = CleanupRule {
        id: "jetbrains_cache".into(),
        name_key: "cleanup.rule.jetbrains".into(),
        description_key: "cleanup.rule.jetbrains.desc".into(),
        category: CleanupCategory::ApplicationCache,
        base_path: base.clone(),
        is_deep_scan: true,
        safety_level: RiskLevel::Safe,
    };
    let items = Scanner::run_scan(vec![rule], true, Arc::new(AtomicBool::new(false)), None).await;
    assert_eq!(items.len(), 3);
    assert_eq!(items.iter().map(|i| i.size_bytes).sum::<u64>(), 12);
    let summary = Cleaner::run_clean(items, Arc::new(AtomicBool::new(false)), None, None).await;
    assert_eq!(summary.bytes_freed, 12);
    assert_eq!(summary.outcome, CleanupOutcome::Complete);
    for preserved in [
        "IntelliJIdea2025.2/LocalHistory",
        "IntelliJIdea2025.2/recovery",
        "IntelliJIdea2025.2/workspace",
        "IntelliJIdea2025.2/tmp",
        "unknown2025.2/caches",
        "PyCharm2025.1/LocalHistory",
    ] {
        assert!(base.join(preserved).join("data").exists());
    }
    let mut broad = cleanup_item(base.clone());
    broad.rule_id = "jetbrains_cache".into();
    assert_eq!(
        Cleaner::run_clean(vec![broad], Arc::new(AtomicBool::new(false)), None, None)
            .await
            .outcome,
        CleanupOutcome::Partial
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn apt_orphans_fail_closed_without_elevation() {
    let mut item = cleanup_item(PathBuf::from("orphans:apt:previously-seen-package"));
    item.rule_id = "orphaned_packages".into();
    let summary =
        Cleaner::run_clean(vec![item], Arc::new(AtomicBool::new(false)), None, None).await;
    assert_eq!(summary.outcome, CleanupOutcome::Partial);
    assert_eq!(summary.bytes_freed, 0);
    assert!(summary.cleaned_ids.is_empty());
}

#[tokio::test]
async fn journal_failure_is_not_reported_as_cleanup_success() {
    use std::os::unix::fs::PermissionsExt;
    if std::env::var_os("TIDY_TEST_JOURNAL_FAILURE").is_some() {
        let mut item = cleanup_item(PathBuf::from("journalctl:--user"));
        item.rule_id = "systemd_journal".into();
        let summary =
            Cleaner::run_clean(vec![item], Arc::new(AtomicBool::new(false)), None, None).await;
        assert_eq!(summary.outcome, CleanupOutcome::Partial);
        assert_eq!(summary.bytes_freed, 0);
        assert!(summary.cleaned_ids.is_empty());
        return;
    }
    let root = fixture("failed_journal");
    fs::write(root.join("journalctl"), b"#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(root.join("journalctl"), fs::Permissions::from_mode(0o700)).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "journal_failure_is_not_reported_as_cleanup_success",
            "--nocapture",
        ])
        .env("PATH", &root)
        .env("TIDY_TEST_JOURNAL_FAILURE", "1")
        .status()
        .unwrap();
    assert!(status.success());
    fs::remove_dir_all(root).unwrap();
}

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
    assert!(
        res.is_ok(),
        "Expected safe tmp subpath to be allowed: {:?}",
        res
    );

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
    assert!(
        summary.errors.is_empty(),
        "Cleaning errors: {:?}",
        summary.errors
    );

    // CRITICAL ASSERTION: The outside secret file MUST STILL EXIST and have intact content!
    assert!(
        secret_file.exists(),
        "Target file must not be deleted via symlink traversal!"
    );
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
        Scanner::parse_journal_size(
            "Archived and active journals take up 16.6M in the file system."
        ),
        (16.6 * 1024.0 * 1024.0) as u64
    );

    assert_eq!(
        Scanner::parse_journal_size(
            "Archived and active journals take up 1.5G in the file system."
        ),
        (1.5 * 1024.0 * 1024.0 * 1024.0) as u64
    );

    assert_eq!(
        Scanner::parse_journal_size(
            "Archived and active journals take up 512K in the file system."
        ),
        512 * 1024
    );

    // Comma decimal separator support
    assert_eq!(
        Scanner::parse_journal_size(
            "Archived and active journals take up 16,6M in the file system."
        ),
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

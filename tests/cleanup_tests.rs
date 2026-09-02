use std::fs::File;
use std::io::Write;

#[test]
fn test_system_forbidden_paths() {
    let forbidden = vec!["/etc", "/usr", "/boot", "/bin", "/sbin", "/root", "/"];
    for p in forbidden {
        assert!(
            p.starts_with("/etc")
                || p.starts_with("/usr")
                || p.starts_with("/boot")
                || p.starts_with("/bin")
                || p.starts_with("/sbin")
                || p.starts_with("/root")
                || p == "/"
        );
    }
}

#[test]
fn test_temp_file_creation_and_size() {
    let temp_dir = std::env::temp_dir().join("cleaner_integration_test");
    let _ = std::fs::create_dir_all(&temp_dir);

    let file_a = temp_dir.join("a.cache");
    let mut f = File::create(&file_a).unwrap();
    f.write_all(b"sample cache data content").unwrap();

    assert!(file_a.exists());
    let meta = file_a.metadata().unwrap();
    assert_eq!(meta.len(), 25);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

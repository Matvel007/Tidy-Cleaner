use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Barrier, Mutex, MutexGuard,
};
use tidy_cleaner::app::state::AppState;
use tidy_cleaner::settings::{autostart::AppAutostartManager, AppSettings, SettingsStorage};
use tidy_cleaner::startup::desktop::{
    with_directory_lock, DesktopAutostart, APP_AUTOSTART_FILE_NAME,
};
use tidy_cleaner::startup::models::{CreateStartupRequest, StartupSource};
use tidy_cleaner::startup::scanner::StartupScanner;
use tidy_cleaner::theme::ThemeMode;

static ENVIRONMENT: Mutex<()> = Mutex::new(());
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

// This test executable owns every environment mutation; other suites are processes.
// HOME and both XDG paths are isolated, including icon lookup and system fallbacks.
struct Fixture {
    root: PathBuf,
    saved: Vec<(&'static str, Option<OsString>)>,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new() -> Self {
        let guard = ENVIRONMENT.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!(
            "tidy-regression-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for dir in [
            "home",
            "config/autostart",
            "system-first/autostart",
            "system-second/autostart",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let saved = ["HOME", "XDG_CONFIG_HOME", "XDG_CONFIG_DIRS"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect();
        std::env::set_var("HOME", root.join("home"));
        std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
        std::env::set_var(
            "XDG_CONFIG_DIRS",
            std::env::join_paths([root.join("system-first"), root.join("system-second")]).unwrap(),
        );
        Self {
            root,
            saved,
            _guard: guard,
        }
    }

    fn user(&self, name: &str) -> PathBuf {
        self.root.join("config/autostart").join(name)
    }
    fn system(&self, name: &str) -> PathBuf {
        self.root.join("system-first/autostart").join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn request(name: &str, exec: &str) -> CreateStartupRequest {
    CreateStartupRequest {
        name: name.into(),
        exec: exec.into(),
        comment: String::new(),
        icon: String::new(),
        terminal: false,
    }
}

#[test]
fn filename_collisions_preserve_original() {
    let fixture = Fixture::new();
    let path =
        DesktopAutostart::write_user_autostart_entry(&request("Backup Job", "/bin/true")).unwrap();
    let original = fs::read(&path).unwrap();
    for name in ["Backup Job", "Backup_Job", "BACKUP JOB"] {
        assert!(
            DesktopAutostart::write_user_autostart_entry(&request(name, "/bin/false")).is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    fs::write(
        fixture.system("system_job.desktop"),
        "[Desktop Entry]\nName=System Job\nExec=/bin/true\n",
    )
    .unwrap();
    assert!(
        DesktopAutostart::write_user_autostart_entry(&request("System Job", "/bin/false")).is_err()
    );
    assert!(!fixture.user("system_job.desktop").exists());
}

#[test]
fn toggle_updates_only_desktop_entry_group() {
    let fixture = Fixture::new();
    let path = fixture.user("multi.desktop");
    let action = "[Desktop Action Open]\nName=Open\nHidden=false\nX-GNOME-Autostart-enabled=true\nX-KDE-autostart-enabled=true\n";
    fs::write(
        &path,
        format!("[Desktop Entry]\nName=Multi\nExec=/bin/true\n{}", action),
    )
    .unwrap();
    let item = DesktopAutostart::parse_file(&path, StartupSource::User).unwrap();
    DesktopAutostart::toggle_entry(&item, false).unwrap();
    assert!(
        !DesktopAutostart::parse_file(&path, StartupSource::User)
            .unwrap()
            .enabled
    );
    assert!(fs::read_to_string(&path).unwrap().ends_with(action));
    DesktopAutostart::toggle_entry(&item, true).unwrap();
    assert!(
        DesktopAutostart::parse_file(&path, StartupSource::User)
            .unwrap()
            .enabled
    );
    assert!(fs::read_to_string(&path).unwrap().ends_with(action));
}

#[test]
fn disable_refresh_remove_keeps_system_override_disabled() {
    let fixture = Fixture::new();
    let system = fixture.system("system.desktop");
    let original = "[Desktop Entry]\nName=System\nExec=/bin/true\n";
    fs::write(&system, original).unwrap();
    let item = StartupScanner::scan_all().pop().unwrap();
    DesktopAutostart::toggle_entry(&item, false).unwrap();
    let refreshed = StartupScanner::scan_all().pop().unwrap();
    assert_eq!(refreshed.source, StartupSource::System);
    assert_eq!(refreshed.id, item.id);
    assert_eq!(refreshed.file_path, system);
    assert!(!refreshed.enabled);
    DesktopAutostart::remove_entry(&refreshed).unwrap();
    assert!(!StartupScanner::scan_all().pop().unwrap().enabled);
    assert_eq!(fs::read_to_string(system).unwrap(), original);
    assert!(fixture.user("system.desktop").is_file());
}

#[test]
fn xdg_first_system_directory_wins_under_user_override() {
    let fixture = Fixture::new();
    fs::write(
        fixture.system("priority.desktop"),
        "[Desktop Entry]\nName=First\nExec=/bin/true\n",
    )
    .unwrap();
    fs::write(
        fixture
            .root
            .join("system-second/autostart/priority.desktop"),
        "[Desktop Entry]\nName=Second\nExec=/bin/false\n",
    )
    .unwrap();
    let item = StartupScanner::scan_all().pop().unwrap();
    assert_eq!(item.name, "First");
    assert_eq!(item.exec, "/bin/true");
    fs::write(
        fixture.user("priority.desktop"),
        "[Desktop Entry]\nName=User Override\nHidden=true\n",
    )
    .unwrap();
    let item = StartupScanner::scan_all().pop().unwrap();
    assert_eq!(item.name, "User Override");
    assert_eq!(item.source, StartupSource::System);
    assert_eq!(item.file_path, fixture.system("priority.desktop"));
    assert!(!item.enabled);
}

#[test]
fn atomic_temporary_and_destination_symlinks_never_clobber_targets() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let target = fixture.root.join("atomic.json");
    let sentinel = fixture.root.join("sentinel");
    fs::write(&sentinel, "important").unwrap();
    symlink(&sentinel, fixture.root.join(".atomic.json.tmp")).unwrap();
    // Occupy several names in the new namespace, too. create_new must skip them.
    for sequence in 0..64 {
        symlink(
            &sentinel,
            fixture.root.join(format!(
                ".atomic.json.{}.{}.tmp",
                std::process::id(),
                sequence
            )),
        )
        .unwrap();
    }
    symlink(&sentinel, &target).unwrap();
    DesktopAutostart::atomic_write_file(&target, "replacement").unwrap();
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "important");
    assert_eq!(fs::read_to_string(&target).unwrap(), "replacement");
    assert!(!fs::symlink_metadata(target)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn concurrent_writers_publish_complete_files_and_reject_duplicate_create() {
    let fixture = Fixture::new();
    let path = fixture.root.join("concurrent.json");
    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                DesktopAutostart::atomic_write_file(&path, &i.to_string().repeat(65536)).unwrap();
                DesktopAutostart::write_user_autostart_entry(&request(
                    "Concurrent",
                    &format!("/bin/true {}", i),
                ))
                .is_ok()
            })
        })
        .collect();
    let successes = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .filter(|success| *success)
        .count();
    assert_eq!(successes, 1);
    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content.len(), 65536);
    assert!(content.bytes().all(|byte| byte == content.as_bytes()[0]));
    assert!(fs::read_dir(&fixture.root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn settings_save_is_atomic_and_reports_storage_errors() {
    let fixture = Fixture::new();
    let settings = AppSettings::default();
    SettingsStorage::save(&settings).unwrap();
    let path = SettingsStorage::config_path();
    let original = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(SettingsStorage::save(&settings).is_err());
    fs::remove_dir(&path).unwrap();
    fs::write(&path, original).unwrap();
    let state = AppState::new();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(state.set_theme(ThemeMode::Light).is_err());
    assert_eq!(state.get_theme(), ThemeMode::Dark);
    assert_eq!(
        DesktopAutostart::get_user_autostart_dir(),
        fixture.root.join("config/autostart")
    );
}

#[test]
fn disabled_file_is_authoritative_over_stale_settings_boolean() {
    let fixture = Fixture::new();
    AppAutostartManager::set_app_autostart(true, false).unwrap();
    let path = fixture.user(APP_AUTOSTART_FILE_NAME);
    let item = DesktopAutostart::parse_file(&path, StartupSource::User).unwrap();
    let state = AppState::new();
    state.settings.write().unwrap().autostart = true;
    DesktopAutostart::toggle_entry(&item, false).unwrap();
    let disabled = fs::read(&path).unwrap();
    AppAutostartManager::set_start_minimized(true).unwrap();
    assert_eq!(fs::read(&path).unwrap(), disabled);
    assert!(!AppAutostartManager::is_enabled().unwrap());
    state.set_start_minimized(true).unwrap();
    assert_eq!(fs::read(&path).unwrap(), disabled);
    assert!(state.settings.read().unwrap().start_minimized);
    state.reconcile_autostart().unwrap();
    assert!(!state.settings.read().unwrap().autostart);
    assert!(!SettingsStorage::load().autostart);
    fs::remove_file(&path).unwrap();
    AppAutostartManager::set_start_minimized(false).unwrap();
    assert!(!path.exists());
}

#[test]
fn own_autostart_disable_masks_system_fallback_and_reports_legacy_delete_error() {
    let fixture = Fixture::new();
    fs::write(
        fixture.system(APP_AUTOSTART_FILE_NAME),
        "[Desktop Entry]\nName=Tidy Cleaner\nExec=/bin/true\n",
    )
    .unwrap();
    assert!(AppAutostartManager::is_enabled().unwrap());
    AppAutostartManager::set_app_autostart(false, false).unwrap();
    assert!(!AppAutostartManager::is_enabled().unwrap());
    fs::create_dir(fixture.user("tidy_cleaner.desktop")).unwrap();
    assert!(AppAutostartManager::set_app_autostart(true, false).is_err());
    assert!(!AppAutostartManager::is_enabled().unwrap());
    let state = AppState::new();
    let updates = state.autostart_updates.subscribe();
    assert!(state.set_autostart(true).is_err());
    assert!(updates.has_changed().unwrap());
    assert!(!state.settings.read().unwrap().autostart);
}

#[test]
fn lock_file_symlink_is_rejected() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let sentinel = fixture.root.join("sentinel");
    fs::write(&sentinel, "important").unwrap();
    symlink(&sentinel, fixture.root.join(".tidy-cleaner.lock")).unwrap();
    assert!(with_directory_lock(&fixture.root, || Ok(())).is_err());
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "important");
}

#[test]
fn persistence_child_process() {
    let Some(root) = std::env::var_os("TIDY_PERSISTENCE_CHILD") else {
        return;
    };
    let root = PathBuf::from(root);
    for _ in 0..20 {
        with_directory_lock(&root, || {
            let path = root.join("counter");
            let counter: usize = fs::read_to_string(&path)?.parse().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
            fs::write(path, (counter + 1).to_string())?;
            Ok(())
        })
        .unwrap();
        DesktopAutostart::atomic_write_file(
            &root.join("shared"),
            &std::process::id().to_string().repeat(1024),
        )
        .unwrap();
    }
}

#[test]
fn cooperating_processes_serialize_conflicting_operations() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("counter"), "0").unwrap();
    let children: Vec<_> = (0..4)
        .map(|_| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "persistence_child_process", "--test-threads=1"])
                .env("TIDY_PERSISTENCE_CHILD", &fixture.root)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    assert_eq!(
        fs::read_to_string(fixture.root.join("counter")).unwrap(),
        "80"
    );
    let content = fs::read_to_string(fixture.root.join("shared")).unwrap();
    let pid_len = content.len() / 1024;
    assert_eq!(content, content[..pid_len].repeat(1024));
}

#[test]
fn settings_state_lock_is_not_held_during_slow_work_and_failed_save_reconciles_files() {
    let fixture = Fixture::new();
    let state = Arc::new(AppState::new());
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (finish_tx, finish_rx) = std::sync::mpsc::channel();
    let worker_state = state.clone();
    let worker = std::thread::spawn(move || {
        worker_state.update_settings(|_| {
            started_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            Ok(())
        })
    });
    started_rx.recv().unwrap();
    assert!(state.settings.try_write().is_ok());
    finish_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();

    AppAutostartManager::set_app_autostart(true, false).unwrap();
    let path = SettingsStorage::config_path();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(state.reconcile_autostart().is_err());
    assert!(state.settings.read().unwrap().autostart);
    assert!(fixture.user(APP_AUTOSTART_FILE_NAME).is_file());
}

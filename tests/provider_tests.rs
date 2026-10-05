use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use tidy_cleaner::applications::desktop_entries::{parse_exec, DesktopEntryRegistry};
use tidy_cleaner::applications::flatpak::{target_args, FlatpakProvider};
use tidy_cleaner::applications::models::{ApplicationItem, PackageSource};
use tidy_cleaner::applications::traits::PackageManagerProvider;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/tmp/opencode/tidy-provider-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn app(id: &str, source: PackageSource) -> ApplicationItem {
    ApplicationItem {
        id: id.into(),
        package_id: id.into(),
        name: "Firefox".into(),
        version: "1".into(),
        description: String::new(),
        source,
        icon: String::new(),
        icon_path: None,
        exec_cmd: Some("/bin/true".into()),
        installed_size_bytes: None,
        size_formatted: String::new(),
        desktop_file_path: None,
        is_desktop_app: true,
        selected: false,
    }
}

#[test]
fn full_flatpak_identity_survives_scope_arch_branch_and_named_installations() {
    let listing = "org.example.App/x86_64/stable\tuser\tExample\t1\tDescription\t1 MB\norg.example.App/x86_64/stable\tsystem\tExample\t2\tDescription\t2 MB\norg.example.App/aarch64/beta\twork\tExample\t3\tDescription\t3 MB\n";
    let apps = FlatpakProvider::parse_listing(listing).unwrap();
    assert_eq!(apps.len(), 3);
    let ids: std::collections::HashSet<_> = apps.iter().map(|a| &a.id).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(
        target_args(&apps[0].package_id).unwrap(),
        ("--user".into(), "app/org.example.App/x86_64/stable".into())
    );
    assert!(apps[1].exec_cmd.as_ref().unwrap().contains("--system"));
    assert!(apps[2]
        .exec_cmd
        .as_ref()
        .unwrap()
        .contains("--installation=work app/org.example.App/aarch64/beta"));
    assert!(apps.iter().all(|a| a.desktop_file_path.is_none()));
    for bad in [
        "org.example.App",
        "user|app/org.example.App",
        "user|runtime/org.example.App/x86_64/stable",
        "user|app/--bad/x86_64/stable",
        "bad scope|app/org.example.App/x86_64/stable",
    ] {
        assert!(target_args(bad).is_err(), "{bad}");
    }
}

#[test]
fn desktop_exec_preserves_quoting_and_expands_fields_without_a_shell() {
    let args = parse_exec(
        r#""/path with spaces/app" "two words" "" %c %k %i %U --literal=%% $(touch)"#,
        "Display Name",
        "app-icon",
        Path::new("/tmp/app.desktop"),
    )
    .unwrap();
    assert_eq!(
        args,
        [
            "/path with spaces/app",
            "two words",
            "",
            "Display Name",
            "/tmp/app.desktop",
            "--icon",
            "app-icon",
            "--literal=%",
            "$(touch)"
        ]
    );
    assert_eq!(
        parse_exec(r#"app "say \"hello\"""#, "", "", Path::new("")).unwrap()[1],
        "say \"hello\""
    );
    for bad in [
        "",
        "\"unterminated",
        "app %unknown",
        "app --file=%f",
        "app\nExec=bad",
        "app trailing\\",
    ] {
        assert!(parse_exec(bad, "", "", Path::new("")).is_err(), "{bad}");
    }
}

#[test]
fn desktop_precedence_hidden_overrides_visibility_and_nested_ids() {
    let fixture = Fixture::new();
    fixture.write(
        "first/browser.desktop",
        "[Desktop Entry]\nType=Application\nName=Hidden override\nHidden=true\nExec=/bin/true\n",
    );
    fixture.write(
        "second/browser.desktop",
        "[Desktop Entry]\nType=Application\nName=Wrong lower entry\nExec=/bin/true\n",
    );
    fixture.write(
        "first/tools/editor.desktop",
        "[Desktop Entry]\nType=Application\nName=Editor\nExec=/bin/true\n",
    );
    fixture.write(
        "second/tools/editor.desktop",
        "[Desktop Entry]\nType=Application\nName=Wrong editor\nExec=/bin/true\n",
    );
    fixture.write("first/missing.desktop", "[Desktop Entry]\nType=Application\nName=Missing\nTryExec=/tmp/opencode/nonexistent-tryexec\nExec=/bin/true\n");
    fixture.write("first/other.desktop", "[Desktop Entry]\nType=Application\nName=Other\nOnlyShowIn=NoSuchTidyTestDesktop;\nExec=/bin/true\n");
    let entries = DesktopEntryRegistry::scan_directories(&[
        fixture.0.join("first"),
        fixture.0.join("second"),
    ])
    .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries["tools-editor.desktop"].name, "Editor");
}

#[test]
fn shortcuts_are_identity_scoped_exclusive_and_preserve_symlinks() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let native = app("pacman:firefox", PackageSource::Pacman);
    let flatpak = app(
        "flatpak:user|app/org.mozilla.firefox/x86_64/stable",
        PackageSource::Flatpak,
    );
    let original = DesktopEntryRegistry::create_shortcut_in(&native, &fixture.0).unwrap();
    let flatpak_path = DesktopEntryRegistry::create_shortcut_in(&flatpak, &fixture.0).unwrap();
    assert_ne!(original, flatpak_path);
    let bytes = fs::read(&original).unwrap();
    let second = DesktopEntryRegistry::create_shortcut_in(&native, &fixture.0).unwrap();
    assert_ne!(original, second);
    assert_eq!(fs::read(&original).unwrap(), bytes);
    let victim = fixture.write("victim", "sentinel");
    fs::remove_file(&original).unwrap();
    symlink(&victim, &original).unwrap();
    let third = DesktopEntryRegistry::create_shortcut_in(&native, &fixture.0).unwrap();
    assert_ne!(third, original);
    assert!(fs::symlink_metadata(&original)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(victim).unwrap(), "sentinel");
    fs::remove_file(&original).unwrap();
    symlink(fixture.0.join("missing-target"), &original).unwrap();
    assert_ne!(
        DesktopEntryRegistry::create_shortcut_in(&native, &fixture.0).unwrap(),
        original
    );
    assert!(fs::symlink_metadata(&original)
        .unwrap()
        .file_type()
        .is_symlink());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let app = native.clone();
            let dir = fixture.0.clone();
            std::thread::spawn(move || {
                DesktopEntryRegistry::create_shortcut_in(&app, &dir).unwrap()
            })
        })
        .collect();
    let paths: std::collections::HashSet<_> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(paths.len(), 8);
}

#[test]
fn providers_use_fake_commands_only_in_isolated_child() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let scripts = [
        (
            "pacman",
            r#"#!/bin/sh
case "$1" in
 --version) exit 0;;
 -Qqo) case "$3" in *firefox.desktop) printf 'firefox\n';; *) exit 1;; esac;;
 -Qn) printf 'firefox 1\nflatpak 2\n';;
 -Qen) printf 'firefox 1\nflatpak 2\n';;
 -Qm) exit 1;;
 -Qi) printf 'fake details\n';;
 *) exit 99;;
esac
"#,
        ),
        (
            "dpkg-query",
            r#"#!/bin/sh
case "$1" in
 --version) exit 0;;
 -S) printf 'firefox: %s\n' "$2";;
 -W) printf '%s\t%s\tinstall ok installed\n' "${FAKE_ESSENTIAL:-no}" "${FAKE_PROTECTED:-no}";;
 *) exit 99;;
esac
"#,
        ),
        (
            "flatpak",
            r#"#!/bin/sh
case "$1" in
 --version) exit 0;;
 list) printf 'org.example.App/x86_64/stable\tuser\tExample\t1\tDescription\t1 MB\norg.example.App/aarch64/beta\twork\tExample\t2\tDescription\t2 MB\n';;
 info) printf 'fake details\n';;
 uninstall) printf 'flatpak\n' >> "$FAKE_LOG"; printf '%s\n' "$@" >> "$FAKE_LOG";;
 *) exit 99;;
esac
"#,
        ),
        (
            "snap",
            r#"#!/bin/sh
case "$1" in
 --version) exit 0;;
 list) printf 'Name Version Rev Tracking Publisher Notes\ngnome-calculator 1 1 stable fake -\ngnome-runtime 1 1 stable fake -\n';;
 info) printf 'fake details\n';;
 *) exit 99;;
esac
"#,
        ),
        (
            "pkexec",
            r#"#!/bin/sh
printf 'pkexec\n' >> "$FAKE_LOG"
printf '%s\n' "$@" >> "$FAKE_LOG"
if [ "$FAKE_DENY" = 1 ]; then printf 'fake denied\n' >&2; exit 126; fi
"#,
        ),
        ("gtk-launch", "#!/bin/sh\nif [ \"$FAKE_GTK_SUCCESS\" = 1 ]; then /bin/sleep 30 & printf '%s' \"$!\" > \"$FAKE_GTK_PID\"; exit 0; fi\nexit 1\n"),
        (
            "fake-launch",
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_LAUNCH_LOG\"\n",
        ),
    ];
    for (name, script) in scripts {
        let path = fixture.write(&format!("bin/{name}"), script);
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fixture.write("data/applications/firefox.desktop", "[Desktop Entry]\nType=Application\nName=Native Firefox\nName[fr]=Navigateur\nExec=another-browser-binary %U\n");
    fixture.write("data/applications/org.example.App.desktop", "[Desktop Entry]\nType=Application\nName=Unrelated Flatpak\nExec=/usr/bin/flatpak run org.example.App\nX-Flatpak=org.example.App\n");
    fixture.write("data/applications/sneaky.desktop", "[Desktop Entry]\nType=Application\nName=Wrapper without metadata\nExec=/usr/bin/flatpak run org.example.App\n");
    fixture.write("data/applications/gnome-calculator_calc.desktop", "[Desktop Entry]\nType=Application\nName=Calculator\nExec=env BAMF_DESKTOP_FILE_HINT=/tmp/calculator.desktop /snap/bin/gnome-calculator.calc %U\nX-SnapInstanceName=gnome-calculator\n");
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fake_provider_child", "--nocapture"])
        .env("TIDY_PROVIDER_FIXTURE", &fixture.0)
        .env("PATH", fixture.0.join("bin"))
        .env("HOME", &fixture.0)
        .env("XDG_DATA_HOME", fixture.0.join("data"))
        .env("XDG_DATA_DIRS", fixture.0.join("empty-system"))
        .env("LC_ALL", "fr_FR.UTF-8")
        .env("FAKE_LOG", fixture.0.join("commands.log"))
        .env("FAKE_LAUNCH_LOG", fixture.0.join("launch.log"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn fake_provider_child() {
    let Ok(root) = std::env::var("TIDY_PROVIDER_FIXTURE") else {
        return;
    };
    use tidy_cleaner::applications::{
        aur::AurProvider, dpkg::DpkgProvider, manager::ApplicationManager, pacman::PacmanProvider,
        rpm::RpmProvider, snap::SnapProvider,
    };
    let root = PathBuf::from(root);
    let native = PacmanProvider::new().list_installed().unwrap();
    let firefox = native.iter().find(|a| a.package_id == "firefox").unwrap();
    assert_eq!(firefox.name, "Navigateur");
    assert!(firefox
        .exec_cmd
        .as_ref()
        .unwrap()
        .starts_with("another-browser-binary"));
    assert!(native
        .iter()
        .all(|a| !a.name.contains("Flatpak") && !a.name.contains("Wrapper")));
    assert!(AurProvider::new().list_installed().unwrap().is_empty());
    PacmanProvider::new().uninstall("firefox").unwrap();
    AurProvider::new().uninstall("foreign-bin").unwrap();
    assert!(PacmanProvider::new().uninstall("--recursive").is_err());
    DpkgProvider::new().uninstall("firefox:amd64").unwrap();
    std::env::set_var("FAKE_PROTECTED", "yes");
    assert!(DpkgProvider::new().uninstall("protected").is_err());
    std::env::remove_var("FAKE_PROTECTED");
    std::env::set_var("FAKE_ESSENTIAL", "yes");
    assert!(DpkgProvider::new().uninstall("essential").is_err());
    std::env::remove_var("FAKE_ESSENTIAL");
    let rpm_error = RpmProvider::new()
        .uninstall("firefox")
        .unwrap_err()
        .to_string();
    assert!(rpm_error.contains("read-only") && rpm_error.contains("OSTree"));
    let flatpak = FlatpakProvider::new().list_installed().unwrap();
    for app in &flatpak {
        FlatpakProvider::new().get_details(&app.package_id).unwrap();
        FlatpakProvider::new().uninstall(&app.package_id).unwrap();
    }
    let snaps = SnapProvider::new().list_installed().unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].package_id, "gnome-calculator");
    assert!(snaps[0]
        .exec_cmd
        .as_ref()
        .unwrap()
        .contains("/snap/bin/gnome-calculator.calc"));
    std::env::set_var("FAKE_DENY", "1");
    assert!(PacmanProvider::new().uninstall("denied").is_err());
    let commands = fs::read_to_string(root.join("commands.log")).unwrap();
    assert!(commands.contains("pacman\n-R\n--noconfirm\n--\nfirefox"));
    assert!(commands.contains("pacman\n-R\n--noconfirm\n--\nforeign-bin"));
    assert!(commands.contains("dpkg\n--no-force-all\n--remove\n--\nfirefox:amd64"));
    assert!(!commands.contains("protected") && !commands.contains("essential"));
    assert!(
        !commands.contains("-Rns")
            && !commands.contains("apt-get")
            && !commands.contains("dnf")
            && !commands.contains("--nodeps")
            && !commands.contains("zypper")
    );
    assert!(commands.contains("--user\napp/org.example.App/x86_64/stable"));
    assert!(commands.contains("--installation=work\napp/org.example.App/aarch64/beta"));
    let mut launched = app("launch", PackageSource::Pacman);
    let desktop = root.join("data/applications/firefox.desktop");
    launched.desktop_file_path = Some(desktop);
    launched.exec_cmd = Some("fake-launch \"two words\" \"$(not-a-shell)\" %U".into());
    ApplicationManager::launch_app(&launched).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("launch.log")).unwrap(),
        "two words\n$(not-a-shell)\n"
    );
    launched.exec_cmd = Some("/bin/false".into());
    assert!(ApplicationManager::launch_app(&launched).is_err());
    std::env::set_var("FAKE_GTK_SUCCESS", "1");
    let gtk_pid_file = root.join("gtk.pid");
    std::env::set_var("FAKE_GTK_PID", &gtk_pid_file);
    launched.exec_cmd = Some("/tmp/opencode/no-fallback-executable".into());
    let start = std::time::Instant::now();
    let result = ApplicationManager::launch_app(&launched);
    let pid: i32 = fs::read_to_string(gtk_pid_file).unwrap().parse().unwrap();
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    assert!(result.is_ok() && alive);
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

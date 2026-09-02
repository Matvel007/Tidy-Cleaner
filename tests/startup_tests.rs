use std::fs;
use tidy_cleaner::startup::desktop::DesktopAutostart;
use tidy_cleaner::startup::models::{CreateStartupRequest, StartupSource};

#[test]
fn test_validate_startup_request() {
    let empty_name = CreateStartupRequest {
        name: "".to_string(),
        exec: "/usr/bin/discord".to_string(),
        comment: "".to_string(),
        icon: "".to_string(),
        terminal: false,
    };
    assert!(DesktopAutostart::validate_request(&empty_name).is_err());

    let empty_exec = CreateStartupRequest {
        name: "Discord".to_string(),
        exec: "   ".to_string(),
        comment: "".to_string(),
        icon: "".to_string(),
        terminal: false,
    };
    assert!(DesktopAutostart::validate_request(&empty_exec).is_err());

    let valid_req = CreateStartupRequest {
        name: "Discord".to_string(),
        exec: "/usr/bin/discord".to_string(),
        comment: "All-in-one voice and text chat".to_string(),
        icon: "discord".to_string(),
        terminal: false,
    };
    assert!(DesktopAutostart::validate_request(&valid_req).is_ok());
}

#[test]
fn test_generate_and_parse_desktop_autostart() {
    let req = CreateStartupRequest {
        name: "My Custom Script".to_string(),
        exec: "/home/user/myscript.sh --daemon".to_string(),
        comment: "Background backup script".to_string(),
        icon: "utilities-terminal".to_string(),
        terminal: true,
    };

    let content = DesktopAutostart::generate_desktop_file_content(&req);
    assert!(content.contains("[Desktop Entry]"));
    assert!(content.contains("Name=My Custom Script"));
    assert!(content.contains("Exec=/home/user/myscript.sh --daemon"));
    assert!(content.contains("Comment=Background backup script"));
    assert!(content.contains("Icon=utilities-terminal"));
    assert!(content.contains("Terminal=true"));
    assert!(content.contains("X-GNOME-Autostart-enabled=true"));
    assert!(content.contains("X-KDE-autostart-enabled=true"));

    let dir = std::env::temp_dir().join(format!("tidy_test_startup_{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let file_path = dir.join("my_custom_script.desktop");
    fs::write(&file_path, content).unwrap();

    let parsed = DesktopAutostart::parse_file(&file_path, StartupSource::User).unwrap();
    assert_eq!(parsed.name, "My Custom Script");
    assert_eq!(parsed.exec, "/home/user/myscript.sh --daemon");
    assert_eq!(parsed.comment, "Background backup script");
    assert_eq!(parsed.icon, "utilities-terminal");
    assert!(parsed.is_terminal);
    assert!(parsed.enabled);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_parse_disabled_autostart_entry() {
    let content = "[Desktop Entry]\n\
        Type=Application\n\
        Name=Disabled App\n\
        Exec=/usr/bin/disabled\n\
        Hidden=true\n\
        X-GNOME-Autostart-enabled=false\n";

    let dir = std::env::temp_dir().join(format!("tidy_test_disabled_{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let file_path = dir.join("disabled.desktop");
    fs::write(&file_path, content).unwrap();

    let parsed = DesktopAutostart::parse_file(&file_path, StartupSource::System).unwrap();
    assert_eq!(parsed.name, "Disabled App");
    assert!(!parsed.enabled);

    let _ = fs::remove_dir_all(&dir);
}

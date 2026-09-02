use std::fs;
use std::path::Path;

#[test]
fn test_desktop_file_parsing() {
    let test_dir = std::env::temp_dir().join(format!("tidy_test_dir_{}", std::process::id()));
    let _ = fs::create_dir_all(&test_dir);
    let desktop_file = test_dir.join("test-app.desktop");

    let content = "[Desktop Entry]\n\
        Type=Application\n\
        Name=Test Editor\n\
        Exec=test-editor %U\n\
        Icon=text-editor\n\
        Comment=A test editor application\n\
        Categories=Utility;TextEditor;\n";

    fs::write(&desktop_file, content).expect("Failed to write test desktop file");

    let parsed = parse_test_desktop(&desktop_file).expect("Failed to parse desktop file");
    assert_eq!(parsed.name, "Test Editor");
    assert_eq!(parsed.exec, "test-editor");
    assert_eq!(parsed.icon, "text-editor");
    assert_eq!(parsed.comment, "A test editor application");

    let _ = fs::remove_dir_all(&test_dir);
}

#[derive(Debug, PartialEq)]
struct TestDesktopInfo {
    name: String,
    exec: String,
    icon: String,
    comment: String,
}

fn parse_test_desktop(path: &Path) -> anyhow::Result<TestDesktopInfo> {
    let content = fs::read_to_string(path)?;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut comment = String::new();

    for line in content.lines() {
        let line = line.trim();
        if let Some((key, val)) = line.split_once('=') {
            match key.trim() {
                "Name" if name.is_empty() => name = val.trim().to_string(),
                "Exec" if exec.is_empty() => {
                    let cleaned = val
                        .split_whitespace()
                        .filter(|w| !w.starts_with('%'))
                        .collect::<Vec<_>>()
                        .join(" ");
                    exec = cleaned;
                }
                "Icon" if icon.is_empty() => icon = val.trim().to_string(),
                "Comment" if comment.is_empty() => comment = val.trim().to_string(),
                _ => {}
            }
        }
    }

    Ok(TestDesktopInfo {
        name,
        exec,
        icon,
        comment,
    })
}

#[test]
fn test_pagination_logic() {
    let items: Vec<i32> = (1..=25).collect();
    let page_size = 10;

    // Page 1
    let total_pages = items.len().div_ceil(page_size);
    assert_eq!(total_pages, 3);

    let start = 0;
    let end = (start + page_size).min(items.len());
    assert_eq!(&items[start..end], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    // Page 3
    let start_p3 = 20;
    let end_p3 = (start_p3 + page_size).min(items.len());
    assert_eq!(&items[start_p3..end_p3], &[21, 22, 23, 24, 25]);
}

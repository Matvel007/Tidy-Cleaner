use tidy_cleaner::applications::manager::ApplicationManager;
use tidy_cleaner::applications::models::{ApplicationItem, PackageSource};

fn create_sample_app(id: &str, name: &str, source: PackageSource, desc: &str) -> ApplicationItem {
    ApplicationItem {
        id: id.to_string(),
        package_id: id.to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        description: desc.to_string(),
        source,
        icon: "application-x-executable".to_string(),
        icon_path: None,
        exec_cmd: None,
        installed_size_bytes: Some(1024 * 1024),
        size_formatted: "1 MB".to_string(),
        desktop_file_path: None,
        is_desktop_app: true,
        selected: false,
    }
}

#[test]
fn test_application_filtering_by_query_and_source() {
    let apps = vec![
        create_sample_app("firefox", "Firefox Web Browser", PackageSource::Pacman, "Fast browser"),
        create_sample_app("code", "Visual Studio Code", PackageSource::Flatpak, "Code editor"),
        create_sample_app("discord", "Discord", PackageSource::Flatpak, "Chat platform"),
        create_sample_app("git", "Git", PackageSource::Pacman, "Version control"),
    ];

    // Filter by query
    let filtered_query = ApplicationManager::filter_apps(&apps, "browser", None);
    assert_eq!(filtered_query.len(), 1);
    assert_eq!(filtered_query[0].name, "Firefox Web Browser");

    // Filter by source
    let filtered_source = ApplicationManager::filter_apps(&apps, "", Some(PackageSource::Flatpak));
    assert_eq!(filtered_source.len(), 2);
    assert!(filtered_source.iter().all(|a| a.source == PackageSource::Flatpak));

    // Combined query + source filter
    let filtered_both = ApplicationManager::filter_apps(&apps, "code", Some(PackageSource::Flatpak));
    assert_eq!(filtered_both.len(), 1);
    assert_eq!(filtered_both[0].id, "code");
}

#[test]
fn test_application_pagination() {
    let apps: Vec<ApplicationItem> = (1..=25)
        .map(|i| {
            create_sample_app(
                &format!("app_{}", i),
                &format!("App {}", i),
                PackageSource::Pacman,
                "Sample description",
            )
        })
        .collect();

    let page_size = 10;

    // Page 1
    let (p1, cur_p1, total_pages) = ApplicationManager::paginate_apps(&apps, 1, page_size);
    assert_eq!(cur_p1, 1);
    assert_eq!(total_pages, 3);
    assert_eq!(p1.len(), 10);
    assert_eq!(p1[0].id, "app_1");
    assert_eq!(p1[9].id, "app_10");

    // Page 3
    let (p3, cur_p3, _) = ApplicationManager::paginate_apps(&apps, 3, page_size);
    assert_eq!(cur_p3, 3);
    assert_eq!(p3.len(), 5);
    assert_eq!(p3[0].id, "app_21");
    assert_eq!(p3[4].id, "app_25");

    // Clamp out-of-range page
    let (p_clamped, cur_clamped, _) = ApplicationManager::paginate_apps(&apps, 999, page_size);
    assert_eq!(cur_clamped, 3);
    assert_eq!(p_clamped.len(), 5);
}

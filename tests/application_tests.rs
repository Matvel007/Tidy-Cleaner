use std::sync::{Arc, Mutex};
use tidy_cleaner::applications::manager::ApplicationManager;
use tidy_cleaner::applications::models::{ApplicationItem, PackageSource};
use tidy_cleaner::applications::service::ApplicationService;
use tidy_cleaner::applications::traits::PackageManagerProvider;

struct FakeProvider {
    apps: Vec<ApplicationItem>,
    fail_scan: std::sync::atomic::AtomicBool,
    calls: Mutex<Vec<String>>,
    block: Option<Arc<(Mutex<bool>, std::sync::Condvar)>>,
}

impl PackageManagerProvider for FakeProvider {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn source(&self) -> PackageSource {
        self.apps.first().unwrap().source
    }
    fn is_available(&self) -> bool {
        true
    }
    fn list_installed(&self) -> anyhow::Result<Vec<ApplicationItem>> {
        if self.fail_scan.load(std::sync::atomic::Ordering::SeqCst) {
            anyhow::bail!("fake discovery failure");
        }
        Ok(self.apps.clone())
    }
    fn uninstall(&self, id: &str) -> anyhow::Result<()> {
        self.calls.lock().unwrap().push(id.to_string());
        if let Some(block) = &self.block {
            let (lock, cv) = &**block;
            let mut ready = lock.lock().unwrap();
            while !*ready {
                ready = cv.wait(ready).unwrap();
            }
        }
        if id == "panic" {
            panic!("fake mutation worker panic");
        }
        if id == "failure" {
            anyhow::bail!("fake authorization failure");
        }
        Ok(())
    }
    fn get_details(&self, _: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
}

fn fake_provider(apps: Vec<ApplicationItem>) -> Arc<FakeProvider> {
    Arc::new(FakeProvider {
        apps,
        fail_scan: std::sync::atomic::AtomicBool::new(false),
        calls: Mutex::new(Vec::new()),
        block: None,
    })
}

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
        create_sample_app(
            "firefox",
            "Firefox Web Browser",
            PackageSource::Pacman,
            "Fast browser",
        ),
        create_sample_app(
            "code",
            "Visual Studio Code",
            PackageSource::Flatpak,
            "Code editor",
        ),
        create_sample_app(
            "discord",
            "Discord",
            PackageSource::Flatpak,
            "Chat platform",
        ),
        create_sample_app("git", "Git", PackageSource::Pacman, "Version control"),
    ];

    // Filter by query
    let filtered_query = ApplicationManager::filter_apps(&apps, "browser", None);
    assert_eq!(filtered_query.len(), 1);
    assert_eq!(filtered_query[0].name, "Firefox Web Browser");

    // Filter by source
    let filtered_source = ApplicationManager::filter_apps(&apps, "", Some(PackageSource::Flatpak));
    assert_eq!(filtered_source.len(), 2);
    assert!(filtered_source
        .iter()
        .all(|a| a.source == PackageSource::Flatpak));

    // Combined query + source filter
    let filtered_both =
        ApplicationManager::filter_apps(&apps, "code", Some(PackageSource::Flatpak));
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

#[test]
fn pagination_zero_and_extreme_sizes_do_not_panic() {
    let apps = vec![create_sample_app("one", "One", PackageSource::Pacman, "")];
    assert_eq!(
        ApplicationManager::paginate_apps(&apps, usize::MAX, 0)
            .0
            .len(),
        1
    );
    assert_eq!(
        ApplicationManager::paginate_apps(&apps, usize::MAX, usize::MAX)
            .0
            .len(),
        1
    );
    assert!(ApplicationManager::paginate_apps(&[], 0, 0).0.is_empty());
}

#[tokio::test]
async fn resizing_preserves_exact_first_visible_offset_and_selection() {
    let provider = fake_provider(
        (0..60)
            .map(|i| {
                create_sample_app(
                    &format!("{i:02}"),
                    &format!("App {i:02}"),
                    PackageSource::Pacman,
                    "",
                )
            })
            .collect(),
    );
    let service =
        ApplicationService::with_manager(ApplicationManager::with_providers(vec![provider]));
    service.refresh_installed_apps().await;
    service.set_page(3);
    service.toggle_app_selection("20").await;
    assert_eq!(service.get_current_view().await.0[0].id, "20");
    service.set_page_size(15);
    let (view, page, pages, _) = service.get_current_view().await;
    assert_eq!((view.len(), page, pages), (15, 2, 4));
    assert_eq!(view[0].id, "20");
    assert!(view[0].selected);
    service.set_page_size(6);
    assert_eq!(service.get_current_view().await.0[0].id, "20");
    service.set_search_query("App 59".into());
    assert_eq!(service.get_current_view().await.0[0].id, "59");
    service.set_page_size(0);
    assert_eq!(service.get_current_view().await.0.len(), 1);
}

#[tokio::test]
async fn independent_provider_failure_preserves_last_good_results() {
    let native = fake_provider(vec![create_sample_app(
        "native",
        "Native",
        PackageSource::Pacman,
        "",
    )]);
    let flatpak = fake_provider(vec![create_sample_app(
        "flatpak",
        "Flatpak",
        PackageSource::Flatpak,
        "",
    )]);
    let service = ApplicationService::with_manager(ApplicationManager::with_providers(vec![
        native.clone(),
        flatpak.clone(),
    ]));
    service.refresh_installed_apps().await;
    service.toggle_app_selection("native").await;
    native
        .fail_scan
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let apps = service.refresh_installed_apps().await;
    assert_eq!(apps.len(), 2);
    assert!(apps.iter().find(|app| app.id == "native").unwrap().selected);
    assert!(apps.iter().any(|app| app.id == "flatpak"));
}

#[tokio::test]
async fn single_failure_is_returned_and_terminal_progress_keeps_error() {
    let provider = fake_provider(vec![create_sample_app(
        "failure",
        "Failure",
        PackageSource::Pacman,
        "",
    )]);
    let service =
        ApplicationService::with_manager(ApplicationManager::with_providers(vec![provider]));
    service.refresh_installed_apps().await;
    let (mut rx, handle) = service.uninstall_single_app("failure").await.unwrap();
    assert!(handle
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("authorization"));
    let mut terminal = None;
    while let Ok(progress) = rx.try_recv() {
        if progress.is_completed {
            terminal = Some(progress);
        }
    }
    assert!(terminal
        .unwrap()
        .error_message
        .unwrap()
        .contains("authorization"));
    assert_eq!(service.get_cached_apps().await.len(), 1);
    assert!(service.uninstall_single_app("missing").await.is_err());
}

#[tokio::test]
async fn batch_partial_failure_removes_only_successes_from_cache() {
    let provider = fake_provider(vec![
        create_sample_app("success", "Success", PackageSource::Pacman, ""),
        create_sample_app("failure", "Failure", PackageSource::Pacman, ""),
    ]);
    let service =
        ApplicationService::with_manager(ApplicationManager::with_providers(
            vec![provider.clone()],
        ));
    service.refresh_installed_apps().await;
    service.select_all().await;
    let (mut rx, handle) = service.uninstall_selected().await.unwrap();
    assert!(handle.await.unwrap().is_err());
    assert_eq!(provider.calls.lock().unwrap().len(), 2);
    let cached = service.get_cached_apps().await;
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].id, "failure");
    assert!(cached[0].selected);
    let mut terminal = None;
    while let Ok(progress) = rx.try_recv() {
        if progress.is_completed {
            terminal = Some(progress);
        }
    }
    assert!(terminal.unwrap().error_message.is_some());
}

#[tokio::test]
async fn worker_panic_is_not_success() {
    let provider = fake_provider(vec![create_sample_app(
        "panic",
        "Panic",
        PackageSource::Pacman,
        "",
    )]);
    let service =
        ApplicationService::with_manager(ApplicationManager::with_providers(vec![provider]));
    service.refresh_installed_apps().await;
    let (mut rx, handle) = service.uninstall_single_app("panic").await.unwrap();
    assert!(handle.await.unwrap().is_err());
    let mut terminal = None;
    while let Ok(progress) = rx.try_recv() {
        if progress.is_completed {
            terminal = Some(progress);
        }
    }
    assert!(terminal.unwrap().error_message.unwrap().contains("panic"));
}

#[tokio::test]
async fn overlapping_mutations_are_rejected_and_receiver_drop_does_not_unlock() {
    let block = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let provider = Arc::new(FakeProvider {
        apps: vec![create_sample_app("one", "One", PackageSource::Pacman, "")],
        fail_scan: std::sync::atomic::AtomicBool::new(false),
        calls: Mutex::new(Vec::new()),
        block: Some(block.clone()),
    });
    let service =
        ApplicationService::with_manager(ApplicationManager::with_providers(
            vec![provider.clone()],
        ));
    service.refresh_installed_apps().await;
    let (rx, handle) = service.uninstall_single_app("one").await.unwrap();
    let rejected = service.uninstall_single_app("one").await;
    // Release the fake worker before asserting, so a test failure cannot leave
    // an indefinitely blocked runtime shutdown.
    {
        let (lock, cv) = &*block;
        *lock.lock().unwrap() = true;
        cv.notify_all();
    }
    assert!(rejected.is_err());
    drop(rx);
    assert!(handle.await.unwrap().is_ok());
    assert_eq!(provider.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn aborting_async_observer_does_not_release_live_mutation_guard() {
    let block = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let provider = Arc::new(FakeProvider {
        apps: vec![create_sample_app("one", "One", PackageSource::Pacman, "")],
        fail_scan: std::sync::atomic::AtomicBool::new(false),
        calls: Mutex::new(Vec::new()),
        block: Some(block.clone()),
    });
    let service =
        ApplicationService::with_manager(ApplicationManager::with_providers(
            vec![provider.clone()],
        ));
    service.refresh_installed_apps().await;
    let (_, handle) = service.uninstall_single_app("one").await.unwrap();
    let started = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while provider.calls.lock().unwrap().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await;
    handle.abort();
    let cancelled = handle.await;
    let rejected = service.uninstall_single_app("one").await;
    {
        let (lock, cv) = &*block;
        *lock.lock().unwrap() = true;
        cv.notify_all();
    }
    assert!(started.is_ok());
    assert!(rejected.is_err());
    assert!(cancelled.unwrap_err().is_cancelled());
    assert_eq!(provider.calls.lock().unwrap().len(), 1);
}

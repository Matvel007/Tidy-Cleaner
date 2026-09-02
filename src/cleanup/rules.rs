use crate::cleanup::models::{CleanupCategory, CleanupRule, RiskLevel};
use crate::filesystem::xdg::{
    get_cache_dir, get_config_dir, get_data_dir, get_user_download_dir, home_dir,
};

pub struct RuleRegistry;

impl RuleRegistry {
    pub fn get_default_rules() -> Vec<CleanupRule> {
        let home = home_dir();
        let cache_dir = get_cache_dir();
        let config_dir = get_config_dir();
        let data_dir = get_data_dir();

        let download_dir = get_user_download_dir();

        vec![
            // --- FAST SCAN RULES (Safe) ---
            // 1. General Thumbnails Cache
            CleanupRule {
                id: "thumbnails".to_string(),
                name_key: "cleanup.rule.thumbnails".to_string(),
                description_key: "cleanup.rule.thumbnails.desc".to_string(),
                category: CleanupCategory::Thumbnails,
                base_path: cache_dir.join("thumbnails"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            // 2. Browsers
            CleanupRule {
                id: "firefox_cache".to_string(),
                name_key: "cleanup.rule.firefox".to_string(),
                description_key: "cleanup.rule.firefox.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("mozilla/firefox"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "chrome_cache".to_string(),
                name_key: "cleanup.rule.chrome".to_string(),
                description_key: "cleanup.rule.chrome.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("google-chrome"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "chromium_cache".to_string(),
                name_key: "cleanup.rule.chromium".to_string(),
                description_key: "cleanup.rule.chromium.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("chromium"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "brave_cache".to_string(),
                name_key: "cleanup.rule.brave".to_string(),
                description_key: "cleanup.rule.brave.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("BraveSoftware"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "edge_cache".to_string(),
                name_key: "cleanup.rule.edge".to_string(),
                description_key: "cleanup.rule.edge.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("microsoft-edge"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "opera_cache".to_string(),
                name_key: "cleanup.rule.opera".to_string(),
                description_key: "cleanup.rule.opera.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("opera"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "vivaldi_cache".to_string(),
                name_key: "cleanup.rule.vivaldi".to_string(),
                description_key: "cleanup.rule.vivaldi.desc".to_string(),
                category: CleanupCategory::BrowserCache,
                base_path: cache_dir.join("vivaldi"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            // 3. User Trash Bin (Warning level so user explicitly confirms)
            CleanupRule {
                id: "user_trash".to_string(),
                name_key: "cleanup.rule.trash".to_string(),
                description_key: "cleanup.rule.trash.desc".to_string(),
                category: CleanupCategory::Trash,
                base_path: data_dir.join("Trash/files"),
                is_deep_scan: false,
                safety_level: RiskLevel::Warning,
            },
            // 4. Messaging & Media
            CleanupRule {
                id: "discord_cache".to_string(),
                name_key: "cleanup.rule.discord".to_string(),
                description_key: "cleanup.rule.discord.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: config_dir.join("discord/Cache"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "telegram_cache".to_string(),
                name_key: "cleanup.rule.telegram".to_string(),
                description_key: "cleanup.rule.telegram.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: data_dir.join("TelegramDesktop/tdata/user_data/cache"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "spotify_cache".to_string(),
                name_key: "cleanup.rule.spotify".to_string(),
                description_key: "cleanup.rule.spotify.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: cache_dir.join("spotify"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "steam_shader_cache".to_string(),
                name_key: "cleanup.rule.steam".to_string(),
                description_key: "cleanup.rule.steam.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: data_dir.join("Steam/steamapps/shadercache"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            // 5. Package Managers (Arch, Debian, Ubuntu, Fedora, Flatpak, Snap)
            CleanupRule {
                id: "yay_cache".to_string(),
                name_key: "cleanup.rule.yay".to_string(),
                description_key: "cleanup.rule.yay.desc".to_string(),
                category: CleanupCategory::PackageCache,
                base_path: cache_dir.join("yay"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "paru_cache".to_string(),
                name_key: "cleanup.rule.paru".to_string(),
                description_key: "cleanup.rule.paru.desc".to_string(),
                category: CleanupCategory::PackageCache,
                base_path: cache_dir.join("paru"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "flatpak_cache".to_string(),
                name_key: "cleanup.rule.flatpak".to_string(),
                description_key: "cleanup.rule.flatpak.desc".to_string(),
                category: CleanupCategory::PackageCache,
                base_path: cache_dir.join("flatpak"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            // 6. Build Caches
            CleanupRule {
                id: "cargo_cache".to_string(),
                name_key: "cleanup.rule.cargo".to_string(),
                description_key: "cleanup.rule.cargo.desc".to_string(),
                category: CleanupCategory::BuildCache,
                base_path: home.join(".cargo/.package-cache"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "npm_cache".to_string(),
                name_key: "cleanup.rule.npm".to_string(),
                description_key: "cleanup.rule.npm.desc".to_string(),
                category: CleanupCategory::BuildCache,
                base_path: home.join(".npm/_cacache"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "pip_cache".to_string(),
                name_key: "cleanup.rule.pip".to_string(),
                description_key: "cleanup.rule.pip.desc".to_string(),
                category: CleanupCategory::BuildCache,
                base_path: cache_dir.join("pip"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "go_build_cache".to_string(),
                name_key: "cleanup.rule.gobuild".to_string(),
                description_key: "cleanup.rule.gobuild.desc".to_string(),
                category: CleanupCategory::BuildCache,
                base_path: cache_dir.join("go-build"),
                is_deep_scan: false,
                safety_level: RiskLevel::Safe,
            },
            // --- FULL SCAN ONLY RULES (Warning / Deep Analysis) ---
            // 7. Dynamic User Downloads Directory (Russian "Загрузки", English "Downloads", etc.)
            CleanupRule {
                id: "downloads_folder".to_string(),
                name_key: "cleanup.rule.downloads".to_string(),
                description_key: "cleanup.rule.downloads.desc".to_string(),
                category: CleanupCategory::Downloads,
                base_path: download_dir,
                is_deep_scan: true,
                safety_level: RiskLevel::Warning,
            },
            // 8. IDE & Editor Caches
            CleanupRule {
                id: "vscode_cache".to_string(),
                name_key: "cleanup.rule.vscode".to_string(),
                description_key: "cleanup.rule.vscode.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: config_dir.join("Code/Cache"),
                is_deep_scan: true,
                safety_level: RiskLevel::Safe,
            },
            CleanupRule {
                id: "jetbrains_cache".to_string(),
                name_key: "cleanup.rule.jetbrains".to_string(),
                description_key: "cleanup.rule.jetbrains.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: cache_dir.join("JetBrains"),
                is_deep_scan: true,
                safety_level: RiskLevel::Safe,
            },
            // 9. GPU Shader Cache
            CleanupRule {
                id: "mesa_shader_cache".to_string(),
                name_key: "cleanup.rule.shaders".to_string(),
                description_key: "cleanup.rule.shaders.desc".to_string(),
                category: CleanupCategory::ApplicationCache,
                base_path: cache_dir.join("mesa_shader_cache"),
                is_deep_scan: true,
                safety_level: RiskLevel::Safe,
            },
            // 10. Crash Reports / Coredumps
            CleanupRule {
                id: "systemd_coredump".to_string(),
                name_key: "cleanup.rule.coredumps".to_string(),
                description_key: "cleanup.rule.coredumps.desc".to_string(),
                category: CleanupCategory::CrashReports,
                base_path: data_dir.join("systemd/coredump"),
                is_deep_scan: true,
                safety_level: RiskLevel::Safe,
            },
        ]
    }
}

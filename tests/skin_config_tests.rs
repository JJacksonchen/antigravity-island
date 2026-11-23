use antigravity_island::config::{AppConfig, DisplayMode, Language};
use antigravity_island::skin::{GaugeStyle, SkinRegistry};

#[test]
fn builtins_include_six_curated_skins_and_resolve_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let reg = SkinRegistry::load_from_dir(dir.path().to_path_buf());
    assert_eq!(reg.list().len(), 6);

    let ids: Vec<&str> = reg.list().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "obsidian",
            "frosted-glass",
            "cyber-neon",
            "matcha-paper",
            "solar-aurora",
            "retro-pixel"
        ]
    );

    assert_eq!(reg.resolve("nonexistent").id, "obsidian");
}

#[test]
fn custom_skin_save_reload_and_builtin_override() {
    let dir = tempfile::tempdir().unwrap();
    let mut reg = SkinRegistry::load_from_dir(dir.path().to_path_buf());
    reg.ensure_initialized().unwrap();
    assert!(dir.path().join("README-SKINS.txt").exists());
    assert!(dir
        .path()
        .join("cyberpunk-neon-sample.toml.example")
        .exists());

    let mut custom = reg.resolve("obsidian");
    custom.id = "My Mint Studio!".to_string();
    custom.name = "My Mint Studio".to_string();
    custom.style.gauge = GaugeStyle::Wave;
    custom.geometry.card_radius_px = 99; // clamped to 40
    custom.palette.accent_gemini = "#10b981".to_string();

    let saved = reg.save_custom_skin(custom).unwrap();
    assert_eq!(saved.id, "my-mint-studio");
    assert_eq!(saved.geometry.card_radius_px, 40);
    assert!(!saved.builtin);
    assert!(dir.path().join("my-mint-studio.toml").exists());

    let reloaded = SkinRegistry::load_from_dir(dir.path().to_path_buf());
    assert_eq!(reloaded.list().len(), 7);
    assert_eq!(
        reloaded.resolve("my-mint-studio").style.gauge,
        GaugeStyle::Wave
    );

    // Override a built-in skin ID via custom TOML
    let mut override_obs = reloaded.resolve("obsidian");
    override_obs.name = "Obsidian Pro Override".to_string();
    override_obs.style.gauge = GaugeStyle::Segments;
    let mut reg2 = reloaded;
    reg2.save_custom_skin(override_obs).unwrap();
    assert_eq!(reg2.resolve("obsidian").name, "Obsidian Pro Override");
    assert_eq!(reg2.resolve("obsidian").style.gauge, GaugeStyle::Segments);
}

#[test]
fn app_config_roundtrip_and_sanitization() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("config.toml");

    let cfg = AppConfig {
        skin_id: "cyber-neon".to_string(),
        mode: DisplayMode::NotchHug,
        poll_interval_secs: 1, // clamped to 5
        language: Language::Zh,
        warn_threshold: 25,
        danger_threshold: 40, // clamped <= warn_threshold (25)
        notifications: false,
        pinned_expanded: true,
        visible: true,
        widget_x: Some(120.5),
        widget_y: Some(88.0),
    };
    cfg.save_to_path(&cfg_path).unwrap();

    let loaded = AppConfig::load_from_path(&cfg_path);
    assert_eq!(loaded.skin_id, "cyber-neon");
    assert_eq!(loaded.mode, DisplayMode::NotchHug);
    assert_eq!(loaded.poll_interval_secs, 5);
    assert_eq!(loaded.language, Language::Zh);
    assert_eq!(loaded.warn_threshold, 25);
    assert_eq!(loaded.danger_threshold, 25);
    assert!(!loaded.notifications);
    assert!(loaded.pinned_expanded);
    assert_eq!(loaded.widget_x, Some(120.5));
}

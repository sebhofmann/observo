use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;

use crate::config::Configuration;
use crate::i18n::Language;
use crate::monitor;
use crate::properties;

pub const FILE_NAME: &str = "observo.properties";
pub const DEFAULT_WIDTH: f64 = 1280.0;
pub const DEFAULT_HEIGHT: f64 = 800.0;
pub const MIN_WIDTH: f64 = 1000.0;
pub const MIN_HEIGHT: f64 = 600.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "system" | "auto" => Some(ThemePreference::System),
            "light" => Some(ThemePreference::Light),
            "dark" => Some(ThemePreference::Dark),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ThemePreference::System => "system",
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowGeometry {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
    pub maximized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            maximized: false,
        }
    }
}

impl WindowGeometry {
    pub fn normalised(mut self) -> Self {
        self.width = clamp_finite(self.width, MIN_WIDTH, 10_000.0, DEFAULT_WIDTH);
        self.height = clamp_finite(self.height, MIN_HEIGHT, 10_000.0, DEFAULT_HEIGHT);
        // The old Java app stored -1 for "not positioned yet". Other negative
        // positions are valid on multi-monitor setups (a screen left of the
        // primary one), so only the sentinel is discarded.
        let valid_position =
            |value: &f64| value.is_finite() && *value != -1.0 && value.abs() < 100_000.0;
        self.x = self.x.filter(valid_position);
        self.y = self.y.filter(valid_position);
        self
    }
}

fn clamp_finite(value: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub language: Language,
    pub theme: ThemePreference,
    pub notifications: bool,
    pub sound: bool,
    pub minimize_on_close: bool,
    pub window: WindowGeometry,
    pub provider_id: String,
    pub server: Configuration,
    /// Where this settings object was loaded from and where it is saved.
    pub config_path: PathBuf,
    /// The file exists but could not be read, so saving would destroy it.
    pub load_failed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let provider = monitor::default_provider();
        Self {
            language: Language::detect(),
            theme: ThemePreference::System,
            notifications: true,
            sound: true,
            minimize_on_close: false,
            window: WindowGeometry::default(),
            provider_id: provider.id().to_string(),
            server: provider.default_configuration(),
            config_path: config_path(),
            load_failed: false,
        }
    }
}

impl Settings {
    /// Loads settings from the default location, falling back to defaults.
    pub fn load() -> (Self, Vec<String>) {
        Self::load_from(&config_path())
    }

    pub fn load_from(path: &Path) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        if !path.exists() {
            return (
                Settings {
                    config_path: path.to_path_buf(),
                    ..Settings::default()
                },
                warnings,
            );
        }
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                warnings.push(format!("could not read {}: {error}", path.display()));
                return (
                    Settings {
                        config_path: path.to_path_buf(),
                        load_failed: true,
                        ..Settings::default()
                    },
                    warnings,
                );
            }
        };
        let text = match String::from_utf8(bytes.clone()) {
            Ok(text) => text,
            Err(_) => bytes.iter().map(|byte| *byte as char).collect(),
        };
        let values = properties::parse(&text);
        let mut settings = Self::from_properties(&values, &mut warnings);
        settings.config_path = path.to_path_buf();
        (settings, warnings)
    }

    fn from_properties(values: &BTreeMap<String, String>, warnings: &mut Vec<String>) -> Self {
        let mut settings = Settings::default();

        if let Some(language) = values.get("language") {
            match Language::from_code(language) {
                Some(language) => settings.language = language,
                None => warnings.push(format!("unknown language '{language}', using English")),
            }
        }

        if let Some(theme) = values.get("ui.theme") {
            match ThemePreference::parse(theme) {
                Some(theme) => settings.theme = theme,
                None => warnings.push(format!("unknown theme '{theme}', using system theme")),
            }
        }

        settings.notifications =
            parse_bool(values.get("ui.notifications").map(String::as_str)).unwrap_or(true);
        settings.sound = parse_bool(values.get("ui.sound").map(String::as_str)).unwrap_or(true);
        settings.minimize_on_close =
            parse_bool(values.get("ui.minimizeOnClose").map(String::as_str)).unwrap_or(false);

        let width = parse_f64(values.get("window.width")).unwrap_or(DEFAULT_WIDTH);
        let height = parse_f64(values.get("window.height")).unwrap_or(DEFAULT_HEIGHT);
        settings.window = WindowGeometry {
            x: parse_f64(values.get("window.x")),
            y: parse_f64(values.get("window.y")),
            width,
            height,
            maximized: parse_bool(values.get("window.maximized").map(String::as_str))
                .unwrap_or(false),
        }
        .normalised();

        let provider_id = values
            .get("server.type")
            .map(String::as_str)
            .unwrap_or("zabbix");
        let provider = monitor::provider(provider_id);
        let specs = provider.field_specs();
        let mut configuration = Configuration::new(provider.id(), specs);
        for spec in specs {
            // The Java version stored the field key verbatim, which produced
            // `server.zabbix.zabbix.url`. Read that, the newer
            // `server.zabbix.url`, and the old unprefixed `zabbix.url`.
            let prefixed = format!("server.{}.{}", provider.id(), spec.key);
            let short_key = spec
                .key
                .strip_prefix(&format!("{}.", provider.id()))
                .unwrap_or(spec.key);
            let clean = format!("server.{}.{}", provider.id(), short_key);
            let stored = values
                .get(&prefixed)
                .or_else(|| values.get(&clean))
                .or_else(|| values.get(spec.key))
                .map(String::as_str);
            let Some(stored) = stored else {
                continue;
            };
            match spec.kind {
                crate::config::FieldKind::Text => configuration.set_text(spec.key, stored),
                crate::config::FieldKind::Password => configuration.set_text(spec.key, stored),
                crate::config::FieldKind::Number => match stored.trim().parse::<i64>() {
                    Ok(number) => configuration.set_number(spec.key, number),
                    Err(_) => warnings.push(format!(
                        "invalid number for '{}' ({}), using default {}",
                        spec.key, stored, spec.default_number
                    )),
                },
                crate::config::FieldKind::Boolean => {
                    if let Some(value) = parse_bool(Some(stored)) {
                        configuration.set_boolean(spec.key, value);
                    } else {
                        warnings.push(format!(
                            "invalid boolean for '{}' ({}), using default",
                            spec.key, stored
                        ));
                    }
                }
            }
        }
        configuration = configuration.normalised();

        settings.provider_id = provider.id().to_string();
        settings.server = configuration;
        settings
    }

    /// Persists settings atomically and restricts the file to the owner.
    pub fn save(&self) -> Result<()> {
        if self.load_failed {
            bail!(
                "{} could not be read, not overwriting it",
                self.config_path.display()
            );
        }
        self.save_to(&self.config_path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let mut values = BTreeMap::new();
        values.insert("language".to_string(), self.language.code().to_string());
        values.insert("ui.theme".to_string(), self.theme.as_str().to_string());
        values.insert(
            "ui.notifications".to_string(),
            self.notifications.to_string(),
        );
        values.insert("ui.sound".to_string(), self.sound.to_string());
        values.insert(
            "ui.minimizeOnClose".to_string(),
            self.minimize_on_close.to_string(),
        );
        values.insert(
            "window.x".to_string(),
            self.window
                .x
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-1".to_string()),
        );
        values.insert(
            "window.y".to_string(),
            self.window
                .y
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-1".to_string()),
        );
        values.insert("window.width".to_string(), self.window.width.to_string());
        values.insert("window.height".to_string(), self.window.height.to_string());
        values.insert(
            "window.maximized".to_string(),
            self.window.maximized.to_string(),
        );
        values.insert("server.type".to_string(), self.provider_id.clone());
        for (key, value) in self.server.values() {
            let short_key = key
                .strip_prefix(&format!("{}.", self.provider_id))
                .unwrap_or(key);
            values.insert(
                format!("server.{}.{}", self.provider_id, short_key),
                value.to_property_string(),
            );
        }

        let text = properties::write(&values, "observo Einstellungen / settings");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let temporary = path.with_extension(format!("properties.tmp-{}", std::process::id()));
        fs::write(&temporary, text.as_bytes())
            .with_context(|| format!("could not write {}", temporary.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600));
        }
        match fs::rename(&temporary, path) {
            Ok(()) => Ok(()),
            Err(_) => {
                // Windows cannot replace an existing file with rename().
                let _ = fs::remove_file(path);
                fs::rename(&temporary, path)
                    .with_context(|| format!("could not replace {}", path.display()))
            }
        }
    }

    pub fn poll_interval(&self) -> std::time::Duration {
        self.server.poll_interval()
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("OBSERVO_CONFIG") {
        return PathBuf::from(path);
    }
    if let Some(dirs) = ProjectDirs::from("de", "paschty", "observo") {
        let current = dirs.config_dir().join(FILE_NAME);
        if current.exists() {
            return current;
        }
        // Migrate seamlessly from the Java version, which always used
        // ~/.config/observo on every platform.
        if let Some(home) = directories::BaseDirs::new() {
            let legacy = home
                .home_dir()
                .join(".config")
                .join("observo")
                .join(FILE_NAME);
            if legacy.exists() {
                return legacy;
            }
        }
        current
    } else {
        PathBuf::from(FILE_NAME)
    }
}

fn parse_bool(value: Option<&str>) -> Option<bool> {
    let value = value?.trim().to_ascii_lowercase();
    match value.as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_f64(value: Option<&String>) -> Option<f64> {
    value?.trim().parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "observo-test-{}-{}-{}.properties",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn roundtrip_save_load() {
        let path = temp_path("roundtrip");
        let mut settings = Settings {
            language: Language::German,
            theme: ThemePreference::Dark,
            notifications: false,
            sound: false,
            minimize_on_close: true,
            window: WindowGeometry {
                x: Some(120.0),
                y: Some(80.0),
                width: 1400.0,
                height: 900.0,
                maximized: true,
            },
            ..Settings::default()
        };
        settings
            .server
            .set_text("zabbix.url", "http://localhost:8080");
        settings.server.set_text("zabbix.username", "Admin");
        settings
            .server
            .set_text("zabbix.password", "secret=with:chars");
        settings
            .server
            .set_number("zabbix.pollIntervalMillis", 2_500);
        settings.save_to(&path).unwrap();

        let (loaded, warnings) = Settings::load_from(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.language, Language::German);
        assert_eq!(loaded.theme, ThemePreference::Dark);
        assert!(!loaded.notifications);
        assert!(!loaded.sound);
        assert!(loaded.minimize_on_close);
        assert_eq!(loaded.window, settings.window.normalised());
        assert_eq!(loaded.server.text("zabbix.url"), "http://localhost:8080");
        assert_eq!(loaded.server.text("zabbix.password"), "secret=with:chars");
        assert_eq!(loaded.server.number("zabbix.pollIntervalMillis"), 2_500);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_uses_the_path_it_was_loaded_from() {
        let path = temp_path("custom-path");
        let (settings, warnings) = Settings::load_from(&path);
        assert!(warnings.is_empty());
        assert_eq!(settings.config_path, path);
        settings.save().unwrap();
        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_negative_monitor_positions_but_drops_the_legacy_sentinel() {
        let geometry = WindowGeometry {
            x: Some(-1920.0),
            y: Some(-1.0),
            width: 1400.0,
            height: 900.0,
            maximized: false,
        }
        .normalised();
        assert_eq!(geometry.x, Some(-1920.0));
        assert_eq!(geometry.y, None);
    }

    #[test]
    fn reads_legacy_keys_and_clamps_values() {
        let path = temp_path("legacy");
        fs::write(
            &path,
            "\
language=de
window.x=-1
window.y=-1
window.width=10
window.height=999999
zabbix.url=http://legacy.example.com/zabbix/
zabbix.username=legacy
zabbix.password=pw
zabbix.pollIntervalMillis=1
",
        )
        .unwrap();
        let (loaded, warnings) = Settings::load_from(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.language, Language::German);
        assert_eq!(loaded.window.x, None);
        assert_eq!(loaded.window.width, MIN_WIDTH);
        assert_eq!(loaded.window.height, 10_000.0);
        assert_eq!(
            loaded.server.text("zabbix.url"),
            "http://legacy.example.com/zabbix"
        );
        assert_eq!(loaded.server.number("zabbix.pollIntervalMillis"), 1_000);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn unreadable_file_keeps_its_path_and_is_never_overwritten() {
        let path = temp_path("unreadable");
        // Reading a directory fails on every platform, even when running as root.
        fs::create_dir(&path).unwrap();
        let (loaded, warnings) = Settings::load_from(&path);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(loaded.config_path, path);

        fs::remove_dir(&path).unwrap();
        assert!(loaded.save().is_err());
        assert!(!path.exists());
    }

    #[test]
    fn loading_keeps_negative_window_position() {
        let path = temp_path("negative-window");
        fs::write(&path, "window.x=-1920\nwindow.y=-40\n").unwrap();
        let (loaded, _) = Settings::load_from(&path);
        assert_eq!(loaded.window.x, Some(-1920.0));
        assert_eq!(loaded.window.y, Some(-40.0));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reads_java_double_prefix_format() {
        let path = temp_path("java-format");
        fs::write(
            &path,
            "\
language=en
server.type=zabbix
server.zabbix.zabbix.url=http://java.example.com/zabbix
server.zabbix.zabbix.username=java-user
server.zabbix.zabbix.password=java-pass
server.zabbix.zabbix.pollIntervalMillis=7000
",
        )
        .unwrap();
        let (loaded, warnings) = Settings::load_from(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            loaded.server.text("zabbix.url"),
            "http://java.example.com/zabbix"
        );
        assert_eq!(loaded.server.text("zabbix.username"), "java-user");
        assert_eq!(loaded.server.number("zabbix.pollIntervalMillis"), 7_000);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn changed_interval_survives_java_import() {
        let path = temp_path("java-roundtrip");
        fs::write(
            &path,
            "\
server.type=zabbix
server.zabbix.zabbix.url=https\\://zabbix.example.com/
server.zabbix.zabbix.pollIntervalMillis=60000
",
        )
        .unwrap();
        let (mut loaded, warnings) = Settings::load_from(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.server.number("zabbix.pollIntervalMillis"), 60_000);

        loaded
            .server
            .set_number("zabbix.pollIntervalMillis", 30_000);
        loaded.save_to(&path).unwrap();
        let (reloaded, _) = Settings::load_from(&path);
        assert_eq!(reloaded.server.number("zabbix.pollIntervalMillis"), 30_000);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn invalid_values_do_not_crash() {
        let path = temp_path("invalid");
        fs::write(
            &path,
            "\
language=klingon
ui.theme=neon
window.width=not-a-number
ui.notifications=maybe
zabbix.pollIntervalMillis=abc
",
        )
        .unwrap();
        let (loaded, warnings) = Settings::load_from(&path);
        assert_eq!(loaded.language, Language::detect());
        assert_eq!(loaded.theme, ThemePreference::System);
        assert_eq!(loaded.window.width, DEFAULT_WIDTH);
        assert!(loaded.notifications);
        assert!(!warnings.is_empty());
        let _ = fs::remove_file(path);
    }
}

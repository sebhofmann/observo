use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::properties;

const DE: &str = include_str!("../resources/messages_de.properties");
const EN: &str = include_str!("../resources/messages_en.properties");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    German,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::German, Language::English];

    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::German => "de",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::German => "Deutsch",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let language = code
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or(code)
            .trim()
            .to_ascii_lowercase();
        match language.as_str() {
            "de" | "deu" | "ger" => Some(Language::German),
            "en" | "eng" => Some(Language::English),
            _ => None,
        }
    }

    /// Detects the user's preferred language from the environment.
    ///
    /// Unlike the old Java implementation this falls back to English instead
    /// of an arbitrary JVM default that may not have a translation at all.
    pub fn detect() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(value) = std::env::var(key)
                && let Some(language) = Language::from_code(&value)
            {
                return language;
            }
        }
        Language::English
    }
}

struct Bundles {
    de: BTreeMap<String, String>,
    en: BTreeMap<String, String>,
}

fn bundles() -> &'static Bundles {
    static BUNDLES: OnceLock<Bundles> = OnceLock::new();
    BUNDLES.get_or_init(|| Bundles {
        de: properties::parse(DE),
        en: properties::parse(EN),
    })
}

/// Small translation facade with a deterministic fallback chain:
/// selected language → English → key.
#[derive(Debug, Clone, Copy)]
pub struct I18n {
    language: Language,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Language::detect())
    }
}

impl I18n {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    pub fn language(self) -> Language {
        self.language
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    pub fn has(self, key: &'static str) -> bool {
        let bundles = bundles();
        bundles.de.contains_key(key) || bundles.en.contains_key(key)
    }

    pub fn t(self, key: &'static str) -> &'static str {
        let bundles = bundles();
        let selected = match self.language {
            Language::German => &bundles.de,
            Language::English => &bundles.en,
        };
        if let Some(value) = selected.get(key) {
            return value.as_str();
        }
        if let Some(value) = bundles.en.get(key) {
            return value.as_str();
        }
        log::debug!("missing translation key: {key}");
        key
    }

    /// Translates `key` and replaces `{name}` placeholders.
    pub fn tf(self, key: &'static str, args: &[(&str, &str)]) -> String {
        let mut text = self.t(key).to_string();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    pub fn classification(self, classification: crate::model::Classification) -> &'static str {
        self.t(classification.i18n_key())
    }

    pub fn relative_time(self, timestamp: chrono::DateTime<chrono::Utc>) -> String {
        let seconds = (chrono::Utc::now() - timestamp).num_seconds().max(0);
        if seconds < 5 {
            self.t("time.justNow").to_string()
        } else if seconds < 60 {
            self.tf("time.secondsAgo", &[("count", &seconds.to_string())])
        } else if seconds < 3600 {
            self.tf("time.minutesAgo", &[("count", &(seconds / 60).to_string())])
        } else if seconds < 86_400 {
            self.tf("time.hoursAgo", &[("count", &(seconds / 3600).to_string())])
        } else {
            self.tf(
                "time.daysAgo",
                &[("count", &(seconds / 86_400).to_string())],
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_detection_handles_locale_variants() {
        assert_eq!(Language::from_code("de_DE.UTF-8"), Some(Language::German));
        assert_eq!(Language::from_code("de-AT"), Some(Language::German));
        assert_eq!(Language::from_code("en_US"), Some(Language::English));
        assert_eq!(Language::from_code("fr_FR"), None);
    }

    #[test]
    fn translations_exist_in_both_languages() {
        let de = properties::parse(DE);
        let en = properties::parse(EN);
        assert!(de.len() > 100);
        assert_eq!(de.len(), en.len(), "both bundles should have the same keys");
        for key in en.keys() {
            assert!(de.contains_key(key), "missing German translation: {key}");
        }
    }

    #[test]
    fn fallback_returns_english_then_key() {
        let i18n = I18n::new(Language::German);
        assert_eq!(i18n.t("app.title"), "Observo");
        assert_eq!(i18n.t("does.not.exist"), "does.not.exist");
    }

    #[test]
    fn formats_placeholders() {
        let i18n = I18n::new(Language::English);
        assert_eq!(
            i18n.tf("main.nextPoll", &[("seconds", "5")]),
            "Next poll in 5s"
        );
    }
}

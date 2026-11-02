use std::collections::BTreeMap;
use std::time::Duration;

/// Kind of control that should be rendered for a configuration field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Password,
    Number,
    Boolean,
}

/// Description of one provider configuration field.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    /// Technical key, also used in the settings file (e.g. `zabbix.url`).
    pub key: &'static str,
    pub label_key: &'static str,
    pub help_key: Option<&'static str>,
    pub placeholder_key: Option<&'static str>,
    pub kind: FieldKind,
    pub default_text: &'static str,
    pub default_number: i64,
    pub default_bool: bool,
    pub min: i64,
    pub max: i64,
    pub required: bool,
}

impl FieldSpec {
    pub const fn text(
        key: &'static str,
        label_key: &'static str,
        default_text: &'static str,
        placeholder_key: Option<&'static str>,
        help_key: Option<&'static str>,
        required: bool,
    ) -> Self {
        Self {
            key,
            label_key,
            help_key,
            placeholder_key,
            kind: FieldKind::Text,
            default_text,
            default_number: 0,
            default_bool: false,
            min: 0,
            max: 0,
            required,
        }
    }

    pub const fn password(
        key: &'static str,
        label_key: &'static str,
        default_text: &'static str,
        placeholder_key: Option<&'static str>,
        help_key: Option<&'static str>,
    ) -> Self {
        Self {
            key,
            label_key,
            help_key,
            placeholder_key,
            kind: FieldKind::Password,
            default_text,
            default_number: 0,
            default_bool: false,
            min: 0,
            max: 0,
            required: false,
        }
    }

    pub const fn number(
        key: &'static str,
        label_key: &'static str,
        default_number: i64,
        min: i64,
        max: i64,
        help_key: Option<&'static str>,
    ) -> Self {
        Self {
            key,
            label_key,
            help_key,
            placeholder_key: None,
            kind: FieldKind::Number,
            default_text: "",
            default_number,
            default_bool: false,
            min,
            max,
            required: true,
        }
    }

    pub fn default_value(self) -> ConfigValue {
        match self.kind {
            FieldKind::Text => ConfigValue::Text(self.default_text.to_string()),
            FieldKind::Password => ConfigValue::Password(self.default_text.to_string()),
            FieldKind::Number => ConfigValue::Number(self.default_number),
            FieldKind::Boolean => ConfigValue::Boolean(self.default_bool),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    Text(String),
    Password(String),
    Number(i64),
    Boolean(bool),
}

impl ConfigValue {
    pub fn as_text(&self) -> &str {
        match self {
            ConfigValue::Text(value) | ConfigValue::Password(value) => value,
            ConfigValue::Number(_) | ConfigValue::Boolean(_) => "",
        }
    }

    pub fn as_number(&self) -> i64 {
        match self {
            ConfigValue::Number(value) => *value,
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        matches!(self, ConfigValue::Boolean(true))
    }

    pub fn to_property_string(&self) -> String {
        match self {
            ConfigValue::Text(value) | ConfigValue::Password(value) => value.clone(),
            ConfigValue::Number(value) => value.to_string(),
            ConfigValue::Boolean(value) => value.to_string(),
        }
    }
}

/// Validated provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    provider_id: String,
    values: BTreeMap<&'static str, ConfigValue>,
}

impl Configuration {
    pub fn new(provider_id: impl Into<String>, specs: &'static [FieldSpec]) -> Self {
        let values = specs
            .iter()
            .map(|spec| (spec.key, spec.default_value()))
            .collect();
        Self {
            provider_id: provider_id.into(),
            values,
        }
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn values(&self) -> &BTreeMap<&'static str, ConfigValue> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    pub fn text(&self, key: &str) -> &str {
        self.values.get(key).map(ConfigValue::as_text).unwrap_or("")
    }

    pub fn number(&self, key: &str) -> i64 {
        self.values
            .get(key)
            .map(ConfigValue::as_number)
            .unwrap_or(0)
    }

    pub fn boolean(&self, key: &str) -> bool {
        self.values
            .get(key)
            .map(ConfigValue::as_bool)
            .unwrap_or(false)
    }

    pub fn set(&mut self, key: &str, value: ConfigValue) {
        if let Some(stored) = self.values.get_mut(key) {
            *stored = value;
        }
    }

    pub fn set_text(&mut self, key: &str, value: impl Into<String>) {
        let is_password = matches!(self.values.get(key), Some(ConfigValue::Password(_)));
        let value = value.into();
        if is_password {
            self.set(key, ConfigValue::Password(value));
        } else {
            self.set(key, ConfigValue::Text(value));
        }
    }

    pub fn set_number(&mut self, key: &str, value: i64) {
        self.set(key, ConfigValue::Number(value));
    }

    pub fn set_boolean(&mut self, key: &str, value: bool) {
        self.set(key, ConfigValue::Boolean(value));
    }

    pub fn poll_interval(&self) -> Duration {
        let millis = self
            .number("zabbix.pollIntervalMillis")
            .clamp(1_000, 3_600_000);
        Duration::from_millis(millis as u64)
    }

    /// Validates the configuration, returning field key → translation key.
    pub fn validate(&self, specs: &[FieldSpec]) -> BTreeMap<&'static str, &'static str> {
        let mut errors = BTreeMap::new();
        for spec in specs {
            let value = self
                .values
                .get(spec.key)
                .cloned()
                .unwrap_or_else(|| spec.default_value());
            match spec.kind {
                FieldKind::Text | FieldKind::Password => {
                    let text = value.as_text();
                    if spec.required && text.trim().is_empty() {
                        errors.insert(spec.key, "serverConfig.required");
                    }
                    if spec.key == "zabbix.url"
                        && let Err(reason) = validate_http_url(text)
                    {
                        errors.insert(spec.key, reason);
                    }
                }
                FieldKind::Number => {
                    let number = value.as_number();
                    if number < spec.min || number > spec.max {
                        errors.insert(spec.key, "serverConfig.invalidInterval");
                    }
                }
                FieldKind::Boolean => {}
            }
        }
        if self.text("zabbix.apiToken").trim().is_empty()
            && self.text("zabbix.username").trim().is_empty()
        {
            errors.insert("zabbix.username", "serverConfig.required");
        }
        errors
    }

    /// Returns the configuration with normalised values (trimmed URL, no trailing slash).
    pub fn normalised(&self) -> Self {
        let mut clone = self.clone();
        let url = clone
            .text("zabbix.url")
            .trim()
            .trim_end_matches('/')
            .to_string();
        let username = clone.text("zabbix.username").trim().to_string();
        let api_token = clone.text("zabbix.apiToken").trim().to_string();
        let filter = clone.text("zabbix.filter").trim().to_string();
        clone.set_text("zabbix.url", url);
        clone.set_text("zabbix.username", username);
        clone.set_text("zabbix.apiToken", api_token);
        clone.set_text("zabbix.filter", filter);
        let interval = clone
            .number("zabbix.pollIntervalMillis")
            .clamp(1_000, 3_600_000);
        clone.set_number("zabbix.pollIntervalMillis", interval);
        clone
    }
}

pub fn validate_http_url(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Err("serverConfig.required");
    }
    match url::Url::parse(value) {
        Ok(parsed) => match parsed.scheme() {
            "http" | "https" if parsed.host_str().is_some() => Ok(()),
            _ => Err("serverConfig.invalidUrl"),
        },
        Err(_) => Err("serverConfig.invalidUrl"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELDS: &[FieldSpec] = &[
        FieldSpec::text(
            "zabbix.url",
            "serverConfig.label.zabbix.url",
            "http://localhost/zabbix",
            None,
            None,
            true,
        ),
        FieldSpec::text(
            "zabbix.username",
            "serverConfig.label.zabbix.username",
            "Admin",
            None,
            None,
            true,
        ),
        FieldSpec::password(
            "zabbix.password",
            "serverConfig.label.zabbix.password",
            "zabbix",
            None,
            None,
        ),
        FieldSpec::number(
            "zabbix.pollIntervalMillis",
            "serverConfig.label.zabbix.pollIntervalMillis",
            10_000,
            1_000,
            3_600_000,
            None,
        ),
    ];

    #[test]
    fn defaults_and_normalisation() {
        let mut config = Configuration::new("zabbix", FIELDS);
        config.set_text("zabbix.url", "  http://example.com/zabbix///  ");
        config.set_number("zabbix.pollIntervalMillis", 500);
        let config = config.normalised();
        assert_eq!(config.text("zabbix.url"), "http://example.com/zabbix");
        assert_eq!(config.number("zabbix.pollIntervalMillis"), 1_000);
        assert_eq!(config.poll_interval(), Duration::from_secs(1));
    }

    #[test]
    fn url_validation() {
        assert!(validate_http_url("http://localhost:8080").is_ok());
        assert!(validate_http_url("https://zabbix.example.com/zabbix").is_ok());
        assert!(validate_http_url("ftp://example.com").is_err());
        assert!(validate_http_url("not a url").is_err());
        assert!(validate_http_url("").is_err());
    }

    #[test]
    fn validation_reports_field_errors() {
        let mut config = Configuration::new("zabbix", FIELDS);
        config.set_text("zabbix.url", "nope");
        config.set_number("zabbix.pollIntervalMillis", 10);
        let errors = config.validate(FIELDS);
        assert_eq!(errors.get("zabbix.url"), Some(&"serverConfig.invalidUrl"));
        assert_eq!(
            errors.get("zabbix.pollIntervalMillis"),
            Some(&"serverConfig.invalidInterval")
        );
    }
}

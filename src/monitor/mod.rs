use std::collections::BTreeMap;
use std::fmt;

use crate::config::{Configuration, FieldSpec};
use crate::model::Message;

pub mod zabbix;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Network,
    Timeout,
    Auth,
    Http(u16),
    Protocol,
    Config,
    Tls,
    Other,
}

/// Error returned by a monitor implementation. Messages are safe to show in the
/// UI and never contain credentials.
#[derive(Debug, Clone)]
pub struct MonitorError {
    pub kind: ErrorKind,
    pub message: String,
    pub details: Option<String>,
}

impl MonitorError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        let mut details = details.into();
        // Keep a stack trace or a huge HTML error page out of the status bar.
        if details.len() > 800 {
            details.truncate(800);
            details.push('…');
        }
        self.details = Some(details);
        self
    }

    pub fn i18n_key(&self) -> &'static str {
        match self.kind {
            ErrorKind::Network => "error.network",
            ErrorKind::Timeout => "error.timeout",
            ErrorKind::Auth => "error.auth",
            ErrorKind::Http(_) => "error.http",
            ErrorKind::Protocol => "error.protocol",
            ErrorKind::Config => "error.config",
            ErrorKind::Tls => "error.tls",
            ErrorKind::Other => "error.title",
        }
    }

    pub fn is_auth(&self) -> bool {
        self.kind == ErrorKind::Auth
    }
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MonitorError {}

/// A monitor backend. Implementations are created on the polling thread and are
/// allowed to keep connection/session state between polls.
pub trait Server: Send {
    fn poll(&mut self) -> Result<Vec<Message>, MonitorError>;
    /// Verifies authentication and returns a human readable server version.
    fn test(&mut self) -> Result<String, MonitorError>;
    fn acknowledge(&mut self, event_id: &str, comment: &str) -> Result<(), MonitorError>;
    fn set_configuration(&mut self, configuration: Configuration);
}

/// Describes a pluggable monitor backend.
pub trait ServerProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_key(&self) -> &'static str;
    fn field_specs(&self) -> &'static [FieldSpec];

    fn default_configuration(&self) -> Configuration {
        Configuration::new(self.id(), self.field_specs())
    }

    fn create_server(&self, configuration: Configuration) -> Box<dyn Server>;

    fn validate(&self, configuration: &Configuration) -> BTreeMap<&'static str, &'static str> {
        configuration.validate(self.field_specs())
    }
}

struct Providers {
    list: Vec<Box<dyn ServerProvider>>,
}

fn providers_inner() -> &'static Providers {
    static PROVIDERS: std::sync::OnceLock<Providers> = std::sync::OnceLock::new();
    PROVIDERS.get_or_init(|| Providers {
        list: vec![Box::new(zabbix::ZabbixProvider)],
    })
}

pub fn providers() -> &'static [Box<dyn ServerProvider>] {
    &providers_inner().list
}

pub fn provider(id: &str) -> &'static dyn ServerProvider {
    providers_inner()
        .list
        .iter()
        .map(|provider| provider.as_ref())
        .find(|provider| provider.id() == id)
        .unwrap_or_else(|| default_provider())
}

pub fn default_provider() -> &'static dyn ServerProvider {
    providers_inner().list[0].as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_zabbix_and_falls_back() {
        assert_eq!(providers().len(), 1);
        assert_eq!(default_provider().id(), "zabbix");
        assert_eq!(provider("does-not-exist").id(), "zabbix");
    }
}

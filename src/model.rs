use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

/// Maximum length of an `event.acknowledge` message accepted by Zabbix.
pub const MAX_ACK_COMMENT_CHARS: usize = 2048;

/// Severity bucket used by the UI.
///
/// Zabbix has six severity levels (0..=5). Observo groups them into these
/// buckets so that colours, filters and notifications stay consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Classification {
    Critical,
    Warning,
    Info,
    Recovery,
    Acknowledged,
    Unknown,
}

impl Classification {
    /// Stable order used by the UI when grouping/filtering.
    pub const ALL: [Classification; 6] = [
        Classification::Critical,
        Classification::Warning,
        Classification::Info,
        Classification::Recovery,
        Classification::Acknowledged,
        Classification::Unknown,
    ];

    pub fn i18n_key(self) -> &'static str {
        match self {
            Classification::Critical => "classification.critical",
            Classification::Warning => "classification.warning",
            Classification::Info => "classification.info",
            Classification::Recovery => "classification.recovery",
            Classification::Acknowledged => "classification.acknowledged",
            Classification::Unknown => "classification.unknown",
        }
    }

    /// Problems that should raise a notification.
    pub fn is_alert(self) -> bool {
        matches!(self, Classification::Critical | Classification::Warning)
    }

    /// Zabbix severity (0..=5) for sorting; acknowledged/recovered entries keep
    /// the severity that caused them.
    pub fn rank(self) -> u8 {
        match self {
            Classification::Critical => 5,
            Classification::Warning => 3,
            Classification::Info => 1,
            Classification::Recovery => 2,
            Classification::Acknowledged => 2,
            Classification::Unknown => 0,
        }
    }

    pub fn from_severity(
        severity: u8,
        acknowledged: bool,
        suppressed: bool,
        recovered: bool,
    ) -> Self {
        if recovered {
            Classification::Recovery
        } else if acknowledged {
            Classification::Acknowledged
        } else if suppressed {
            Classification::Info
        } else {
            match severity {
                0 => Classification::Unknown,
                1..=3 => Classification::Warning,
                4..=5 => Classification::Critical,
                _ => Classification::Unknown,
            }
        }
    }
}

/// A problem as shown in the main table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Stable identity for diffing between polls.
    pub id: String,
    /// Zabbix event id. `None` for triggers that have no event yet; such a
    /// message cannot be acknowledged.
    pub event_id: Option<String>,
    pub trigger_id: String,
    pub title: String,
    pub message: String,
    pub hostname: String,
    pub classification: Classification,
    pub timestamp: DateTime<Utc>,
    pub severity: u8,
    pub acknowledged: bool,
    pub suppressed: bool,
    pub recovered: bool,
    pub maintenance: bool,
    pub custom_fields: BTreeMap<String, String>,
}

impl Message {
    pub fn can_acknowledge(&self) -> bool {
        self.event_id.is_some() && !self.acknowledged && !self.recovered && !self.id.is_empty()
    }

    pub fn is_alert(&self) -> bool {
        self.classification.is_alert()
    }

    /// Returns a short identifier that can safely be logged.
    pub fn log_id(&self) -> &str {
        self.event_id.as_deref().unwrap_or(&self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_mapping_matches_zabbix_buckets() {
        assert_eq!(
            Classification::from_severity(5, false, false, false),
            Classification::Critical
        );
        assert_eq!(
            Classification::from_severity(4, false, false, false),
            Classification::Critical
        );
        assert_eq!(
            Classification::from_severity(3, false, false, false),
            Classification::Warning
        );
        assert_eq!(
            Classification::from_severity(1, false, false, false),
            Classification::Warning
        );
        assert_eq!(
            Classification::from_severity(0, false, false, false),
            Classification::Unknown
        );
    }

    #[test]
    fn acknowledged_and_recovered_win_over_severity() {
        assert_eq!(
            Classification::from_severity(5, true, false, false),
            Classification::Acknowledged
        );
        assert_eq!(
            Classification::from_severity(5, true, true, true),
            Classification::Recovery
        );
        assert_eq!(
            Classification::from_severity(5, false, true, false),
            Classification::Info
        );
    }
}

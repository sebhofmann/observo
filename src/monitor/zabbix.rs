use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use ureq::Agent;

use crate::config::{Configuration, FieldSpec};
use crate::model::{Classification, Message};

use super::{ErrorKind, MonitorError, Server, ServerProvider};

pub const FIELDS: &[FieldSpec] = &[
    FieldSpec::text(
        "zabbix.url",
        "serverConfig.label.zabbix.url",
        "http://localhost/zabbix",
        Some("serverConfig.placeholder.zabbix.url"),
        None,
        true,
    ),
    FieldSpec::text(
        "zabbix.username",
        "serverConfig.label.zabbix.username",
        "Admin",
        Some("serverConfig.placeholder.zabbix.username"),
        None,
        true,
    ),
    FieldSpec::password(
        "zabbix.password",
        "serverConfig.label.zabbix.password",
        "zabbix",
        Some("serverConfig.placeholder.zabbix.password"),
        None,
    ),
    FieldSpec::password(
        "zabbix.apiToken",
        "serverConfig.label.zabbix.apiToken",
        "",
        Some("serverConfig.placeholder.zabbix.apiToken"),
        Some("serverConfig.help.zabbix.apiToken"),
    ),
    FieldSpec::number(
        "zabbix.pollIntervalMillis",
        "serverConfig.label.zabbix.pollIntervalMillis",
        10_000,
        1_000,
        3_600_000,
        Some("serverConfig.help.zabbix.pollIntervalMillis"),
    ),
    FieldSpec::text(
        "zabbix.filter",
        "serverConfig.label.zabbix.filter",
        "",
        Some("serverConfig.placeholder.zabbix.filter"),
        Some("serverConfig.help.zabbix.filter"),
        false,
    ),
];

pub struct ZabbixProvider;

impl ServerProvider for ZabbixProvider {
    fn id(&self) -> &'static str {
        "zabbix"
    }

    fn display_key(&self) -> &'static str {
        "serverType.zabbix"
    }

    fn field_specs(&self) -> &'static [FieldSpec] {
        FIELDS
    }

    fn create_server(&self, configuration: Configuration) -> Box<dyn Server> {
        Box::new(ZabbixServer::new(configuration))
    }
}

const CHUNK_SIZE: usize = 200;
const MAX_MESSAGES: usize = 5_000;
const MAX_RESPONSE_BYTES: u64 = 20 * 1024 * 1024;
const ACK_ACTION_ACKNOWLEDGE: i64 = 2;
const ACK_ACTION_MESSAGE: i64 = 4;

struct Session {
    version: String,
    token: String,
    bearer: bool,
}

pub struct ZabbixServer {
    configuration: Configuration,
    agent: Agent,
    session: Option<Session>,
    request_id: u64,
}

impl ZabbixServer {
    pub fn new(configuration: Configuration) -> Self {
        let agent_config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .build();
        Self {
            configuration,
            agent: agent_config.into(),
            session: None,
            request_id: 0,
        }
    }

    fn api_url(&self) -> Result<String, MonitorError> {
        let raw = self.configuration.text("zabbix.url").trim();
        let mut parsed = url::Url::parse(raw).map_err(|_| {
            MonitorError::new(
                ErrorKind::Config,
                "invalid Zabbix URL: expected http(s) URL",
            )
        })?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(MonitorError::new(
                ErrorKind::Config,
                "invalid Zabbix URL: only http(s) URLs are supported",
            ));
        }
        if !parsed.path().ends_with("/api_jsonrpc.php") {
            let path = format!("{}/api_jsonrpc.php", parsed.path().trim_end_matches('/'));
            parsed.set_path(&path);
        }
        Ok(parsed.to_string())
    }

    fn next_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }

    fn rpc_once(
        &mut self,
        method: &str,
        params: Value,
        authenticated: bool,
        token: Option<&str>,
        bearer: bool,
    ) -> Result<Value, MonitorError> {
        let url = self.api_url()?;
        let id = self.next_id();
        let mut body = serde_json::Map::new();
        body.insert("jsonrpc".into(), json!("2.0"));
        body.insert("method".into(), json!(method));
        body.insert("params".into(), params);
        body.insert("id".into(), json!(id));
        if authenticated
            && !bearer
            && let Some(token) = token
            && !token.is_empty()
        {
            body.insert("auth".into(), json!(token));
        }

        let mut request = self
            .agent
            .post(&url)
            .header("Content-Type", "application/json-rpc")
            .header("Accept", "application/json");
        if authenticated
            && bearer
            && let Some(token) = token
            && !token.is_empty()
        {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let mut response = request
            .send_json(Value::Object(body))
            .map_err(map_transport_error)?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|error| {
                MonitorError::new(ErrorKind::Network, "could not read Zabbix response")
                    .with_details(error.to_string())
            })?;

        let parsed: Value = serde_json::from_str(&text).map_err(|error| {
            let kind = if status == 401 || status == 403 {
                ErrorKind::Auth
            } else {
                ErrorKind::Protocol
            };
            MonitorError::new(kind, "Zabbix returned an invalid JSON response")
                .with_details(format!("HTTP {status}: {error}"))
        })?;

        if !(200..=299).contains(&status) {
            let message = parsed
                .get("error")
                .and_then(|error| error.get("data"))
                .and_then(Value::as_str)
                .unwrap_or("HTTP request failed");
            return Err(http_error(status, message));
        }

        if let Some(error) = parsed.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = error
                .get("data")
                .and_then(Value::as_str)
                .or_else(|| error.get("message").and_then(Value::as_str))
                .unwrap_or("unknown Zabbix API error");
            let kind = if is_auth_message(message) || code == -32602 && message.contains("login") {
                ErrorKind::Auth
            } else {
                ErrorKind::Protocol
            };
            return Err(
                MonitorError::new(kind, message.to_string()).with_details(format!("code {code}"))
            );
        }

        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| MonitorError::new(ErrorKind::Protocol, "Zabbix response has no result"))
    }

    fn rpc(
        &mut self,
        method: &str,
        params: Value,
        authenticated: bool,
    ) -> Result<Value, MonitorError> {
        if authenticated {
            self.ensure_session()?;
        }
        let token = self
            .session
            .as_ref()
            .map(|session| session.token.clone())
            .unwrap_or_default();
        let bearer = self
            .session
            .as_ref()
            .map(|session| session.bearer)
            .unwrap_or(false);
        let first = self.rpc_once(method, params.clone(), authenticated, Some(&token), bearer);
        match first {
            Ok(result) => Ok(result),
            Err(error) if authenticated && error.is_auth() => {
                log::info!("Zabbix session expired, re-authenticating");
                self.session = None;
                self.ensure_session()?;
                let token = self
                    .session
                    .as_ref()
                    .map(|session| session.token.clone())
                    .unwrap_or_default();
                let bearer = self
                    .session
                    .as_ref()
                    .map(|session| session.bearer)
                    .unwrap_or(false);
                self.rpc_once(method, params, true, Some(&token), bearer)
            }
            Err(error) => Err(error),
        }
    }

    fn ensure_session(&mut self) -> Result<(), MonitorError> {
        if self.session.is_some() {
            return Ok(());
        }

        let version = self
            .rpc_once("apiinfo.version", json!([]), false, None, false)?
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        if version.is_empty() {
            return Err(MonitorError::new(
                ErrorKind::Protocol,
                "Zabbix did not report an API version",
            ));
        }
        if compare_versions(&version, "4.4") == Ordering::Less {
            return Err(MonitorError::new(
                ErrorKind::Config,
                "Zabbix 4.4 or newer is required",
            ));
        }
        log::info!("connected to Zabbix API {version}");

        let api_token = self
            .configuration
            .text("zabbix.apiToken")
            .trim()
            .to_string();
        if !api_token.is_empty() {
            if compare_versions(&version, "5.4") == Ordering::Less {
                return Err(MonitorError::new(
                    ErrorKind::Config,
                    "API tokens require Zabbix 5.4 or newer",
                ));
            }
            let bearer = compare_versions(&version, "6.4") != Ordering::Less;
            self.session = Some(Session {
                version,
                token: api_token,
                bearer,
            });
            return Ok(());
        }

        let username = self.configuration.text("zabbix.username").trim();
        let password = self.configuration.text("zabbix.password");
        if username.is_empty() {
            return Err(MonitorError::new(
                ErrorKind::Config,
                "a username or API token is required",
            ));
        }
        let user_key = if compare_versions(&version, "6.4") != Ordering::Less {
            "username"
        } else {
            "user"
        };
        let mut params = serde_json::Map::new();
        params.insert(user_key.to_string(), json!(username));
        params.insert("password".to_string(), json!(password));
        let params = Value::Object(params);
        let token = self
            .rpc_once("user.login", params, false, None, false)?
            .as_str()
            .ok_or_else(|| {
                MonitorError::new(
                    ErrorKind::Protocol,
                    "Zabbix login returned no session token",
                )
            })?
            .to_string();
        let bearer = compare_versions(&version, "6.4") != Ordering::Less;
        self.session = Some(Session {
            version,
            token,
            bearer,
        });
        Ok(())
    }

    fn fetch_trigger_ids(&mut self) -> Result<Vec<String>, MonitorError> {
        let result = self.rpc(
            "trigger.get",
            json!({
                "only_true": 1,
                "monitored": 1,
                "active": 1,
                "skipDependent": 1,
                "output": ["triggerid"]
            }),
            true,
        )?;
        let array = result.as_array().ok_or_else(|| {
            MonitorError::new(ErrorKind::Protocol, "trigger.get did not return an array")
        })?;
        Ok(array
            .iter()
            .filter_map(|trigger| non_empty_string(trigger.get("triggerid")))
            .collect())
    }

    fn fetch_trigger_details(&mut self, ids: &[String]) -> Result<Vec<Value>, MonitorError> {
        let mut triggers = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(CHUNK_SIZE) {
            let result = self.rpc(
                "trigger.get",
                json!({
                    "triggerids": chunk,
                    "output": [
                        "triggerid", "description", "lastchange", "priority",
                        "manual_close", "value"
                    ],
                    "expandDescription": 1,
                    "selectLastEvent": [
                        "eventid", "name", "opdata", "clock", "acknowledged",
                        "value", "severity"
                    ],
                    "selectHosts": [
                        "hostid", "host", "name", "status", "maintenance_status"
                    ],
                    "selectItems": ["itemid", "name", "lastvalue", "lastclock", "state"]
                }),
                true,
            )?;
            let array = result.as_array().ok_or_else(|| {
                MonitorError::new(ErrorKind::Protocol, "trigger.get did not return an array")
            })?;
            triggers.extend(array.iter().cloned());
        }
        Ok(triggers)
    }

    fn parse_filter(&self) -> Vec<String> {
        self.configuration
            .text("zabbix.filter")
            .split(';')
            .map(str::trim)
            .filter(|term| !term.is_empty())
            .map(|term| term.to_lowercase())
            .collect()
    }
}

impl Server for ZabbixServer {
    fn poll(&mut self) -> Result<Vec<Message>, MonitorError> {
        let ids = self.fetch_trigger_ids()?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let filters = self.parse_filter();
        let triggers = self.fetch_trigger_details(&ids)?;
        let mut messages = Vec::with_capacity(triggers.len());
        let mut seen = HashSet::new();
        for trigger in &triggers {
            let Some(message) = message_from_trigger(trigger) else {
                continue;
            };
            if !seen.insert(message.id.clone()) {
                continue;
            }
            if is_filtered(&message, &filters) {
                log::debug!("filtered out Zabbix message '{}'", message.title);
                continue;
            }
            messages.push(message);
        }
        sort_messages(&mut messages);
        if messages.len() > MAX_MESSAGES {
            log::warn!(
                "Zabbix returned {} messages, truncating to {}",
                messages.len(),
                MAX_MESSAGES
            );
            messages.truncate(MAX_MESSAGES);
        }
        Ok(messages)
    }

    fn test(&mut self) -> Result<String, MonitorError> {
        self.ensure_session()?;
        self.rpc(
            "trigger.get",
            json!({"output": ["triggerid"], "limit": 1}),
            true,
        )?;
        Ok(self
            .session
            .as_ref()
            .map(|session| session.version.clone())
            .unwrap_or_default())
    }

    fn acknowledge(&mut self, event_id: &str, comment: &str) -> Result<(), MonitorError> {
        let event_id = event_id.trim();
        if event_id.is_empty() || event_id == "error" {
            return Err(MonitorError::new(
                ErrorKind::Config,
                "this message has no acknowledgeable event id",
            ));
        }
        let comment = comment.trim();
        let mut action = ACK_ACTION_ACKNOWLEDGE;
        let mut params = json!({
            "eventids": [event_id],
            "action": action
        });
        if !comment.is_empty() {
            action |= ACK_ACTION_MESSAGE;
            params = json!({
                "eventids": [event_id],
                "action": action,
                "message": comment
            });
        }
        let result = self.rpc("event.acknowledge", params, true)?;
        let acknowledged = result
            .get("eventids")
            .and_then(Value::as_array)
            .map(|ids| ids.iter().any(|id| json_to_string(id) == event_id))
            .unwrap_or(false);
        if acknowledged {
            Ok(())
        } else {
            Err(MonitorError::new(
                ErrorKind::Protocol,
                "Zabbix did not acknowledge the event",
            ))
        }
    }

    fn set_configuration(&mut self, configuration: Configuration) {
        if self.configuration != configuration {
            log::debug!("Zabbix configuration changed, resetting session");
            self.configuration = configuration;
            self.session = None;
        }
    }
}

/// Tests a configuration without touching a long-lived server instance.
pub fn test_configuration(configuration: &Configuration) -> Result<String, MonitorError> {
    ZabbixServer::new(configuration.clone()).test()
}

fn map_transport_error(error: ureq::Error) -> MonitorError {
    match error {
        ureq::Error::Timeout(_) => {
            MonitorError::new(ErrorKind::Timeout, "Zabbix request timed out")
        }
        ureq::Error::HostNotFound => {
            MonitorError::new(ErrorKind::Network, "Zabbix host was not found")
        }
        ureq::Error::ConnectionFailed => {
            MonitorError::new(ErrorKind::Network, "could not connect to the Zabbix server")
        }
        ureq::Error::Tls(message) => {
            MonitorError::new(ErrorKind::Tls, "TLS connection failed").with_details(message)
        }
        ureq::Error::Rustls(error) => MonitorError::new(ErrorKind::Tls, "TLS connection failed")
            .with_details(error.to_string()),
        ureq::Error::StatusCode(status) => http_error(status, "HTTP request failed"),
        ureq::Error::Io(error) => {
            let kind = if error.to_string().to_lowercase().contains("certificate") {
                ErrorKind::Tls
            } else {
                ErrorKind::Network
            };
            MonitorError::new(kind, "network error").with_details(error.to_string())
        }
        ureq::Error::BadUri(message) => {
            MonitorError::new(ErrorKind::Config, "invalid Zabbix URL").with_details(message)
        }
        other => MonitorError::new(ErrorKind::Network, "could not talk to Zabbix")
            .with_details(other.to_string()),
    }
}

fn http_error(status: u16, message: &str) -> MonitorError {
    let kind = if status == 401 || status == 403 {
        ErrorKind::Auth
    } else {
        ErrorKind::Http(status)
    };
    MonitorError::new(kind, message.to_string()).with_details(format!("HTTP {status}"))
}

fn is_auth_message(message: &str) -> bool {
    let message = message.to_lowercase();
    [
        "not authorised",
        "not authorized",
        "unauthorized",
        "session terminated",
        "re-login",
        "login name or password",
        "incorrect user name or password",
        "permission denied",
        "api token",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let text = json_to_string(value);
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn json_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => {
            if *boolean {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn empty_object<'a>() -> &'a serde_json::Map<String, Value> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, Value>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(serde_json::Map::new)
}

fn object(value: &Value) -> &serde_json::Map<String, Value> {
    value.as_object().unwrap_or(empty_object())
}

fn last_event(trigger: &Value) -> &serde_json::Map<String, Value> {
    match trigger.get("lastEvent") {
        Some(Value::Array(items)) => items.first().map(object).unwrap_or_else(empty_object),
        Some(value @ Value::Object(_)) => object(value),
        _ => empty_object(),
    }
}

fn build_message(trigger: &Value) -> String {
    let event = last_event(trigger);
    let opdata = event.get("opdata").map(json_to_string).unwrap_or_default();
    if !opdata.trim().is_empty() {
        return opdata;
    }
    let items = trigger
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name").map(json_to_string).unwrap_or_default();
                    if name.trim().is_empty() {
                        return None;
                    }
                    let value = item
                        .get("lastvalue")
                        .map(json_to_string)
                        .unwrap_or_default();
                    Some(if value.trim().is_empty() {
                        name
                    } else {
                        format!("{name}: {value}")
                    })
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if !items.is_empty() {
        return items;
    }
    event
        .get("name")
        .map(json_to_string)
        .filter(|text| !text.trim().is_empty())
        .or_else(|| trigger.get("description").map(json_to_string))
        .unwrap_or_default()
}

/// Converts one `trigger.get` object into the UI message model.
///
/// This function is pure and covered by unit tests with recorded Zabbix
/// payloads.
pub fn message_from_trigger(trigger: &Value) -> Option<Message> {
    let trigger_id = non_empty_string(trigger.get("triggerid"))?;
    let event = last_event(trigger);
    let event_id = non_empty_string(event.get("eventid"));
    let description = trigger
        .get("description")
        .map(json_to_string)
        .unwrap_or_default();
    let event_name = event.get("name").map(json_to_string).unwrap_or_default();
    let title = if event_name.trim().is_empty() {
        description
    } else {
        event_name
    };

    let hosts = trigger
        .get("hosts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let hostname = hosts
        .iter()
        .find_map(|host| {
            let name = host.get("name").map(json_to_string).unwrap_or_default();
            if name.trim().is_empty() {
                host.get("host").map(json_to_string)
            } else {
                Some(name)
            }
        })
        .unwrap_or_default();
    let maintenance = hosts.iter().any(|host| {
        host.get("maintenance_status")
            .map(|value| json_to_string(value) == "1")
            .unwrap_or(false)
    });

    let severity = event
        .get("severity")
        .or_else(|| trigger.get("priority"))
        .map(|value| json_to_string(value).parse::<u8>().unwrap_or(0).min(5))
        .unwrap_or(0);
    let acknowledged = event
        .get("acknowledged")
        .map(|value| json_to_string(value) == "1")
        .unwrap_or(false);
    let event_value = event.get("value").map(json_to_string).unwrap_or_default();
    let trigger_value = trigger.get("value").map(json_to_string).unwrap_or_default();
    let recovered = event_value == "0" || (event_value.is_empty() && trigger_value == "0");
    let classification =
        Classification::from_severity(severity, acknowledged, maintenance, recovered);

    let clock = event
        .get("clock")
        .map(|value| json_to_string(value).parse::<i64>().unwrap_or(0))
        .or_else(|| {
            trigger
                .get("lastchange")
                .map(|value| json_to_string(value).parse::<i64>().unwrap_or(0))
        })
        .unwrap_or(0);
    let timestamp = DateTime::from_timestamp(clock, 0).unwrap_or_else(Utc::now);

    let mut custom = BTreeMap::new();
    custom.insert("triggerid".to_string(), trigger_id.clone());
    if let Some(event_id) = &event_id {
        custom.insert("eventid".to_string(), event_id.clone());
    }
    custom.insert("severity".to_string(), severity.to_string());
    custom.insert(
        "acknowledged".to_string(),
        if acknowledged { "1" } else { "0" }.to_string(),
    );
    custom.insert(
        "manual_close".to_string(),
        trigger
            .get("manual_close")
            .map(json_to_string)
            .unwrap_or_else(|| "0".to_string()),
    );
    if maintenance {
        custom.insert("maintenance".to_string(), "1".to_string());
    }
    if let Some(opdata) = event.get("opdata").map(json_to_string)
        && !opdata.trim().is_empty()
    {
        custom.insert("opdata".to_string(), opdata);
    }
    let hosts_joined = hosts
        .iter()
        .map(|host| {
            let name = host.get("name").map(json_to_string).unwrap_or_default();
            if name.trim().is_empty() {
                host.get("host").map(json_to_string).unwrap_or_default()
            } else {
                name
            }
        })
        .filter(|name| !name.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if !hosts_joined.is_empty() {
        custom.insert("hosts".to_string(), hosts_joined);
    }

    let id = event_id
        .clone()
        .unwrap_or_else(|| format!("trigger:{trigger_id}"));

    Some(Message {
        id,
        event_id,
        trigger_id,
        title,
        message: build_message(trigger),
        hostname,
        classification,
        timestamp,
        severity,
        acknowledged,
        suppressed: maintenance,
        recovered,
        maintenance,
        custom_fields: custom,
    })
}

fn is_filtered(message: &Message, filters: &[String]) -> bool {
    if filters.is_empty() {
        return false;
    }
    let title = message.title.to_lowercase();
    let text = message.message.to_lowercase();
    let host = message.hostname.to_lowercase();
    let all_hosts = message
        .custom_fields
        .get("hosts")
        .map(|hosts| hosts.to_lowercase())
        .unwrap_or_default();
    filters.iter().any(|filter| {
        title.contains(filter)
            || text.contains(filter)
            || host.contains(filter)
            || all_hosts.contains(filter)
    })
}

pub fn sort_messages(messages: &mut [Message]) {
    messages.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| b.classification.rank().cmp(&a.classification.rank()))
            .then_with(|| b.timestamp.cmp(&a.timestamp))
    });
}

/// Numeric version comparison that ignores pre-release suffixes.
pub fn compare_versions(left: &str, right: &str) -> Ordering {
    fn parts(version: &str) -> [u64; 3] {
        let mut result = [0; 3];
        for (index, part) in version.split('.').take(3).enumerate() {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            result[index] = digits.parse().unwrap_or(0);
        }
        result
    }
    parts(left).cmp(&parts(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_trigger() -> Value {
        serde_json::from_str(
            r#"{
                "triggerid": "10472",
                "description": "High CPU load on web-01",
                "priority": "4",
                "lastchange": "1710000000",
                "manual_close": "1",
                "value": "1",
                "lastEvent": {
                    "eventid": "99123",
                    "name": "High CPU load on web-01",
                    "opdata": "CPU load: 93 %",
                    "clock": "1710000100",
                    "acknowledged": "0",
                    "value": "1",
                    "severity": "4"
                },
                "hosts": [{
                    "hostid": "10084",
                    "host": "web-01",
                    "name": "web-01",
                    "status": "0",
                    "maintenance_status": "0"
                }],
                "items": [{
                    "itemid": "1",
                    "name": "CPU load",
                    "lastvalue": "93",
                    "lastclock": "1710000100",
                    "state": "0"
                }]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn maps_a_problem_trigger() {
        let message = message_from_trigger(&sample_trigger()).unwrap();
        assert_eq!(message.id, "99123");
        assert_eq!(message.event_id.as_deref(), Some("99123"));
        assert_eq!(message.trigger_id, "10472");
        assert_eq!(message.title, "High CPU load on web-01");
        assert_eq!(message.message, "CPU load: 93 %");
        assert_eq!(message.hostname, "web-01");
        assert_eq!(message.classification, Classification::Critical);
        assert_eq!(message.severity, 4);
        assert!(!message.acknowledged);
        assert!(message.can_acknowledge());
        assert_eq!(message.timestamp.timestamp(), 1_710_000_100);
    }

    #[test]
    fn maps_acknowledged_and_maintenance() {
        let mut trigger = sample_trigger();
        trigger["lastEvent"]["acknowledged"] = json!("1");
        trigger["hosts"][0]["maintenance_status"] = json!("1");
        let message = message_from_trigger(&trigger).unwrap();
        assert!(message.acknowledged);
        assert!(message.maintenance);
        assert_eq!(message.classification, Classification::Acknowledged);
        assert!(!message.can_acknowledge());
    }

    #[test]
    fn maps_recovered_event() {
        let mut trigger = sample_trigger();
        trigger["lastEvent"]["value"] = json!("0");
        trigger["value"] = json!("0");
        let message = message_from_trigger(&trigger).unwrap();
        assert!(message.recovered);
        assert_eq!(message.classification, Classification::Recovery);
        assert!(!message.can_acknowledge());
    }

    #[test]
    fn falls_back_to_trigger_description_and_items() {
        let mut trigger = sample_trigger();
        trigger["lastEvent"] = json!({});
        let message = message_from_trigger(&trigger).unwrap();
        assert_eq!(message.title, "High CPU load on web-01");
        assert_eq!(message.message, "CPU load: 93");
        assert_eq!(message.id, "trigger:10472");
        assert!(!message.can_acknowledge());
    }

    #[test]
    fn filters_are_case_insensitive_and_cover_host() {
        let message = message_from_trigger(&sample_trigger()).unwrap();
        assert!(is_filtered(&message, &["cpu load".into()]));
        assert!(is_filtered(&message, &["WEB-01".to_lowercase()]));
        assert!(!is_filtered(&message, &["database".into()]));
    }

    #[test]
    fn compares_versions() {
        assert_eq!(compare_versions("7.0.5", "6.4"), Ordering::Greater);
        assert_eq!(compare_versions("6.4.0rc1", "6.4"), Ordering::Equal);
        assert_eq!(compare_versions("5.0", "6.4"), Ordering::Less);
        assert_eq!(compare_versions("7", "7.0.0"), Ordering::Equal);
    }

    #[test]
    fn builds_api_url() {
        let server = ZabbixServer::new(Configuration::new("zabbix", FIELDS));
        assert_eq!(
            server.api_url().unwrap(),
            "http://localhost/zabbix/api_jsonrpc.php"
        );
        let mut config = Configuration::new("zabbix", FIELDS);
        config.set_text("zabbix.url", "https://zabbix.example.com/api_jsonrpc.php");
        let server = ZabbixServer::new(config);
        assert_eq!(
            server.api_url().unwrap(),
            "https://zabbix.example.com/api_jsonrpc.php"
        );

        let mut config = Configuration::new("zabbix", FIELDS);
        config.set_text("zabbix.url", "http://localhost:8080/zabbix/?tenant=one");
        let server = ZabbixServer::new(config);
        assert_eq!(
            server.api_url().unwrap(),
            "http://localhost:8080/zabbix/api_jsonrpc.php?tenant=one"
        );
    }

    #[test]
    fn sorts_by_severity_then_time() {
        let mut low = message_from_trigger(&sample_trigger()).unwrap();
        low.severity = 2;
        low.id = "low".into();
        let mut high = low.clone();
        high.severity = 5;
        high.id = "high".into();
        let mut messages = vec![low, high];
        sort_messages(&mut messages);
        assert_eq!(messages[0].id, "high");
    }
}

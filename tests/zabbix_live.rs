//! Live integration test against the Docker Zabbix instance.
//!
//! Run it with:
//!
//! ```sh
//! docker compose -f docker/zabbix/docker-compose.yml up -d
//! python3 docker/zabbix/seed.py setup
//! python3 docker/zabbix/seed.py trigger
//! OBSERVO_ZABBIX_URL=http://localhost:8080 cargo test --test zabbix_live -- --nocapture
//! ```
//!
//! Without `OBSERVO_ZABBIX_URL` the test is a no-op so `cargo test` works on
//! machines without Docker.

use observo::config::Configuration;
use observo::model::Classification;
use observo::monitor::Server;
use observo::monitor::zabbix::{self, FIELDS, ZabbixServer};
use observo::poll::{PollEvent, PollHandle};

fn test_configuration() -> Option<Configuration> {
    let url = std::env::var("OBSERVO_ZABBIX_URL").ok()?;
    let mut configuration = Configuration::new("zabbix", FIELDS);
    configuration.set_text("zabbix.url", url);
    configuration.set_text(
        "zabbix.username",
        std::env::var("OBSERVO_ZABBIX_USER").unwrap_or_else(|_| "Admin".to_string()),
    );
    configuration.set_text(
        "zabbix.password",
        std::env::var("OBSERVO_ZABBIX_PASSWORD").unwrap_or_else(|_| "zabbix".to_string()),
    );
    configuration.set_text(
        "zabbix.apiToken",
        std::env::var("OBSERVO_ZABBIX_TOKEN").unwrap_or_default(),
    );
    configuration.set_number("zabbix.pollIntervalMillis", 2_000);
    Some(configuration.normalised())
}

#[test]
fn live_zabbix_roundtrip() {
    let Some(configuration) = test_configuration() else {
        eprintln!("OBSERVO_ZABBIX_URL not set, skipping live Zabbix test");
        return;
    };

    let version = zabbix::test_configuration(&configuration).expect("connection test");
    eprintln!("connected to Zabbix {version}");
    assert!(!version.is_empty());

    let mut server = ZabbixServer::new(configuration.clone());
    let messages = server.poll().expect("poll");
    assert!(!messages.is_empty(), "seed script did not create problems");
    assert!(
        messages
            .iter()
            .any(|message| message.title.contains("High CPU load on web-01")),
        "web-01 demo problem missing: {:?}",
        messages
            .iter()
            .map(|message| &message.title)
            .collect::<Vec<_>>()
    );

    let web = messages
        .iter()
        .find(|message| message.title.contains("High CPU load on web-01"))
        .expect("web-01 problem present")
        .clone();
    assert!(matches!(
        web.classification,
        Classification::Critical | Classification::Acknowledged
    ));

    let ackable = messages
        .iter()
        .find(|message| message.can_acknowledge())
        .cloned();

    // Case-insensitive filters must hide matching messages.
    let mut filtered = configuration.clone();
    filtered.set_text("zabbix.filter", "nginx is not running");
    let mut filtered_server = ZabbixServer::new(filtered);
    let filtered_messages = filtered_server.poll().expect("filtered poll");
    assert!(
        filtered_messages
            .iter()
            .all(|message| !message.title.contains("nginx")),
        "filter did not hide nginx problem"
    );

    if let Some(ackable) = ackable {
        server
            .acknowledge(
                ackable.event_id.as_deref().unwrap(),
                "observo integration test",
            )
            .expect("acknowledge");
        let after = server.poll().expect("poll after acknowledge");
        let acknowledged = after
            .iter()
            .find(|message| message.event_id == ackable.event_id)
            .expect("acknowledged event still present");
        assert!(acknowledged.acknowledged);
        assert_eq!(acknowledged.classification, Classification::Acknowledged);
    } else {
        eprintln!("all demo problems are acknowledged; skipping acknowledge assertion");
    }

    // Exercise the same background worker the GUI uses.
    let mut worker = PollHandle::start("zabbix", configuration);
    let events = worker.events();
    let mut saw_poll = false;
    let mut second_event = None;
    while let Ok(event) = events.recv_blocking() {
        match event {
            PollEvent::Polled {
                messages,
                error: None,
                ..
            } => {
                saw_poll = true;
                if second_event.is_none() {
                    second_event = messages
                        .iter()
                        .find(|message| message.can_acknowledge())
                        .and_then(|message| message.event_id.clone());
                    if let Some(event_id) = &second_event {
                        worker.acknowledge(event_id.clone(), "observo worker test");
                    }
                }
            }
            PollEvent::Acknowledged { result, .. } => {
                result.expect("worker acknowledge");
                break;
            }
            PollEvent::Polled {
                error: Some(error), ..
            } => {
                panic!("worker poll failed: {error}");
            }
            _ => {}
        }
    }
    assert!(saw_poll, "worker did not deliver a poll event");
    worker.stop();
}

use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use log::{debug, error, info};

use crate::config::Configuration;
use crate::model::Message;
use crate::monitor::{self, MonitorError};

#[derive(Debug)]
pub enum PollCommand {
    UpdateConfiguration(Configuration),
    PollNow,
    Acknowledge { event_id: String, comment: String },
    Stop,
}

#[derive(Debug)]
pub enum PollEvent {
    Polled {
        at: Instant,
        messages: Vec<Message>,
        error: Option<MonitorError>,
    },
    Acknowledged {
        event_id: String,
        result: Result<(), MonitorError>,
    },
    ConfigurationApplied,
}

/// Owns the background polling thread.
///
/// The thread is intentionally not joined on drop: a quit must never wait for
/// a slow HTTP request. Sending `Stop` is enough because the process is about to
/// end and the thread has no work that needs to be flushed.
pub struct PollHandle {
    commands: Sender<PollCommand>,
    events: async_channel::Receiver<PollEvent>,
    thread: Option<JoinHandle<()>>,
}

impl PollHandle {
    pub fn start(provider_id: &str, configuration: Configuration) -> Self {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (event_tx, event_rx) = async_channel::unbounded();
        let provider = monitor::provider(provider_id);
        let thread = thread::Builder::new()
            .name("observo-poll".to_string())
            .spawn(move || poll_loop(provider, configuration, command_rx, event_tx))
            .expect("could not start polling thread");
        Self {
            commands: command_tx,
            events: event_rx,
            thread: Some(thread),
        }
    }

    pub fn events(&self) -> async_channel::Receiver<PollEvent> {
        self.events.clone()
    }

    pub fn send(&self, command: PollCommand) -> bool {
        self.commands.send(command).is_ok()
    }

    pub fn update_configuration(&self, configuration: Configuration) -> bool {
        self.send(PollCommand::UpdateConfiguration(configuration))
    }

    pub fn poll_now(&self) -> bool {
        self.send(PollCommand::PollNow)
    }

    pub fn acknowledge(&self, event_id: impl Into<String>, comment: impl Into<String>) -> bool {
        self.send(PollCommand::Acknowledge {
            event_id: event_id.into(),
            comment: comment.into(),
        })
    }

    pub fn stop(&mut self) {
        let _ = self.commands.send(PollCommand::Stop);
        if let Some(thread) = self.thread.take() {
            // The worker may currently be inside an HTTP request. Detach rather
            // than blocking the UI thread during shutdown.
            std::mem::drop(thread);
        }
    }
}

impl Drop for PollHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn poll_loop(
    provider: &'static dyn monitor::ServerProvider,
    initial_configuration: Configuration,
    commands: Receiver<PollCommand>,
    events: async_channel::Sender<PollEvent>,
) {
    let mut configuration = initial_configuration;
    let mut interval = configuration.poll_interval();
    let mut server = provider.create_server(configuration.clone());
    let mut next_poll = Instant::now();

    loop {
        let now = Instant::now();
        if now >= next_poll {
            let started = Instant::now();
            let result = server.poll();
            let (messages, error) = match result {
                Ok(messages) => {
                    debug!(
                        "poll finished in {:?} with {} messages",
                        started.elapsed(),
                        messages.len()
                    );
                    (messages, None)
                }
                Err(error) => {
                    error!("Zabbix poll failed: {error} (details: {:?})", error.details);
                    (Vec::new(), Some(error))
                }
            };
            if events
                .send_blocking(PollEvent::Polled {
                    at: Instant::now(),
                    messages,
                    error,
                })
                .is_err()
            {
                break;
            }
            next_poll = Instant::now() + interval;
        }

        let timeout = next_poll.saturating_duration_since(Instant::now());
        match commands.recv_timeout(timeout) {
            Ok(PollCommand::UpdateConfiguration(new_configuration)) => {
                if new_configuration != configuration {
                    info!(
                        "applying new server configuration (interval {:?})",
                        new_configuration.poll_interval()
                    );
                    configuration = new_configuration;
                    interval = configuration.poll_interval();
                    server.set_configuration(configuration.clone());
                    if events
                        .send_blocking(PollEvent::ConfigurationApplied)
                        .is_err()
                    {
                        break;
                    }
                }
                next_poll = Instant::now();
            }
            Ok(PollCommand::PollNow) => {
                next_poll = Instant::now();
            }
            Ok(PollCommand::Acknowledge { event_id, comment }) => {
                let result = server.acknowledge(&event_id, &comment);
                if let Err(error) = &result {
                    error!("acknowledge failed: {error}");
                }
                if events
                    .send_blocking(PollEvent::Acknowledged { event_id, result })
                    .is_err()
                {
                    break;
                }
                // Refresh quickly so the acknowledged state becomes visible.
                next_poll = Instant::now() + Duration::from_millis(250);
            }
            Ok(PollCommand::Stop) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    debug!("polling thread stopped");
}

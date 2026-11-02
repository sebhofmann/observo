use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::base::{Disableable as _, StyledExt as _};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Theme, ThemeMode, WindowExt,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    notification::{Notification, NotificationType},
    table::{DataTable, TableEvent, TableState},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, Hsla,
    InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Task, Window, div, px,
};
use log::{error, warn};

use crate::config::Configuration;
use crate::i18n::{I18n, Language};
use crate::model::{Classification, MAX_ACK_COMMENT_CHARS, Message};
use crate::monitor::MonitorError;
use crate::notify;
use crate::poll::{PollEvent, PollHandle};
use crate::settings::{Settings, ThemePreference};
use crate::ui::table::{MessagesTable, classification_color};
use crate::ui::widgets::{badge, icon_tile, metric, separator, status_pill};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectionStatus {
    #[default]
    Connecting,
    Online,
    Offline,
}

/// What a successful poll changed compared to the polls before it.
struct PollSuccess {
    new_alerts: HashSet<String>,
    restored: bool,
}

/// Remembers poll results between polls. The displayed status also passes
/// through `Connecting` on manual refreshes, so connection changes are detected
/// here against the last real result instead.
#[derive(Default)]
struct PollTracker {
    settled: ConnectionStatus,
    known_alerts: HashSet<String>,
    baseline_ready: bool,
}

impl PollTracker {
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Returns whether this failure ended a working connection.
    fn record_failure(&mut self) -> bool {
        let lost = self.settled == ConnectionStatus::Online;
        self.settled = ConnectionStatus::Offline;
        lost
    }

    fn record_success(&mut self, alerts: HashSet<String>) -> PollSuccess {
        let restored = self.settled == ConnectionStatus::Offline;
        self.settled = ConnectionStatus::Online;
        // The first successful poll only seeds the baseline: problems that
        // already existed when the app started must not notify.
        let new_alerts = if self.baseline_ready {
            alerts.difference(&self.known_alerts).cloned().collect()
        } else {
            HashSet::new()
        };
        self.baseline_ready = true;
        self.known_alerts = alerts;
        PollSuccess {
            new_alerts,
            restored,
        }
    }
}

pub struct ObservoApp {
    pub settings: Settings,
    pub i18n: I18n,
    pub messages: Vec<Message>,
    pub rows: Vec<Message>,
    pub selected_id: Option<String>,
    pub search_text: String,
    pub filter: Option<Classification>,
    pub only_open: bool,
    pub status: ConnectionStatus,
    pub last_error: Option<MonitorError>,
    pub last_update: Option<Instant>,
    pub last_attempt: Option<Instant>,
    pub ack_in_flight: bool,
    pub table: Entity<TableState<MessagesTable>>,
    pub search: Entity<InputState>,
    pub focus_handle: gpui_kit::FocusHandle,
    poll: PollHandle,
    tracker: PollTracker,
    _subscriptions: Vec<Subscription>,
    _poll_task: Task<()>,
    _clock_task: Task<()>,
}

impl ObservoApp {
    pub fn new(settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let i18n = I18n::new(settings.language);
        let focus_handle = cx.focus_handle();

        let table = cx.new(|cx| {
            TableState::new(MessagesTable::new(settings.language), window, cx)
                .row_selectable(true)
                .col_selectable(false)
                .cell_selectable(false)
                .sortable(true)
                .col_movable(false)
                .col_resizable(true)
                .row_header(false)
        });

        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(i18n.t("main.searchPlaceholder")));

        let table_subscription = cx.subscribe_in(
            &table,
            window,
            |this, table, event: &TableEvent, window, cx| match event {
                TableEvent::SelectRow(row) => {
                    this.selected_id = table
                        .read(cx)
                        .delegate()
                        .message(*row)
                        .map(|message| message.id.clone());
                    this.sync_table_layout(cx);
                    cx.notify();
                }
                TableEvent::DoubleClickedRow(row) => {
                    if let Some(message) = table.read(cx).delegate().message(*row).cloned() {
                        this.selected_id = Some(message.id.clone());
                        this.sync_table_layout(cx);
                        this.open_acknowledge(message, window, cx);
                    }
                    cx.notify();
                }
                TableEvent::ClearSelection => {
                    this.selected_id = None;
                    this.sync_table_layout(cx);
                    cx.notify();
                }
                _ => {}
            },
        );

        let search_subscription = cx.subscribe_in(
            &search,
            window,
            |this, state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.search_text = state.read(cx).value().to_string();
                    this.refresh_rows(cx);
                    cx.notify();
                }
            },
        );

        let poll = PollHandle::start(&settings.provider_id, settings.server.clone());
        let events = poll.events();
        let poll_task = cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update_in(cx, |this, window, cx| {
                        this.handle_poll_event(event, window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        // The footer countdown and "last updated" label are derived from the
        // clock, so they need a redraw even when no event arrives.
        let clock_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });

        let mut app = Self {
            settings,
            i18n,
            messages: Vec::new(),
            rows: Vec::new(),
            selected_id: None,
            search_text: String::new(),
            filter: None,
            only_open: false,
            status: ConnectionStatus::Connecting,
            last_error: None,
            last_update: None,
            last_attempt: None,
            ack_in_flight: false,
            table,
            search,
            focus_handle,
            poll,
            tracker: PollTracker::default(),
            _subscriptions: vec![table_subscription, search_subscription],
            _poll_task: poll_task,
            _clock_task: clock_task,
        };
        app.apply_theme(window, cx);

        let appearance_weak = cx.entity().downgrade();
        let appearance_subscription = window.observe_window_appearance(move |window, cx| {
            appearance_weak
                .update(cx, |this, cx| {
                    if this.settings.theme == ThemePreference::System {
                        this.apply_theme(window, cx);
                    }
                })
                .ok();
        });
        app._subscriptions.push(appearance_subscription);

        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let minimize = weak
                .read_with(cx, |this, _| this.settings.minimize_on_close)
                .unwrap_or(false);
            weak.update(cx, |this, _| {
                this.capture_window(window);
                if let Err(error) = this.settings.save() {
                    error!("could not save settings on close: {error}");
                }
            })
            .ok();
            if minimize {
                window.minimize_window();
                false
            } else {
                true
            }
        });

        app
    }

    fn capture_window(&mut self, window: &Window) {
        // A maximized window reports the screen size as its bounds. Persist that
        // state separately and keep the last windowed geometry as restore size.
        self.settings.window.maximized = window.is_maximized();
        if !self.settings.window.maximized {
            let bounds = window.bounds();
            self.settings.window.x = Some(bounds.origin.x.as_f32() as f64);
            self.settings.window.y = Some(bounds.origin.y.as_f32() as f64);
            self.settings.window.width = bounds.size.width.as_f32() as f64;
            self.settings.window.height = bounds.size.height.as_f32() as f64;
        }
        self.settings.window = self.settings.window.clone().normalised();
    }

    fn apply_theme(&self, window: &mut Window, cx: &mut App) {
        let mode = match self.settings.theme {
            ThemePreference::System => ThemeMode::from(window.appearance()),
            ThemePreference::Light => ThemeMode::Light,
            ThemePreference::Dark => ThemeMode::Dark,
        };
        Theme::change(mode, Some(window), cx);
    }

    fn handle_poll_event(&mut self, event: PollEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            PollEvent::Polled {
                at,
                messages,
                error,
            } => {
                self.last_attempt = Some(at);
                if let Some(error) = error {
                    self.status = ConnectionStatus::Offline;
                    self.last_error = Some(error);
                    if self.tracker.record_failure() {
                        self.notify_connection_lost(window, cx);
                    }
                } else {
                    self.status = ConnectionStatus::Online;
                    self.last_error = None;
                    self.last_update = Some(at);
                    self.messages = messages;
                    self.refresh_rows(cx);

                    // Notifications are based on all server-filtered problems,
                    // not on the transient search/severity view.
                    let current_alerts: HashSet<String> = self
                        .messages
                        .iter()
                        .filter(|message| message.is_alert())
                        .map(|message| message.id.clone())
                        .collect();
                    let PollSuccess {
                        new_alerts,
                        restored,
                    } = self.tracker.record_success(current_alerts);
                    if !new_alerts.is_empty() {
                        let new_critical = self
                            .messages
                            .iter()
                            .filter(|message| {
                                message.severity >= 4 && new_alerts.contains(&message.id)
                            })
                            .count();
                        self.notify_new_problems(new_alerts.len(), new_critical, window, cx);
                    }

                    if restored {
                        self.notify_connection_restored(window, cx);
                    }
                }
                cx.notify();
            }
            PollEvent::Acknowledged { event_id, result } => {
                self.ack_in_flight = false;
                match result {
                    Ok(()) => {
                        if let Some(message) = self
                            .messages
                            .iter_mut()
                            .find(|message| message.event_id.as_deref() == Some(&event_id))
                        {
                            message.acknowledged = true;
                            message.classification = Classification::Acknowledged;
                        }
                        self.refresh_rows(cx);
                        window.push_notification(
                            Notification::new()
                                .title(self.i18n.t("notification.ackSuccess.title"))
                                .message(self.i18n.t("notification.ackSuccess.text"))
                                .with_type(NotificationType::Success),
                            cx,
                        );
                        self.poll.poll_now();
                    }
                    Err(error) => {
                        warn!("acknowledge failed: {error}");
                        window.push_notification(
                            Notification::new()
                                .title(self.i18n.t("notification.ackError.title"))
                                .message(
                                    self.i18n.tf(
                                        "notification.ackError.text",
                                        &[("error", &error.message)],
                                    ),
                                )
                                .with_type(NotificationType::Error),
                            cx,
                        );
                    }
                }
                cx.notify();
            }
            PollEvent::ConfigurationApplied => {
                // A poll that was running during the change may have delivered
                // results of the old server in the meantime.
                self.forget_server_state(cx);
                cx.notify();
            }
        }
    }

    fn notify_new_problems(
        &self,
        count: usize,
        critical: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.settings.notifications {
            return;
        }
        log::debug!("notifying about {count} new problems ({critical} critical)");
        let title = self.i18n.t("notification.newProblems.title");
        let message = self.i18n.tf(
            "notification.newProblems.text",
            &[
                ("count", &count.to_string()),
                ("critical", &critical.to_string()),
            ],
        );
        window.push_notification(
            Notification::new()
                .title(title)
                .message(message)
                .with_type(if critical > 0 {
                    NotificationType::Warning
                } else {
                    NotificationType::Info
                })
                .system(),
            cx,
        );
        if self.settings.sound {
            notify::play_bell();
        }
    }

    fn notify_connection_lost(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.notifications {
            return;
        }
        window.push_notification(
            Notification::new()
                .title(self.i18n.t("notification.connectionLost.title"))
                .message(self.i18n.t("notification.connectionLost.text"))
                .with_type(NotificationType::Error)
                .system(),
            cx,
        );
    }

    fn notify_connection_restored(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.notifications {
            return;
        }
        window.push_notification(
            Notification::new()
                .title(self.i18n.t("notification.connectionRestored.title"))
                .message(self.i18n.t("notification.connectionRestored.text"))
                .with_type(NotificationType::Success),
            cx,
        );
    }

    fn refresh_rows(&mut self, cx: &mut Context<Self>) {
        let query = self.search_text.trim().to_lowercase();
        let filter = self.filter;
        let only_open = self.only_open;
        let rows: Vec<Message> = self
            .messages
            .iter()
            .filter(|message| matches_filters(message, &query, filter, only_open))
            .cloned()
            .collect();

        if let Some(selected) = &self.selected_id
            && !rows.iter().any(|message| &message.id == selected)
        {
            self.selected_id = None;
        }
        log::debug!(
            "refresh_rows: {} messages, {} rows",
            self.messages.len(),
            rows.len()
        );
        self.rows = rows;

        let selected_id = self.selected_id.clone();
        let table_rows = self.rows.clone();
        let compact = self.selected_id.is_some();
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_messages(table_rows);
            let layout_changed = table.delegate_mut().set_compact(compact);
            if layout_changed {
                table.refresh(cx);
            }
            // The delegate applies its own column sort, so the row index is only
            // known after set_messages.
            let selected_index = selected_id
                .as_deref()
                .and_then(|id| table.delegate().index_of(id));
            match selected_index {
                // Selecting a row scrolls to it, which must not happen on every poll.
                Some(index) if table.selected_row() == Some(index) => {}
                Some(index) => table.set_selected_row(index, cx),
                None => table.clear_selection(cx),
            }
            cx.notify();
        });
    }

    fn sync_table_layout(&mut self, cx: &mut Context<Self>) {
        let compact = self.selected_id.is_some();
        self.table.update(cx, |table, cx| {
            if table.delegate_mut().set_compact(compact) {
                table.refresh(cx);
            }
            cx.notify();
        });
    }

    fn close_details(&mut self, cx: &mut Context<Self>) {
        self.selected_id = None;
        self.table.update(cx, |table, cx| {
            if table.delegate_mut().set_compact(false) {
                table.refresh(cx);
            }
            table.clear_selection(cx);
        });
        cx.notify();
    }

    fn set_filter(&mut self, filter: Option<Classification>, cx: &mut Context<Self>) {
        if self.filter == filter {
            return;
        }
        self.filter = filter;
        self.refresh_rows(cx);
        cx.notify();
    }

    fn set_only_open(&mut self, only_open: bool, cx: &mut Context<Self>) {
        if self.only_open == only_open {
            return;
        }
        self.only_open = only_open;
        self.refresh_rows(cx);
        cx.notify();
    }

    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_text.clear();
        self.filter = None;
        self.only_open = false;
        self.search.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.refresh_rows(cx);
        cx.notify();
    }

    fn counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut critical = 0;
        let mut warning = 0;
        let mut info = 0;
        let mut acknowledged = 0;
        let mut recovered = 0;
        for message in &self.messages {
            match message.classification {
                Classification::Critical => critical += 1,
                Classification::Warning => warning += 1,
                Classification::Info => info += 1,
                Classification::Acknowledged => acknowledged += 1,
                Classification::Recovery => recovered += 1,
                Classification::Unknown => {}
            }
        }
        (critical, warning, info, acknowledged, recovered)
    }

    fn selected_message(&self) -> Option<&Message> {
        let id = self.selected_id.as_ref()?;
        self.rows.iter().find(|message| &message.id == id)
    }

    /// Drops everything that belongs to the previously configured server, so
    /// that none of its problems stay visible or can be acknowledged.
    fn forget_server_state(&mut self, cx: &mut Context<Self>) {
        self.status = ConnectionStatus::Connecting;
        self.last_error = None;
        self.last_update = None;
        self.tracker.reset();
        self.messages.clear();
        self.selected_id = None;
        self.refresh_rows(cx);
    }

    fn refresh_now(&mut self, cx: &mut Context<Self>) {
        self.status = ConnectionStatus::Connecting;
        self.poll.poll_now();
        cx.notify();
    }

    pub(crate) fn apply_server_configuration(
        &mut self,
        configuration: Configuration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let provider = crate::monitor::provider(&self.settings.provider_id);
        let errors = provider.validate(&configuration);
        if !errors.is_empty() {
            return Err(self.i18n.t("serverConfig.invalid").to_string());
        }
        let configuration = configuration.normalised();
        let changed = configuration != self.settings.server;
        self.settings.server = configuration.clone();
        self.settings.provider_id = provider.id().to_string();
        if let Err(error) = self.settings.save() {
            return Err(self.i18n.tf(
                "notification.settingsError.text",
                &[("error", &error.to_string())],
            ));
        }
        if changed {
            self.forget_server_state(cx);
            self.poll.update_configuration(configuration);
        } else {
            self.refresh_now(cx);
        }
        window.push_notification(
            Notification::new()
                .message(self.i18n.t("serverConfig.saved"))
                .with_type(NotificationType::Success),
            cx,
        );
        cx.notify();
        Ok(())
    }

    pub(crate) fn start_acknowledge(
        &mut self,
        message: Message,
        comment: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ack_in_flight {
            return;
        }
        let comment: String = comment.chars().take(MAX_ACK_COMMENT_CHARS).collect();
        let Some(event_id) = message.event_id.clone() else {
            window.push_notification(
                Notification::new()
                    .title(self.i18n.t("acknowledge.notAcknowledgeable.title"))
                    .message(self.i18n.t("acknowledge.notAcknowledgeable.text"))
                    .with_type(NotificationType::Warning),
                cx,
            );
            return;
        };
        self.ack_in_flight = true;
        if !self.poll.acknowledge(event_id, comment) {
            self.ack_in_flight = false;
            window.push_notification(
                Notification::new()
                    .title(self.i18n.t("notification.ackError.title"))
                    .message(self.i18n.t("acknowledge.error.text"))
                    .with_type(NotificationType::Error),
                cx,
            );
        }
        cx.notify();
    }

    pub(crate) fn set_language(
        &mut self,
        language: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.language == language {
            return;
        }
        self.settings.language = language;
        self.i18n.set_language(language);
        self.search.update(cx, |state, cx| {
            state.set_placeholder(self.i18n.t("main.searchPlaceholder"), window, cx);
        });
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_language(language);
            table.refresh(cx);
            table.refresh_header_layout(cx);
            cx.notify();
        });
        self.persist_settings(window, cx);
        cx.notify();
    }

    pub(crate) fn set_theme(
        &mut self,
        theme: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.theme == theme {
            return;
        }
        log::debug!("applying theme preference {theme:?}");
        self.settings.theme = theme;
        self.apply_theme(window, cx);
        self.persist_settings(window, cx);
        cx.notify();
    }

    pub(crate) fn set_notifications(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.notifications = enabled;
        self.persist_settings(window, cx);
        cx.notify();
    }

    pub(crate) fn set_sound(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.sound = enabled;
        self.persist_settings(window, cx);
        cx.notify();
    }

    pub(crate) fn set_minimize_on_close(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.minimize_on_close = enabled;
        self.persist_settings(window, cx);
        cx.notify();
    }

    fn persist_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.settings.save() {
            error!("could not save settings: {error}");
            window.push_notification(
                Notification::new()
                    .title(self.i18n.t("notification.settingsError.title"))
                    .message(self.i18n.tf(
                        "notification.settingsError.text",
                        &[("error", &error.to_string())],
                    ))
                    .with_type(NotificationType::Error),
                cx,
            );
        }
    }

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::ui::dialogs::open_settings(self, window, cx);
    }

    pub(crate) fn open_server_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::ui::dialogs::open_server_config(self, window, cx);
    }

    pub(crate) fn open_acknowledge(
        &mut self,
        message: Message,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::ui::dialogs::open_acknowledge(self, message, window, cx);
    }

    pub(crate) fn open_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::ui::dialogs::open_about(self, window, cx);
    }

    /// Development aid used by the screenshot workflow in CI/manual QA.
    /// Set `OBSERVO_DEBUG_DIALOG=settings|server|ack|details|about`.
    #[cfg(debug_assertions)]
    pub fn debug_open_dialog(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        match which {
            "settings" => self.open_settings(window, cx),
            "server" => self.open_server_config(window, cx),
            "about" => self.open_about(window, cx),
            "details" | "ack" => {
                if let Some(message) = self.rows.first().cloned() {
                    self.selected_id = Some(message.id.clone());
                    let index = 0;
                    self.table.update(cx, |table, cx| {
                        table.set_selected_row(index, cx);
                    });
                    if which == "ack" {
                        self.open_acknowledge(message, window, cx);
                    }
                }
            }
            _ => {}
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let (status_label, status_color, status_background) = match self.status {
            ConnectionStatus::Online => (
                self.i18n.t("main.connected"),
                theme.success,
                theme.success.opacity(0.12),
            ),
            ConnectionStatus::Connecting => (
                self.i18n.t("main.connecting"),
                theme.info,
                theme.info.opacity(0.12),
            ),
            ConnectionStatus::Offline => (
                self.i18n.t("main.offline"),
                theme.danger,
                theme.danger.opacity(0.12),
            ),
        };
        let last_update = self
            .last_update
            .map(|instant| {
                let elapsed = instant.elapsed();
                let pseudo =
                    chrono::Utc::now() - chrono::Duration::from_std(elapsed).unwrap_or_default();
                self.i18n.relative_time(pseudo)
            })
            .unwrap_or_else(|| self.i18n.t("main.neverUpdated").to_string());

        div()
            .h(px(70.))
            .px_5()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(icon_tile(
                IconName::Activity,
                theme.primary,
                theme.primary.opacity(0.12),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(self.i18n.t("app.title")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.i18n.t("app.subtitle")),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap_1()
                    .child(status_pill(status_label, status_color, status_background))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.i18n.tf("main.lastUpdated", &[("time", &last_update)])),
                    ),
            )
            .child(separator(cx))
            .child(
                Button::new("refresh")
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .accessibility_label(self.i18n.t("main.refresh"))
                    .tooltip(self.i18n.t("main.refreshTooltip"))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.refresh_now(cx);
                    })),
            )
            .child(
                Button::new("theme")
                    .ghost()
                    .icon(
                        if matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Dark) {
                            IconName::Sun
                        } else {
                            IconName::Moon
                        },
                    )
                    .accessibility_label(self.i18n.t("main.theme"))
                    .tooltip(self.i18n.t("main.themeTooltip"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let next =
                            if matches!(cx.theme().mode, gpui_kit::component::ThemeMode::Dark) {
                                ThemePreference::Light
                            } else {
                                ThemePreference::Dark
                            };
                        this.set_theme(next, window, cx);
                    })),
            )
            .child(
                Button::new("server")
                    .ghost()
                    .icon(IconName::Server)
                    .accessibility_label(self.i18n.t("main.serverConfig"))
                    .tooltip(self.i18n.t("main.serverConfigTooltip"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_server_config(window, cx);
                    })),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .icon(IconName::Settings)
                    .accessibility_label(self.i18n.t("main.settings"))
                    .tooltip(self.i18n.t("main.settingsTooltip"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(window, cx);
                    })),
            )
            .child(
                Button::new("about")
                    .ghost()
                    .icon(IconName::Info)
                    .accessibility_label(self.i18n.t("about.title"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_about(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let (critical, warning, info, acknowledged, recovered) = self.counts();
        let total = self.messages.len();
        let filters: [(Option<Classification>, &str, usize); 6] = [
            (None, "main.all", total),
            (
                Some(Classification::Critical),
                "classification.critical",
                critical,
            ),
            (
                Some(Classification::Warning),
                "classification.warning",
                warning,
            ),
            (Some(Classification::Info), "classification.info", info),
            (
                Some(Classification::Acknowledged),
                "classification.acknowledged",
                acknowledged,
            ),
            (
                Some(Classification::Recovery),
                "classification.recovery",
                recovered,
            ),
        ];

        let mut chips = div().flex().items_center().gap_1();
        for (filter, label_key, count) in filters {
            let active = self.filter == filter;
            let label = format!("{}  {}", self.i18n.t(label_key), count);
            let mut button = Button::new(format!("filter-{label_key}"))
                .small()
                .label(label)
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.set_filter(filter, cx);
                }));
            button = if active {
                button.primary()
            } else {
                button.ghost()
            };
            chips = chips.child(button);
        }

        let selected = self.selected_message();
        let ack_enabled = selected
            .map(|message| message.can_acknowledge() && !self.ack_in_flight)
            .unwrap_or(false);
        let ack_label = if self.ack_in_flight {
            self.i18n.t("main.ackInFlight")
        } else {
            self.i18n.t("main.acknowledge")
        };

        let mut ack_button = Button::new("acknowledge")
            .small()
            .icon(IconName::Check)
            .label(ack_label)
            .accessibility_label(self.i18n.t("main.acknowledge"))
            .on_click(cx.listener(|this, _, window, cx| {
                if let Some(message) = this.selected_message().cloned() {
                    this.open_acknowledge(message, window, cx);
                }
            }));
        ack_button = if ack_enabled {
            ack_button.primary()
        } else {
            ack_button.ghost().disabled(true)
        };

        div()
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .gap_3()
            .bg(theme.background)
            .child(
                div().w(px(280.)).child(
                    Input::new(&self.search)
                        .prefix(Icon::new(IconName::Search).size_4())
                        .cleanable(true),
                ),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(theme.border))
            .child(chips)
            .child(div().flex_1())
            .child(
                Button::new("only-open")
                    .small()
                    .label(self.i18n.t("main.unacknowledged"))
                    .when(self.only_open, |button| button.primary())
                    .when(!self.only_open, |button| button.ghost())
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.set_only_open(!this.only_open, cx);
                    })),
            )
            .child(ack_button)
            .into_any_element()
    }

    fn render_error_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let error = self.last_error.as_ref()?;
        let theme = cx.theme().clone();
        let message = match error.kind {
            crate::monitor::ErrorKind::Http(status) => self
                .i18n
                .tf(error.i18n_key(), &[("status", &status.to_string())]),
            _ => self.i18n.t(error.i18n_key()).to_string(),
        };
        let details = self
            .i18n
            .tf("error.details", &[("details", &error.detail_text())]);
        Some(
            div()
                .mx_4()
                .mt_3()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.danger.opacity(0.35))
                .bg(theme.danger.opacity(0.08))
                .flex()
                .items_center()
                .gap_3()
                .child(
                    Icon::new(IconName::WifiOff)
                        .size_5()
                        .text_color(theme.danger),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(theme.danger)
                                .child(message),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(details),
                        ),
                )
                .child(div().flex_1())
                .child(
                    Button::new("retry")
                        .small()
                        .outline()
                        .label(self.i18n.t("error.retry"))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.refresh_now(cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let has_filters =
            !self.search_text.trim().is_empty() || self.filter.is_some() || self.only_open;
        let (icon, color, title, text) =
            if self.status == ConnectionStatus::Offline && self.messages.is_empty() {
                (
                    IconName::WifiOff,
                    theme.danger,
                    self.i18n.t("error.title"),
                    self.i18n.t("error.network"),
                )
            } else if self.status == ConnectionStatus::Connecting && self.messages.is_empty() {
                (
                    IconName::RefreshCw,
                    theme.info,
                    self.i18n.t("main.connecting"),
                    self.i18n.t("main.neverUpdated"),
                )
            } else if has_filters {
                (
                    IconName::Search,
                    theme.info,
                    self.i18n.t("main.noResults.title"),
                    self.i18n.t("main.noResults.text"),
                )
            } else {
                (
                    IconName::CircleCheck,
                    theme.success,
                    self.i18n.t("main.noProblems.title"),
                    self.i18n.t("main.noProblems.text"),
                )
            };

        let mut actions = div().flex().items_center().gap_2();
        if has_filters {
            actions = actions.child(
                Button::new("clear-filters")
                    .outline()
                    .label(self.i18n.t("main.clearFilters"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_filters(window, cx);
                    })),
            );
        } else {
            actions = actions.child(
                Button::new("configure")
                    .outline()
                    .icon(IconName::Server)
                    .label(self.i18n.t("main.serverConfig"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_server_config(window, cx);
                    })),
            );
        }
        actions = actions.child(
            Button::new("refresh-empty")
                .ghost()
                .icon(IconName::RefreshCw)
                .label(self.i18n.t("main.refresh"))
                .on_click(cx.listener(|this, _, _window, cx| {
                    this.refresh_now(cx);
                })),
        );

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(icon_tile(icon, color, color.opacity(0.12)))
            .child(div().text_lg().font_semibold().child(title))
            .child(
                div()
                    .max_w(px(420.))
                    .text_sm()
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child(text),
            )
            .child(actions)
            .into_any_element()
    }

    fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(message) = self.selected_message() else {
            return div().into_any_element();
        };
        let color = classification_color(message.classification, cx);
        let local = message.timestamp.with_timezone(&chrono::Local);
        let ack_enabled = message.can_acknowledge() && !self.ack_in_flight;

        let mut footer = div().flex().items_center().gap_2();
        let mut ack = Button::new("details-ack")
            .icon(IconName::Check)
            .label(if self.ack_in_flight {
                self.i18n.t("main.ackInFlight")
            } else {
                self.i18n.t("main.acknowledge")
            })
            .on_click(cx.listener(|this, _, window, cx| {
                if let Some(message) = this.selected_message().cloned() {
                    this.open_acknowledge(message, window, cx);
                }
            }));
        ack = if ack_enabled {
            ack.primary()
        } else {
            ack.ghost().disabled(true)
        };
        footer = footer.child(ack);
        if !ack_enabled {
            footer = footer.child(div().text_xs().text_color(theme.muted_foreground).child(
                if message.acknowledged {
                    self.i18n.t("main.acknowledged")
                } else if message.recovered {
                    self.i18n.t("main.recovered")
                } else {
                    self.i18n.t("acknowledge.notAcknowledgeable.text")
                },
            ));
        }

        div()
            .w(px(370.))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.muted.opacity(0.25))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(48.))
                    .px_4()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child(self.i18n.t("main.details")),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("close-details")
                            .ghost()
                            .xsmall()
                            .icon(IconName::X)
                            .accessibility_label(self.i18n.t("common.close"))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.close_details(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("details-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().flex().child(badge(
                                self.i18n.classification(message.classification),
                                color,
                            )))
                            .child(
                                div()
                                    .text_base()
                                    .font_semibold()
                                    .child(message.title.clone()),
                            )
                            .child(div().text_xs().text_color(theme.muted_foreground).child(
                                format!(
                                    "{} · {}",
                                    message.hostname,
                                    local.format("%d.%m.%Y %H:%M:%S")
                                ),
                            )),
                    )
                    .child(div().text_sm().child(message.message.clone()))
                    .child(separator(cx))
                    .child(detail_row(
                        self.i18n.t("main.classification"),
                        self.i18n.classification(message.classification),
                        theme.muted_foreground,
                    ))
                    .child(detail_row(
                        self.i18n.t("main.severity"),
                        format!("{} / 5", message.severity),
                        theme.muted_foreground,
                    ))
                    .child(detail_row(
                        self.i18n.t("main.eventId"),
                        message
                            .event_id
                            .clone()
                            .unwrap_or_else(|| self.i18n.t("common.none").to_string()),
                        theme.muted_foreground,
                    ))
                    .child(detail_row(
                        self.i18n.t("main.triggerId"),
                        message.trigger_id.clone(),
                        theme.muted_foreground,
                    ))
                    .child(detail_row(
                        self.i18n.t("main.status"),
                        self.i18n.t(status_key(message)),
                        theme.muted_foreground,
                    ))
                    .when(message.maintenance, |element| {
                        element.child(
                            div()
                                .flex()
                                .child(badge(self.i18n.t("main.maintenance"), theme.info)),
                        )
                    })
                    .when(message.recovered, |element| {
                        element.child(
                            div()
                                .flex()
                                .child(badge(self.i18n.t("main.recovered"), theme.success)),
                        )
                    }),
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(footer),
            )
            .into_any_element()
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let (critical, warning, info, _acknowledged, _recovered) = self.counts();
        let muted = theme.muted_foreground;
        let interval = self.settings.poll_interval().as_secs();
        let next_poll = self
            .last_attempt
            .or(self.last_update)
            .map(|last| {
                let elapsed = last.elapsed().as_secs();
                interval.saturating_sub(elapsed)
            })
            .unwrap_or(interval);

        div()
            .h(px(34.))
            .px_4()
            .flex()
            .items_center()
            .gap_4()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(metric(
                IconName::TriangleAlert,
                critical.to_string(),
                self.i18n.t("metrics.critical"),
                theme.danger,
                muted,
            ))
            .child(metric(
                IconName::CircleAlert,
                warning.to_string(),
                self.i18n.t("metrics.warning"),
                theme.warning,
                muted,
            ))
            .child(metric(
                IconName::Info,
                info.to_string(),
                self.i18n.t("metrics.info"),
                theme.info,
                muted,
            ))
            .child(separator(cx))
            .child(
                div().text_xs().text_color(muted).child(
                    self.i18n
                        .tf("main.pollInterval", &[("seconds", &interval.to_string())]),
                ),
            )
            .child(
                div().text_xs().text_color(muted).child(
                    self.i18n
                        .tf("main.nextPoll", &[("seconds", &next_poll.to_string())]),
                ),
            )
            .child(div().flex_1())
            .when(
                !self.settings.server.text("zabbix.filter").trim().is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(theme.info)
                            .child(Icon::new(IconName::ListFilter).size_3())
                            .child(self.i18n.t("main.footer.filterActive")),
                    )
                },
            )
            .child(
                div()
                    .max_w(px(260.))
                    .truncate()
                    .text_xs()
                    .text_color(muted)
                    .child(self.settings.server.text("zabbix.url").to_string()),
            )
            .into_any_element()
    }
}

fn detail_row(label: &'static str, value: impl Into<SharedString>, muted: Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(div().text_xs().text_color(muted).child(label))
        .child(div().text_sm().child(value.into()))
        .into_any_element()
}

fn status_key(message: &Message) -> &'static str {
    if message.acknowledged {
        "main.acknowledged"
    } else if message.recovered {
        "main.recovered"
    } else if message.maintenance {
        "main.maintenance"
    } else {
        "main.open"
    }
}

impl gpui_kit::Render for ObservoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .track_focus(&self.focus_handle)
            .key_context("Observo")
            .on_action(cx.listener(|this, _: &Refresh, _window, cx| {
                this.refresh_now(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.open_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenServerConfig, window, cx| {
                this.open_server_config(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                this.capture_window(window);
                if let Err(error) = this.settings.save() {
                    error!("could not save settings on quit: {error}");
                }
                cx.quit();
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.focus_handle(cx).focus(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseDetails, _window, cx| {
                this.close_details(cx);
            }))
            .child(self.render_header(cx))
            .child(self.render_toolbar(cx));

        if let Some(banner) = self.render_error_banner(cx) {
            root = root.child(banner);
        }

        let content = if self.rows.is_empty() {
            self.render_empty(cx)
        } else {
            div()
                .flex_1()
                .min_h_0()
                .h_full()
                .overflow_hidden()
                .child(
                    DataTable::new(&self.table)
                        .with_size(px(44.))
                        .header_row_height(px(38.))
                        .stripe(false)
                        .bordered(false)
                        .scrollbar_visible(true, true),
                )
                .into_any_element()
        };

        let mut main = div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(div().flex_1().min_w_0().min_h_0().flex().child(content));
        if self.selected_message().is_some() {
            main = main.child(self.render_details(cx));
        }
        root.child(main).child(self.render_footer(cx))
    }
}

fn matches_filters(
    message: &Message,
    query: &str,
    filter: Option<Classification>,
    only_open: bool,
) -> bool {
    if only_open && (message.acknowledged || message.recovered) {
        return false;
    }
    if let Some(filter) = filter
        && message.classification != filter
    {
        return false;
    }
    query.is_empty()
        || message.title.to_lowercase().contains(query)
        || message.message.to_lowercase().contains(query)
        || message.hostname.to_lowercase().contains(query)
}

gpui_kit::actions!(
    observo,
    [
        Refresh,
        OpenSettings,
        OpenServerConfig,
        Quit,
        FocusSearch,
        CloseDetails
    ]
);

pub fn register_key_bindings(cx: &mut App) {
    use gpui_kit::KeyBinding;
    // GPUI's static KeyBinding::new does not expand the "secondary" alias, so
    // bind the platform-native modifier explicitly.
    cx.bind_keys([
        KeyBinding::new("ctrl-r", Refresh, Some("Observo")),
        KeyBinding::new("ctrl-,", OpenSettings, Some("Observo")),
        KeyBinding::new("ctrl-q", Quit, Some("Observo")),
        KeyBinding::new("ctrl-f", FocusSearch, Some("Observo")),
        KeyBinding::new("escape", CloseDetails, Some("Observo")),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-r", Refresh, Some("Observo")),
        KeyBinding::new("cmd-,", OpenSettings, Some("Observo")),
        KeyBinding::new("cmd-q", Quit, Some("Observo")),
        KeyBinding::new("cmd-f", FocusSearch, Some("Observo")),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn message(title: &str, host: &str, classification: Classification) -> Message {
        Message {
            id: title.to_string(),
            event_id: Some("1".to_string()),
            trigger_id: "1".to_string(),
            title: title.to_string(),
            message: "Disk usage /var (%): 96".to_string(),
            hostname: host.to_string(),
            classification,
            timestamp: Utc::now(),
            severity: 5,
            acknowledged: classification == Classification::Acknowledged,
            suppressed: false,
            recovered: classification == Classification::Recovery,
            maintenance: false,
            custom_fields: BTreeMap::new(),
        }
    }

    fn alerts(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn first_successful_poll_only_seeds_the_baseline() {
        let mut tracker = PollTracker::default();
        assert!(tracker.record_success(alerts(&["a"])).new_alerts.is_empty());
        assert_eq!(
            tracker.record_success(alerts(&["a", "b"])).new_alerts,
            alerts(&["b"])
        );
    }

    #[test]
    fn reset_starts_a_new_baseline() {
        let mut tracker = PollTracker::default();
        tracker.record_success(alerts(&["a"]));
        tracker.reset();
        assert!(
            tracker
                .record_success(alerts(&["a", "b"]))
                .new_alerts
                .is_empty()
        );
    }

    #[test]
    fn connection_changes_are_reported_once() {
        let mut tracker = PollTracker::default();
        assert!(!tracker.record_failure());
        assert!(tracker.record_success(HashSet::new()).restored);
        assert!(!tracker.record_success(HashSet::new()).restored);
        assert!(tracker.record_failure());
        assert!(!tracker.record_failure());
    }

    #[test]
    fn reset_forgets_the_connection_state() {
        let mut tracker = PollTracker::default();
        tracker.record_success(HashSet::new());
        tracker.reset();
        assert!(!tracker.record_failure());
        tracker.reset();
        assert!(!tracker.record_success(HashSet::new()).restored);
    }

    #[test]
    fn filters_by_query_host_and_classification() {
        let critical = message("Disk space low", "db-01", Classification::Critical);
        let warning = message("CPU load", "web-01", Classification::Warning);
        assert!(matches_filters(&critical, "", None, false));
        assert!(matches_filters(&critical, "disk", None, false));
        assert!(matches_filters(&critical, "db-01", None, false));
        assert!(!matches_filters(&critical, "web", None, false));
        assert!(matches_filters(
            &critical,
            "",
            Some(Classification::Critical),
            false
        ));
        assert!(!matches_filters(
            &warning,
            "",
            Some(Classification::Critical),
            false
        ));
        assert!(matches_filters(&critical, "", None, false));
    }

    #[test]
    fn only_open_hides_acknowledged_and_recovered() {
        let acknowledged = message("Ack", "a", Classification::Acknowledged);
        let recovered = message("Recovered", "b", Classification::Recovery);
        let open = message("Open", "c", Classification::Critical);
        assert!(!matches_filters(&acknowledged, "", None, true));
        assert!(!matches_filters(&recovered, "", None, true));
        assert!(matches_filters(&open, "", None, true));
    }
}

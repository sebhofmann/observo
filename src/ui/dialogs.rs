use std::collections::BTreeMap;

use gpui_kit::assets::IconName;
use gpui_kit::base::{Disableable as _, StyledExt as _};
use gpui_kit::component::{
    ActiveTheme, Icon, WindowExt,
    button::{Button, ButtonVariants as _},
    form::{Field, Form},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    notification::{Notification, NotificationType},
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, Hsla, InteractiveElement as _, IntoElement,
    ParentElement, StatefulInteractiveElement as _, Styled, Subscription, Task, Window, div, px,
};

use crate::app::ObservoApp;
use crate::config::{Configuration, FieldKind, FieldSpec};
use crate::i18n::{I18n, Language};
use crate::model::{MAX_ACK_COMMENT_CHARS, Message};
use crate::monitor::zabbix;
use crate::monitor::{self, MonitorError};
use crate::settings::ThemePreference;
use crate::ui::table::classification_color;
use crate::ui::widgets::{badge, icon_tile};

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub struct SettingsDialog {
    app: Entity<ObservoApp>,
}

impl SettingsDialog {
    fn new(app: Entity<ObservoApp>) -> Self {
        Self { app }
    }
}

/// The dialog header is built once when the dialog opens, so render the
/// (language-dependent) settings title from an observed view instead.
struct SettingsTitle {
    app: Entity<ObservoApp>,
    _subscription: Subscription,
}

impl SettingsTitle {
    fn new(app: Entity<ObservoApp>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&app, |_, _, cx| cx.notify());
        Self {
            app,
            _subscription: subscription,
        }
    }
}

impl gpui_kit::Render for SettingsTitle {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(self.app.read(cx).i18n.t("settings.title"))
    }
}

impl gpui_kit::Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let max_height = (window.viewport_size().height - px(150.)).clamp(px(360.), px(720.));
        let (i18n, language, theme, notifications, sound, minimize) = {
            let app = self.app.read(cx);
            (
                app.i18n,
                app.settings.language,
                app.settings.theme,
                app.settings.notifications,
                app.settings.sound,
                app.settings.minimize_on_close,
            )
        };
        let theme_colors = cx.theme().clone();

        let language_buttons =
            div()
                .flex()
                .gap_2()
                .children(Language::ALL.into_iter().map(|option| {
                    let id = format!("language-{}", option.code());
                    let button = Button::new(id)
                        .label(option.native_name())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.app.update(cx, |app, cx| {
                                app.set_language(option, window, cx);
                            });
                        }));
                    if option == language {
                        button.primary()
                    } else {
                        button.outline()
                    }
                }));

        let theme_buttons = div().flex().gap_2().children(
            [
                (ThemePreference::System, "settings.theme.system"),
                (ThemePreference::Light, "settings.theme.light"),
                (ThemePreference::Dark, "settings.theme.dark"),
            ]
            .into_iter()
            .map(|(option, key)| {
                let button = Button::new(format!("theme-{}", option.as_str()))
                    .label(i18n.t(key))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.app.update(cx, |app, cx| {
                            app.set_theme(option, window, cx);
                        });
                        cx.notify();
                    }));
                if option == theme {
                    button.primary()
                } else {
                    button.outline()
                }
            }),
        );

        let form = Form::new()
            .w_full()
            .gap_5()
            .child(
                Field::new()
                    .label(i18n.t("settings.language"))
                    .description(i18n.t("settings.language.help"))
                    .child(language_buttons),
            )
            .child(
                Field::new()
                    .label(i18n.t("settings.theme"))
                    .child(theme_buttons),
            )
            .child(
                Field::new().label(i18n.t("settings.notifications")).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            Switch::new("notifications")
                                .label(i18n.t("settings.notifications.label"))
                                .checked(notifications)
                                .on_change(cx.listener(|this, checked, window, cx| {
                                    this.app.update(cx, |app, cx| {
                                        app.set_notifications(*checked, window, cx);
                                    });
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme_colors.muted_foreground)
                                .child(i18n.t("settings.notifications.help")),
                        )
                        .child(
                            Switch::new("sound")
                                .label(i18n.t("settings.sound.label"))
                                .checked(sound)
                                .on_change(cx.listener(|this, checked, window, cx| {
                                    this.app.update(cx, |app, cx| {
                                        app.set_sound(*checked, window, cx);
                                    });
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme_colors.muted_foreground)
                                .child(i18n.t("settings.sound.help")),
                        ),
                ),
            )
            .child(
                Field::new().label(i18n.t("settings.behavior")).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            Switch::new("minimize-on-close")
                                .label(i18n.t("settings.minimizeOnClose.label"))
                                .checked(minimize)
                                .on_change(cx.listener(|this, checked, window, cx| {
                                    this.app.update(cx, |app, cx| {
                                        app.set_minimize_on_close(*checked, window, cx);
                                    });
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme_colors.muted_foreground)
                                .child(i18n.t("settings.minimizeOnClose.help")),
                        ),
                ),
            );

        div()
            .w_full()
            .h(max_height)
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("settings-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(form),
            )
            .child(div().h(px(1.)).bg(theme_colors.border))
            .child(
                div().flex().justify_end().child(
                    Button::new("close-settings")
                        .outline()
                        .label(i18n.t("common.close"))
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        }),
                ),
            )
    }
}

pub fn open_settings(_app: &mut ObservoApp, window: &mut Window, cx: &mut Context<ObservoApp>) {
    let app_entity = cx.entity();
    let view = cx.new(|_| SettingsDialog::new(app_entity.clone()));
    let title = cx.new(|cx| SettingsTitle::new(app_entity, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title(title.clone()).w(px(560.)).child(view.clone())
    });
}

// ---------------------------------------------------------------------------
// Server configuration
// ---------------------------------------------------------------------------

struct ConfigField {
    spec: FieldSpec,
    input: Entity<InputState>,
}

#[derive(Debug, Clone)]
enum TestState {
    Idle,
    Testing,
    Success(String),
    Failed(MonitorError),
}

pub struct ServerConfigDialog {
    app: Entity<ObservoApp>,
    fields: Vec<ConfigField>,
    errors: BTreeMap<&'static str, &'static str>,
    test: TestState,
    save_error: Option<String>,
    test_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ServerConfigDialog {
    fn new(
        app: Entity<ObservoApp>,
        configuration: Configuration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let provider = monitor::provider(configuration.provider_id());
        let mut subscriptions = Vec::new();
        let fields = provider
            .field_specs()
            .iter()
            .map(|spec| {
                let value = match spec.kind {
                    FieldKind::Number => (configuration.number(spec.key) / 1000).to_string(),
                    _ => configuration.text(spec.key).to_string(),
                };
                let input = cx.new(|cx| {
                    let mut state = InputState::new(window, cx).default_value(value);
                    if spec.kind == FieldKind::Password {
                        state = state.masked(true);
                    }
                    if spec.kind == FieldKind::Number {
                        state = state.validate(|value, _| value.parse::<u32>().is_ok());
                    }
                    state
                });
                subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    |this, _state, event: &InputEvent, _window, cx| {
                        if matches!(event, InputEvent::Change) {
                            this.test = TestState::Idle;
                            this.errors.clear();
                            this.save_error = None;
                            cx.notify();
                        }
                    },
                ));
                ConfigField { spec: *spec, input }
            })
            .collect();
        Self {
            app,
            fields,
            errors: BTreeMap::new(),
            test: TestState::Idle,
            save_error: None,
            test_task: None,
            _subscriptions: subscriptions,
        }
    }

    fn collect(&self, cx: &App) -> Configuration {
        self.collect_raw(cx).normalised()
    }

    /// Reads the input fields without normalising, so out of range values
    /// reach validation instead of being clamped silently.
    fn collect_raw(&self, cx: &App) -> Configuration {
        let app = self.app.read(cx);
        let mut configuration = app.settings.server.clone();
        for field in &self.fields {
            let value = field_text(field.spec.kind, &field.input.read(cx).value()).to_string();
            if field.spec.kind == FieldKind::Number {
                let seconds = value.parse::<i64>().unwrap_or(-1);
                configuration.set_number(field.spec.key, seconds.max(0) * 1000);
            } else {
                configuration.set_text(field.spec.key, value);
            }
        }
        configuration
    }

    fn validate(&mut self, cx: &App) -> bool {
        let configuration = self.collect_raw(cx);
        let provider = monitor::provider(&self.app.read(cx).settings.provider_id);
        self.errors = provider.validate(&configuration);
        self.errors.is_empty()
    }

    fn start_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.validate(cx) {
            return;
        }
        let configuration = self.collect(cx);
        self.test = TestState::Testing;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = zabbix::test_configuration(&configuration);
            let _ = tx.send_blocking(result);
        });
        self.test_task = Some(cx.spawn_in(window, async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                this.update_in(cx, |this, _, cx| {
                    this.test = match result {
                        Ok(version) => TestState::Success(version),
                        Err(error) => TestState::Failed(error),
                    };
                    cx.notify();
                })
                .ok();
            }
        }));
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.validate(cx) {
            cx.notify();
            return;
        }
        let configuration = self.collect(cx);
        let result = self.app.update(cx, |app, cx| {
            app.apply_server_configuration(configuration, window, cx)
        });
        match result {
            Ok(()) => {
                self.save_error = None;
                window.close_dialog(cx);
            }
            Err(error) => {
                self.save_error = Some(error);
                cx.notify();
            }
        }
    }

    fn render_field(
        &self,
        field: &ConfigField,
        i18n: I18n,
        theme: &gpui_kit::component::Theme,
    ) -> AnyElement {
        let spec = field.spec;
        let label = if spec.key == "zabbix.pollIntervalMillis" {
            i18n.t("settings.pollInterval")
        } else {
            i18n.t(spec.label_key)
        };
        let help = spec.help_key.map(|key| i18n.t(key)).unwrap_or_default();
        let error = self.errors.get(spec.key).copied();

        let mut input = Input::new(&field.input).w_full();
        if spec.kind == FieldKind::Password {
            input = input.mask_toggle();
        }
        let mut body = div().flex().flex_col().gap_1().child(input);
        if let Some(error_key) = error {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(i18n.t(error_key)),
            );
        } else if !help.is_empty() {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(help),
            );
        }
        Field::new()
            .label(label)
            .required(spec.required)
            .child(body)
            .into_any_element()
    }
}

impl gpui_kit::Render for ServerConfigDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let max_height = (window.viewport_size().height - px(150.)).clamp(px(360.), px(720.));
        let (i18n, provider_id) = {
            let app = self.app.read(cx);
            (app.i18n, app.settings.provider_id.clone())
        };
        let theme = cx.theme().clone();
        let provider = monitor::provider(&provider_id);

        let fields = self
            .fields
            .iter()
            .map(|field| self.render_field(field, i18n, &theme))
            .collect::<Vec<_>>();

        let test_status =
            match &self.test {
                TestState::Idle => None,
                TestState::Testing => Some(
                    div()
                        .text_xs()
                        .text_color(theme.info)
                        .child(i18n.t("serverConfig.testing"))
                        .into_any_element(),
                ),
                TestState::Success(version) => Some(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::CircleCheck)
                                .size_4()
                                .text_color(theme.success),
                        )
                        .child(div().text_xs().text_color(theme.success).child(
                            if version.is_empty() {
                                i18n.t("serverConfig.testSuccessNoVersion").to_string()
                            } else {
                                i18n.tf("serverConfig.testSuccess", &[("version", version)])
                            },
                        ))
                        .into_any_element(),
                ),
                TestState::Failed(error) => Some(
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .size_4()
                                .text_color(theme.danger),
                        )
                        .child(div().text_xs().text_color(theme.danger).child(
                            i18n.tf("serverConfig.testFailed", &[("error", &error.message)]),
                        ))
                        .into_any_element(),
                ),
            };

        let mut content = div().flex().flex_col().gap_5();
        content = content.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .rounded_lg()
                .bg(theme.muted.opacity(0.35))
                .p_3()
                .child(icon_tile(
                    IconName::Server,
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
                                .text_sm()
                                .font_medium()
                                .child(i18n.t(provider.display_key())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(i18n.t("serverConfig.subtitle")),
                        ),
                )
                .child(div().flex_1()),
        );
        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(section_label(
                    i18n.t("serverConfig.connection"),
                    theme.muted_foreground,
                ))
                .children(fields.into_iter().take(4))
                .child(section_label(
                    i18n.t("serverConfig.polling"),
                    theme.muted_foreground,
                ))
                .children(
                    self.fields
                        .iter()
                        .skip(4)
                        .map(|field| self.render_field(field, i18n, &theme)),
                ),
        );
        let status = {
            let mut status = div().flex().flex_col().gap_1();
            let mut has_status = false;
            if let Some(test_status) = test_status {
                status = status.child(test_status);
                has_status = true;
            }
            if let Some(error) = &self.save_error {
                status = status.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(error.clone()),
                );
                has_status = true;
            }
            has_status.then(|| status.into_any_element())
        };

        let testing = matches!(self.test, TestState::Testing);
        let mut footer = div().flex().items_center().gap_2();
        footer = footer.child(
            Button::new("test-connection")
                .outline()
                .icon(IconName::Activity)
                .label(i18n.t("serverConfig.test"))
                .disabled(testing)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.start_test(window, cx);
                })),
        );
        footer = footer.child(div().flex_1());
        footer = footer.child(
            Button::new("cancel-server")
                .outline()
                .label(i18n.t("common.cancel"))
                .on_click(|_, window, cx| {
                    window.close_dialog(cx);
                }),
        );
        footer = footer.child(
            Button::new("save-server")
                .primary()
                .label(i18n.t("serverConfig.save"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.save(window, cx);
                })),
        );

        div()
            .w_full()
            .h(max_height)
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("server-config-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(div().h(px(1.)).bg(theme.border))
            .when_some(status, |element, status| element.child(status))
            .child(footer)
    }
}

fn section_label(label: &'static str, color: Hsla) -> AnyElement {
    div()
        .text_xs()
        .font_semibold()
        .text_color(color)
        .child(label)
        .into_any_element()
}

pub fn open_server_config(app: &mut ObservoApp, window: &mut Window, cx: &mut Context<ObservoApp>) {
    let i18n = app.i18n;
    let configuration = app.settings.server.clone();
    let app_entity = cx.entity();
    let view = cx.new(|cx| ServerConfigDialog::new(app_entity, configuration, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(i18n.t("serverConfig.title"))
            .w(px(640.))
            .child(view.clone())
    });
}

// ---------------------------------------------------------------------------
// Acknowledge
// ---------------------------------------------------------------------------

pub struct AcknowledgeDialog {
    app: Entity<ObservoApp>,
    message: Message,
    i18n: I18n,
    comment: Entity<TextareaState>,
    _subscription: Subscription,
}

impl AcknowledgeDialog {
    fn new(
        app: Entity<ObservoApp>,
        message: Message,
        i18n: I18n,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let comment = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder(i18n.t("acknowledge.dialog.placeholder"))
        });
        let subscription = cx.subscribe_in(
            &comment,
            window,
            |_this, _state, _event: &InputEvent, _window, cx| {
                cx.notify();
            },
        );
        Self {
            app,
            message,
            i18n,
            comment,
            _subscription: subscription,
        }
    }
}

impl gpui_kit::Render for AcknowledgeDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = self.i18n;
        let theme = cx.theme().clone();
        let color = classification_color(self.message.classification, cx);
        let in_flight = self.app.read(cx).ack_in_flight;
        let can_acknowledge = self.message.can_acknowledge();
        let comment_length = self.comment.read(cx).value().chars().count();
        let comment_too_long = comment_length > MAX_ACK_COMMENT_CHARS;

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().child(badge(
                        i18n.classification(self.message.classification),
                        color,
                    )))
                    .child(div().text_sm().child(i18n.tf(
                        "acknowledge.dialog.description",
                        &[
                            ("title", &self.message.title),
                            ("host", &self.message.hostname),
                        ],
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{} · {}",
                                i18n.t("main.eventId"),
                                self.message
                                    .event_id
                                    .clone()
                                    .unwrap_or_else(|| i18n.t("common.none").to_string())
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(i18n.t("acknowledge.dialog.comment")),
                    )
                    .child(Textarea::new(&self.comment).w_full())
                    .child(
                        div().flex().justify_end().child(
                            div()
                                .text_xs()
                                .text_color(if comment_too_long {
                                    theme.danger
                                } else {
                                    theme.muted_foreground
                                })
                                .child(i18n.tf(
                                    "acknowledge.dialog.commentCount",
                                    &[
                                        ("count", &comment_length.to_string()),
                                        ("max", &MAX_ACK_COMMENT_CHARS.to_string()),
                                    ],
                                )),
                        ),
                    ),
            )
            .when(!can_acknowledge, |element| {
                element.child(
                    div()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(i18n.t("acknowledge.notAcknowledgeable.text")),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1())
                    .child(
                        Button::new("cancel-ack")
                            .outline()
                            .label(i18n.t("common.cancel"))
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Button::new("confirm-ack")
                            .primary()
                            .icon(IconName::Check)
                            .label(if in_flight {
                                i18n.t("main.ackInFlight")
                            } else {
                                i18n.t("acknowledge.dialog.confirm")
                            })
                            .disabled(in_flight || !can_acknowledge || comment_too_long)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let comment = this.comment.read(cx).value().to_string();
                                let message = this.message.clone();
                                this.app.update(cx, |app, cx| {
                                    app.start_acknowledge(message, comment, window, cx);
                                });
                                window.close_dialog(cx);
                            })),
                    ),
            )
    }
}

pub fn open_acknowledge(
    app: &mut ObservoApp,
    message: Message,
    window: &mut Window,
    cx: &mut Context<ObservoApp>,
) {
    if !message.can_acknowledge() {
        window.push_notification(
            Notification::new()
                .title(app.i18n.t("acknowledge.notAcknowledgeable.title"))
                .message(app.i18n.t("acknowledge.notAcknowledgeable.text"))
                .with_type(NotificationType::Warning),
            cx,
        );
        return;
    }
    let i18n = app.i18n;
    let app_entity = cx.entity();
    let view = cx.new(|cx| AcknowledgeDialog::new(app_entity, message, i18n, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(i18n.t("acknowledge.dialog.title"))
            .w(px(500.))
            .child(view.clone())
    });
}

// ---------------------------------------------------------------------------
// About
// ---------------------------------------------------------------------------

pub struct AboutDialog {
    i18n: I18n,
}

impl gpui_kit::Render for AboutDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = self.i18n;
        let theme = cx.theme().clone();
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(icon_tile(
                IconName::Activity,
                theme.primary,
                theme.primary.opacity(0.12),
            ))
            .child(div().text_sm().child(i18n.t("about.description")))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(i18n.tf("about.version", &[("version", env!("CARGO_PKG_VERSION"))])),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(i18n.t("about.license")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(i18n.t("about.builtWith")),
            )
    }
}

pub fn open_about(app: &mut ObservoApp, window: &mut Window, cx: &mut Context<ObservoApp>) {
    let i18n = app.i18n;
    let view = cx.new(|_| AboutDialog { i18n });
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(i18n.t("about.title"))
            .w(px(420.))
            .child(view.clone())
    });
}

/// Passwords may legitimately contain surrounding whitespace.
fn field_text(kind: FieldKind, raw: &str) -> &str {
    if kind == FieldKind::Password {
        raw
    } else {
        raw.trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_keep_surrounding_whitespace() {
        assert_eq!(field_text(FieldKind::Password, " secret "), " secret ");
    }

    #[test]
    fn other_fields_are_trimmed() {
        assert_eq!(field_text(FieldKind::Text, " admin "), "admin");
        assert_eq!(field_text(FieldKind::Number, " 30 "), "30");
    }
}

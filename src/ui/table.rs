use chrono::Local;
use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::{
    App, Context, Hsla, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    Stateful, Styled, Window, div,
};

use crate::i18n::{I18n, Language};
use crate::model::{Classification, Message};
use crate::monitor::zabbix::sort_messages;

pub struct MessagesTable {
    messages: Vec<Message>,
    language: Language,
    sort: Option<(usize, ColumnSort)>,
    compact: bool,
}

impl MessagesTable {
    pub fn new(language: Language) -> Self {
        Self {
            messages: Vec::new(),
            language,
            sort: None,
            compact: false,
        }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    /// Narrower columns while the detail panel is open so that all columns
    /// remain visible on typical laptop screens.
    pub fn set_compact(&mut self, compact: bool) -> bool {
        if self.compact == compact {
            return false;
        }
        self.compact = compact;
        true
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        log::debug!("table delegate received {} messages", messages.len());
        self.messages = messages;
        self.apply_sort();
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn message(&self, index: usize) -> Option<&Message> {
        self.messages.get(index)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.messages.iter().position(|message| message.id == id)
    }

    fn i18n(&self) -> I18n {
        I18n::new(self.language)
    }

    fn apply_sort(&mut self) {
        match self.sort {
            None | Some((_, ColumnSort::Default)) => sort_messages(&mut self.messages),
            Some((column, direction)) => {
                let ascending = direction == ColumnSort::Ascending;
                self.messages.sort_by(|a, b| {
                    let ordering = match column {
                        0 => a
                            .severity
                            .cmp(&b.severity)
                            .then_with(|| a.classification.rank().cmp(&b.classification.rank())),
                        1 => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                        2 => a.message.to_lowercase().cmp(&b.message.to_lowercase()),
                        3 => a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()),
                        4 => a.timestamp.cmp(&b.timestamp),
                        5 => a
                            .acknowledged
                            .cmp(&b.acknowledged)
                            .then_with(|| a.recovered.cmp(&b.recovered)),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if ascending {
                        ordering
                    } else {
                        ordering.reverse()
                    }
                });
            }
        }
    }

    fn column(&self, index: usize, _cx: &App) -> Column {
        let i18n = self.i18n();
        match index {
            0 => Column::new("type", i18n.t("table.classification"))
                .width(120.)
                .sortable(),
            1 => Column::new("title", i18n.t("table.title"))
                .width(if self.compact { 220. } else { 250. })
                .min_width(170.)
                .sortable(),
            2 => Column::new("message", i18n.t("table.message"))
                .width(if self.compact { 240. } else { 440. })
                .min_width(200.)
                .sortable(),
            3 => Column::new("host", i18n.t("table.host"))
                .width(if self.compact { 100. } else { 120. })
                .sortable(),
            4 => Column::new("time", i18n.t("table.timestamp"))
                .width(if self.compact { 95. } else { 115. })
                .sortable(),
            5 => Column::new("status", i18n.t("table.status"))
                .width(160.)
                .sortable(),
            _ => Column::new("unknown", "?").width(60.),
        }
    }
}

pub fn classification_color(classification: Classification, cx: &App) -> Hsla {
    let theme = cx.theme();
    match classification {
        Classification::Critical => theme.danger,
        Classification::Warning => theme.warning,
        Classification::Info => theme.info,
        Classification::Recovery => theme.success,
        Classification::Acknowledged => theme.muted_foreground,
        Classification::Unknown => theme.muted_foreground,
    }
}

impl TableDelegate for MessagesTable {
    fn columns_count(&self, _: &App) -> usize {
        6
    }

    fn rows_count(&self, _: &App) -> usize {
        self.messages.len()
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        self.column(col_ix, cx)
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        self.sort = Some((col_ix, sort));
        self.apply_sort();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(message) = self.messages.get(row_ix) else {
            return div().into_any_element();
        };
        let i18n = self.i18n();
        let theme_color = classification_color(message.classification, cx);
        match col_ix {
            0 => {
                let label = i18n.classification(message.classification);
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .h(gpui_kit::px(22.))
                            .px_2()
                            .rounded_full()
                            .border_1()
                            .border_color(theme_color.opacity(0.35))
                            .bg(theme_color.opacity(0.12))
                            .text_color(theme_color)
                            .text_xs()
                            .font_medium()
                            .flex()
                            .items_center()
                            .child(label),
                    )
                    .into_any_element()
            }
            1 => div()
                .w_full()
                .truncate()
                .font_medium()
                .text_color(cx.theme().foreground)
                .child(message.title.clone())
                .into_any_element(),
            2 => div()
                .w_full()
                .truncate()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(message.message.clone())
                .into_any_element(),
            3 => div()
                .w_full()
                .truncate()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(message.hostname.clone())
                .into_any_element(),
            4 => {
                let local = message.timestamp.with_timezone(&Local);
                let text = if local.date_naive() == Local::now().date_naive() {
                    local.format("%H:%M:%S").to_string()
                } else {
                    local.format("%d.%m. %H:%M").to_string()
                };
                div()
                    .w_full()
                    .truncate()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(text)
                    .into_any_element()
            }
            5 => {
                let key = status_key(message);
                let color = if message.acknowledged {
                    cx.theme().muted_foreground
                } else if message.recovered {
                    cx.theme().success
                } else if message.maintenance {
                    cx.theme().info
                } else {
                    cx.theme().warning
                };
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(gpui_kit::px(6.)).rounded_full().bg(color))
                    .child(div().text_sm().text_color(color).child(i18n.t(key)))
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> Stateful<gpui_kit::Div> {
        let color = self
            .messages
            .get(row_ix)
            .map(|message| classification_color(message.classification, cx))
            .unwrap_or_else(|| cx.theme().border);
        div()
            .id(("observo-row", row_ix))
            .border_l_2()
            .border_color(color)
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(IconName::Inbox).size_6())
            .child(self.i18n().t("main.noProblems.title"))
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let message = self.messages.get(row_ix)?;
        let i18n = self.i18n();
        Some(match col_ix {
            0 => i18n.classification(message.classification).into(),
            1 => message.title.clone().into(),
            2 => message.message.clone().into(),
            3 => message.hostname.clone().into(),
            4 => message.timestamp.to_string().into(),
            5 => i18n.t(status_key(message)).into(),
            _ => "".into(),
        })
    }

    fn context_menu(
        &mut self,
        _row_ix: usize,
        menu: gpui_kit::component::menu::PopupMenu,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> gpui_kit::component::menu::PopupMenu {
        // Row actions are exposed through the detail panel and double-click.
        // Keeping the default menu avoids hidden, unreachable commands.
        menu
    }

    fn loading(&self, _: &App) -> bool {
        false
    }
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

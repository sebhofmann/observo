use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{ActiveTheme, Icon};
use gpui_kit::{AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};

/// A small rounded status badge used for classifications and status labels.
pub fn badge(label: impl Into<SharedString>, color: Hsla) -> AnyElement {
    div()
        .h(px(22.))
        .px_2()
        .rounded_full()
        .border_1()
        .border_color(color.opacity(0.35))
        .bg(color.opacity(0.12))
        .text_color(color)
        .text_xs()
        .font_medium()
        .flex()
        .items_center()
        .gap_1()
        .child(label.into())
        .into_any_element()
}

/// A coloured dot followed by text, used in the header and footer.
pub fn status_pill(label: impl Into<SharedString>, color: Hsla, background: Hsla) -> AnyElement {
    div()
        .h(px(28.))
        .px_3()
        .rounded_full()
        .bg(background)
        .border_1()
        .border_color(color.opacity(0.25))
        .flex()
        .items_center()
        .gap_2()
        .child(div().size(px(7.)).rounded_full().bg(color).flex_shrink_0())
        .child(
            div()
                .text_xs()
                .font_medium()
                .text_color(color)
                .child(label.into()),
        )
        .into_any_element()
}

/// A monochrome icon inside a subtle rounded square.
pub fn icon_tile(icon: IconName, color: Hsla, background: Hsla) -> AnyElement {
    div()
        .size(px(36.))
        .rounded_lg()
        .bg(background)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).size_5().text_color(color))
        .into_any_element()
}

/// A compact metric used in the footer.
pub fn metric(
    icon: IconName,
    value: impl Into<SharedString>,
    label: impl Into<SharedString>,
    color: Hsla,
    muted: Hsla,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(Icon::new(icon).size_3().text_color(color))
        .child(
            div()
                .text_xs()
                .font_medium()
                .text_color(color)
                .child(value.into()),
        )
        .child(div().text_xs().text_color(muted).child(label.into()))
        .into_any_element()
}

/// A vertical separator with the theme's border colour.
pub fn separator(cx: &gpui_kit::App) -> AnyElement {
    div()
        .h(px(18.))
        .w(px(1.))
        .bg(cx.theme().border)
        .flex_shrink_0()
        .into_any_element()
}

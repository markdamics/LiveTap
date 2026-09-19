//! Collapsible, syntax-highlighted JSON tree — a transcription of
//! `renderJsonNode` from the Ginei design's `.dc.html` reference script.

use std::collections::HashSet;

use iced::widget::{button, container, row, text, Space};
use iced::Element;
use serde_json::Value;

use crate::theme::{self, json};
use crate::Message;

const INDENT_PX: f32 = 16.0;

/// Renders `value` as a flattened list of indented rows, starting at the
/// synthetic root key `"body"` (matching the design's `root` node).
///
/// `expanded` holds manually toggled node paths. While `search` is
/// non-empty, expansion is instead driven entirely by which paths contain a
/// match, so a search automatically reveals its hits regardless of manual
/// collapse state (matched rows also get the design's `accent-quiet`
/// row-background tint).
pub fn view(value: &Value, expanded: &HashSet<String>, search: &str) -> Element<'static, Message> {
    let mut rows = Vec::new();
    let search_lower = search.to_lowercase();
    let match_paths = if search_lower.is_empty() {
        None
    } else {
        let mut set = HashSet::new();
        collect_match_ancestors(value, "root", &search_lower, &mut set);
        Some(set)
    };

    render_node(
        value,
        "root",
        "body".to_string(),
        0,
        expanded,
        &match_paths,
        &search_lower,
        &mut rows,
    );

    iced::widget::column(rows).into()
}

fn collect_match_ancestors(
    value: &Value,
    path: &str,
    search_lower: &str,
    out: &mut HashSet<String>,
) -> bool {
    let self_matches = match value {
        Value::String(s) => s.to_lowercase().contains(search_lower),
        Value::Number(n) => n.to_string().contains(search_lower),
        Value::Bool(b) => b.to_string().contains(search_lower),
        Value::Null => "null".contains(search_lower),
        _ => false,
    };

    let mut any_child_matches = false;
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                let key_matches = key.to_lowercase().contains(search_lower);
                let child_matches =
                    collect_match_ancestors(child, &child_path, search_lower, out);
                any_child_matches |= key_matches || child_matches;
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                any_child_matches |=
                    collect_match_ancestors(child, &child_path, search_lower, out);
            }
        }
        _ => {}
    }

    if self_matches || any_child_matches {
        out.insert(path.to_string());
    }
    self_matches || any_child_matches
}

#[allow(clippy::too_many_arguments)]
fn render_node(
    value: &Value,
    path: &str,
    key_label: String,
    depth: usize,
    expanded: &HashSet<String>,
    match_paths: &Option<HashSet<String>>,
    search_lower: &str,
    rows: &mut Vec<Element<'static, Message>>,
) {
    let is_match = !search_lower.is_empty()
        && match_paths
            .as_ref()
            .map(|set| set.contains(path))
            .unwrap_or(false);

    let is_object = matches!(value, Value::Object(_) | Value::Array(_));

    if !is_object {
        rows.push(json_row(
            depth,
            None,
            key_label,
            leaf_value(value),
            is_match,
        ));
        return;
    }

    let (open, close, is_empty) = match value {
        Value::Object(map) => ("{", "}", map.is_empty()),
        Value::Array(items) => ("[", "]", items.is_empty()),
        _ => unreachable!(),
    };

    if is_empty {
        rows.push(json_row(
            depth,
            None,
            key_label,
            text(format!("{open}{close}"))
                .color(json::bracket())
                .font(theme::FONT_REGULAR)
                .into(),
            is_match,
        ));
        return;
    }

    let is_open = is_expanded(path, expanded, match_paths, search_lower);
    let bracket_preview = if is_open {
        open.to_string()
    } else {
        format!("{open}…{close}")
    };

    rows.push(json_row(
        depth,
        Some(toggle(path, is_open)),
        key_label,
        text(bracket_preview)
            .color(json::bracket())
            .font(theme::FONT_REGULAR)
            .into(),
        is_match,
    ));

    if is_open {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let child_path = format!("{path}.{key}");
                    render_node(
                        child,
                        &child_path,
                        key.clone(),
                        depth + 1,
                        expanded,
                        match_paths,
                        search_lower,
                        rows,
                    );
                }
            }
            Value::Array(items) => {
                for (i, child) in items.iter().enumerate() {
                    let child_path = format!("{path}[{i}]");
                    render_node(
                        child,
                        &child_path,
                        format!("[{i}]"),
                        depth + 1,
                        expanded,
                        match_paths,
                        search_lower,
                        rows,
                    );
                }
            }
            _ => unreachable!(),
        }

        rows.push(
            row![
                Space::new().width(depth as f32 * INDENT_PX + 10.0),
                text(close).color(json::bracket()).font(theme::FONT_REGULAR),
            ]
            .into(),
        );
    }
}

fn is_expanded(
    path: &str,
    expanded: &HashSet<String>,
    match_paths: &Option<HashSet<String>>,
    search_lower: &str,
) -> bool {
    if path == "root" {
        return true;
    }
    if search_lower.is_empty() {
        expanded.contains(path)
    } else {
        match_paths
            .as_ref()
            .map(|set| set.contains(path))
            .unwrap_or(false)
    }
}

fn toggle(path: &str, expanded: bool) -> Element<'static, Message> {
    button(
        text(if expanded { "▾" } else { "▸" })
            .size(12)
            .color(json::caret()),
    )
    .padding(0)
    .style(button::text)
    .on_press(Message::ToggleJsonNode(path.to_string()))
    .into()
}

fn json_row(
    depth: usize,
    toggle: Option<Element<'static, Message>>,
    key_label: String,
    value_element: Element<'static, Message>,
    is_match: bool,
) -> Element<'static, Message> {
    let mut items: Vec<Element<'static, Message>> = vec![Space::new()
        .width(depth as f32 * INDENT_PX + 10.0)
        .into()];

    if let Some(toggle) = toggle {
        items.push(toggle);
    }

    items.push(
        text(format!("{key_label}:"))
            .color(json::key())
            .font(theme::FONT_REGULAR)
            .size(theme::TEXT_BODY_SM)
            .into(),
    );
    items.push(value_element);

    container(row(items).spacing(theme::SPACE_2))
        .padding([2, 4])
        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
            background: is_match.then(|| json::match_background().into()),
            border: iced::Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn leaf_value(value: &Value) -> Element<'static, Message> {
    match value {
        Value::String(s) => text(format!("\"{s}\""))
            .color(json::string())
            .font(theme::FONT_REGULAR)
            .size(theme::TEXT_BODY_SM)
            .into(),
        Value::Number(n) => text(n.to_string())
            .color(json::number())
            .font(theme::FONT_REGULAR)
            .size(theme::TEXT_BODY_SM)
            .into(),
        Value::Bool(b) => text(b.to_string())
            .color(json::boolean())
            .font(theme::FONT_REGULAR)
            .size(theme::TEXT_BODY_SM)
            .into(),
        Value::Null => text("null")
            .color(json::null())
            .font(theme::FONT_REGULAR)
            .size(theme::TEXT_BODY_SM)
            .into(),
        _ => text("").into(),
    }
}

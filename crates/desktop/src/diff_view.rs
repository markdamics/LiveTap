//! Structural diff between two captured bodies — a transcription of
//! `flatten` + `computeDiffRows` from the Ginei design's `.dc.html`
//! reference script: each JSON body is flattened to a `dot.path -> value`
//! map (arrays are kept as a single stringified leaf, not recursed into),
//! then the two maps are compared key by key.

use std::collections::{BTreeMap, BTreeSet};

use iced::widget::{column, container, row, text};
use iced::{Element, Length};
use serde_json::Value;

use crate::theme;
use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowStatus {
    Same,
    Changed,
    Added,
    Removed,
}

impl RowStatus {
    fn color(self) -> iced::Color {
        match self {
            RowStatus::Same => theme::text_faint(),
            RowStatus::Changed => theme::status_warning(),
            RowStatus::Added => theme::status_success(),
            RowStatus::Removed => theme::status_danger(),
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            RowStatus::Same => "=",
            RowStatus::Changed => "~",
            RowStatus::Added => "+",
            RowStatus::Removed => "-",
        }
    }
}

fn flatten(value: &Value, prefix: &str, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(child, &path, out);
            }
        }
        Value::Object(_) => {
            out.insert(prefix.to_string(), "{}".to_string());
        }
        Value::Array(items) if items.is_empty() => {
            out.insert(prefix.to_string(), "[]".to_string());
        }
        Value::Array(_) => {
            out.insert(
                prefix.to_string(),
                serde_json::to_string(value).unwrap_or_default(),
            );
        }
        Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        other => {
            out.insert(prefix.to_string(), other.to_string());
        }
    }
}

pub fn view(body_a: &[u8], body_b: &[u8]) -> Element<'static, Message> {
    let value_a: Value = serde_json::from_slice(body_a).unwrap_or(Value::Null);
    let value_b: Value = serde_json::from_slice(body_b).unwrap_or(Value::Null);

    let mut flat_a = BTreeMap::new();
    let mut flat_b = BTreeMap::new();
    flatten(&value_a, "", &mut flat_a);
    flatten(&value_b, "", &mut flat_b);

    let paths: BTreeSet<&String> = flat_a.keys().chain(flat_b.keys()).collect();

    let rows: Vec<Element<'static, Message>> = paths
        .into_iter()
        .map(|path| {
            let in_a = flat_a.get(path);
            let in_b = flat_b.get(path);
            let status = match (in_a, in_b) {
                (Some(_), None) => RowStatus::Removed,
                (None, Some(_)) => RowStatus::Added,
                (Some(a), Some(b)) if a != b => RowStatus::Changed,
                _ => RowStatus::Same,
            };
            diff_row(path, in_a, in_b, status)
        })
        .collect();

    column(rows).into()
}

fn diff_row(
    path: &str,
    a: Option<&String>,
    b: Option<&String>,
    status: RowStatus,
) -> Element<'static, Message> {
    let header = row![
        text(path.to_string())
            .size(theme::TEXT_MICRO)
            .font(theme::FONT_MEDIUM)
            .color(theme::text_faint()),
        iced::widget::Space::new().width(Length::Fill),
        text(status.symbol()).size(theme::TEXT_MICRO).color(status.color()),
    ];

    let base_color = if status == RowStatus::Same {
        theme::text_faint()
    } else {
        theme::text_body()
    };
    let a_color = if status == RowStatus::Removed {
        theme::status_danger()
    } else {
        base_color
    };
    let b_color = if status == RowStatus::Added {
        theme::status_success()
    } else {
        base_color
    };

    let content = column![
        header,
        text(format!("A · {}", a.map(String::as_str).unwrap_or("—")))
            .size(theme::TEXT_BODY_SM)
            .color(a_color),
        text(format!("B · {}", b.map(String::as_str).unwrap_or("—")))
            .size(theme::TEXT_BODY_SM)
            .color(b_color),
    ]
    .spacing(4);

    let bar = container(iced::widget::Space::new().width(3.0).height(Length::Fill)).style(
        move |_theme: &iced::Theme| iced::widget::container::Style {
            background: Some(status.color().into()),
            ..Default::default()
        },
    );

    row![bar, container(content).padding([8, 16]).width(Length::Fill)]
        .align_y(iced::Alignment::Start)
        .into()
}

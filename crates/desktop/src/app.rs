use std::collections::HashSet;
use std::time::Duration;

use iced::widget::{button, column, container, row, scrollable, text, text_input, pick_list, Space};
use iced::{Center, Element, Fill, Subscription, Task, Theme};

use livetap_core::relay_client::{self, RelayEvent, RelaySession};
use livetap_core::LiveTapCore;
use livetap_shared::{CapturedRequest, HeaderPair};

use crate::{diff_view, json_view, theme};

const DEFAULT_RELAY_URL: &str = "http://127.0.0.1:8787";
const HOOK_URL_DISPLAY: &str = "hook.dev/x89f-22a";
const COPY_FEEDBACK_DURATION: Duration = Duration::from_millis(1600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Terminal,
    Workbench,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Headers,
    Body,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionStatus {
    Idle,
    Connecting,
    Connected,
    Disconnected,
}

pub struct State {
    core: Option<LiveTapCore>,
    status_message: Option<String>,
    screen: Screen,
    relay_base_url: String,
    connection: ConnectionStatus,
    session: Option<RelaySession>,
    /// Newest capture first.
    requests: Vec<CapturedRequest>,
    selected_id: Option<String>,
    tab: InspectorTab,
    expanded_json: HashSet<String>,
    search: String,
    diff_a: Option<String>,
    diff_b: Option<String>,
    copied: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    GoTerminal,
    GoWorkbench,
    RelayUrlChanged(String),
    Connect,
    SessionCreated(Result<RelaySession, String>),
    Relay(RelayEvent),
    SelectRequest(String),
    TabSelected(InspectorTab),
    ToggleJsonNode(String),
    SearchChanged(String),
    ClearFeed,
    CopyHookUrl,
    CopyFeedbackExpired,
    DiffASelected(String),
    DiffBSelected(String),
}

impl State {
    pub fn new() -> (Self, Task<Message>) {
        let (core, status_message) = match app_data_dir() {
            Some(dir) => match LiveTapCore::open(dir.display().to_string()) {
                Ok(core) => (Some(core), None),
                Err(err) => (None, Some(format!("local storage unavailable: {err}"))),
            },
            None => (
                None,
                Some("local storage unavailable: no app data directory".to_string()),
            ),
        };

        (
            Self {
                core,
                status_message,
                screen: Screen::Terminal,
                relay_base_url: DEFAULT_RELAY_URL.to_string(),
                connection: ConnectionStatus::Idle,
                session: None,
                requests: Vec::new(),
                selected_id: None,
                tab: InspectorTab::Headers,
                expanded_json: HashSet::new(),
                search: String::new(),
                diff_a: None,
                diff_b: None,
                copied: false,
            },
            Task::none(),
        )
    }

    pub fn theme(&self) -> Theme {
        theme::theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match &self.session {
            Some(session) => {
                Subscription::run_with(session.ws_url.clone(), relay_client::connect)
                    .map(Message::Relay)
            }
            None => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::GoTerminal => {
                self.screen = Screen::Terminal;
                Task::none()
            }
            Message::GoWorkbench => {
                self.screen = Screen::Workbench;
                Task::none()
            }
            Message::RelayUrlChanged(url) => {
                self.relay_base_url = url;
                Task::none()
            }
            Message::Connect => {
                self.connection = ConnectionStatus::Connecting;
                self.status_message = None;
                self.session = None;
                self.requests.clear();
                self.selected_id = None;
                self.diff_a = None;
                self.diff_b = None;
                let base_url = self.relay_base_url.clone();
                Task::perform(
                    async move {
                        relay_client::create_session(&base_url)
                            .await
                            .map_err(|err| err.to_string())
                    },
                    Message::SessionCreated,
                )
            }
            Message::SessionCreated(Ok(session)) => {
                self.session = Some(session);
                Task::none()
            }
            Message::SessionCreated(Err(err)) => {
                self.connection = ConnectionStatus::Idle;
                self.status_message = Some(format!("couldn't reach relay: {err}"));
                Task::none()
            }
            Message::Relay(RelayEvent::Connected) => {
                self.connection = ConnectionStatus::Connected;
                Task::none()
            }
            Message::Relay(RelayEvent::Disconnected) => {
                self.connection = ConnectionStatus::Disconnected;
                Task::none()
            }
            Message::Relay(RelayEvent::Captured(captured)) => {
                let captured = *captured;
                if let Some(core) = &self.core {
                    if let Err(err) = core.insert_captured_request(captured.clone()) {
                        self.status_message = Some(format!("failed to persist capture: {err}"));
                    }
                }
                if self.selected_id.is_none() {
                    self.selected_id = Some(captured.id.clone());
                }
                if self.diff_a.is_none() {
                    self.diff_a = Some(captured.id.clone());
                } else if self.diff_b.is_none() && self.diff_a.as_deref() != Some(&captured.id) {
                    self.diff_b = Some(captured.id.clone());
                }
                self.requests.insert(0, captured);
                Task::none()
            }
            Message::SelectRequest(id) => {
                self.selected_id = Some(id);
                self.tab = InspectorTab::Headers;
                self.search.clear();
                Task::none()
            }
            Message::TabSelected(tab) => {
                self.tab = tab;
                Task::none()
            }
            Message::ToggleJsonNode(path) => {
                if !self.expanded_json.remove(&path) {
                    self.expanded_json.insert(path);
                }
                Task::none()
            }
            Message::SearchChanged(search) => {
                self.search = search;
                Task::none()
            }
            Message::ClearFeed => {
                self.requests.clear();
                self.selected_id = None;
                self.diff_a = None;
                self.diff_b = None;
                Task::none()
            }
            Message::CopyHookUrl => {
                self.copied = true;
                Task::batch([
                    iced::clipboard::write(format!("https://{HOOK_URL_DISPLAY}")),
                    Task::perform(tokio::time::sleep(COPY_FEEDBACK_DURATION), |_| {
                        Message::CopyFeedbackExpired
                    }),
                ])
            }
            Message::CopyFeedbackExpired => {
                self.copied = false;
                Task::none()
            }
            Message::DiffASelected(id) => {
                self.diff_a = Some(id);
                Task::none()
            }
            Message::DiffBSelected(id) => {
                self.diff_b = Some(id);
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        row![
            self.sidebar(),
            iced::widget::rule::vertical(1).style(theme::hairline),
            container(match self.screen {
                Screen::Terminal => self.terminal_screen(),
                Screen::Workbench => self.workbench_screen(),
            })
            .width(Fill)
            .height(Fill),
        ]
        .height(Fill)
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let nav_item = |label: &'static str, active: bool, on_press: Message| {
            button(text(label).size(theme::TEXT_BODY_SM))
                .padding([9, 12])
                .width(Fill)
                .style(move |_theme: &Theme, _status| button::Style {
                    background: active.then(|| theme::accent_quiet().into()),
                    text_color: if active {
                        theme::text_accent()
                    } else {
                        theme::text_muted()
                    },
                    border: iced::Border {
                        radius: theme::RADIUS_SM.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .on_press(on_press)
        };

        let nav = column![
            nav_item(
                "Live catch terminal",
                self.screen == Screen::Terminal,
                Message::GoTerminal
            ),
            nav_item(
                "API workbench",
                self.screen == Screen::Workbench,
                Message::GoWorkbench
            ),
        ]
        .spacing(2);

        let stream_dot_color = match self.connection {
            ConnectionStatus::Connected => theme::status_success(),
            ConnectionStatus::Connecting => theme::status_warning(),
            _ => theme::text_faint(),
        };
        let stream_label = match self.connection {
            ConnectionStatus::Connected => "Stream connected",
            ConnectionStatus::Connecting => "Connecting…",
            ConnectionStatus::Disconnected => "Reconnecting…",
            ConnectionStatus::Idle => "Not connected",
        };

        let footer = column![
            iced::widget::rule::horizontal(1).style(theme::hairline),
            row![
                container(Space::new().width(6.0).height(6.0)).style(move |_: &Theme| container::Style {
                    background: Some(stream_dot_color.into()),
                    border: iced::Border {
                        radius: 999.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                text(stream_label)
                    .size(theme::TEXT_MICRO)
                    .color(theme::text_muted()),
            ]
            .spacing(6)
            .align_y(Center)
            .padding(iced::Padding {
                top: 16.0,
                ..Default::default()
            }),
        ]
        .spacing(0);

        container(
            column![
                text("Ginei")
                    .size(14.0)
                    .font(theme::FONT_MEDIUM)
                    .color(theme::text_strong()),
                text("Webhooks")
                    .size(theme::TEXT_MICRO)
                    .color(theme::text_faint()),
                Space::new().height(theme::SPACE_5),
                nav,
                Space::new().height(Fill),
                footer,
            ]
            .spacing(theme::SPACE_2)
            .padding([theme::SPACE_6 as u16, theme::SPACE_5 as u16]),
        )
        .width(220.0)
        .height(Fill)
        .style(theme::sidebar_style)
        .into()
    }

    fn top_bar(&self, title: &'static str) -> Element<'_, Message> {
        let right: Element<'_, Message> = match self.screen {
            Screen::Terminal => row![
                container(
                    text(HOOK_URL_DISPLAY)
                        .size(12.0)
                        .color(theme::text_strong())
                )
                .padding([7, 12])
                .style(theme::inset_field_style),
                button(text(if self.copied { "✓" } else { "Copy" }).size(12.0))
                    .padding(8)
                    .style(move |_theme: &Theme, _status| button::Style {
                        background: Some(
                            if self.copied {
                                theme::accent_quiet()
                            } else {
                                theme::surface_raised()
                            }
                            .into()
                        ),
                        text_color: if self.copied {
                            theme::text_accent()
                        } else {
                            theme::text_body()
                        },
                        border: iced::Border {
                            color: theme::border_hairline(),
                            width: 1.0,
                            radius: theme::RADIUS_SM.into(),
                        },
                        ..Default::default()
                    })
                    .on_press(Message::CopyHookUrl),
            ]
            .spacing(theme::SPACE_3)
            .align_y(Center)
            .into(),
            Screen::Workbench => Space::new().into(),
        };

        container(
            row![
                text(title).size(15.0).color(theme::text_strong()),
                Space::new().width(Fill),
                right,
            ]
            .align_y(Center),
        )
        .padding([0, theme::SPACE_7 as u16])
        .height(64.0)
        .width(Fill)
        .style(theme::topbar_style)
        .into()
    }

    fn terminal_screen(&self) -> Element<'_, Message> {
        let selected = self
            .selected_id
            .as_ref()
            .and_then(|id| self.requests.iter().find(|r| &r.id == id));

        column![
            self.top_bar("Live catch terminal"),
            iced::widget::rule::horizontal(1).style(theme::hairline),
            row![
                self.feed_view(),
                iced::widget::rule::vertical(1).style(theme::hairline),
                self.inspector_view(selected),
            ]
            .height(Fill),
        ]
        .height(Fill)
        .into()
    }

    fn workbench_screen(&self) -> Element<'_, Message> {
        column![
            self.top_bar("API workbench"),
            iced::widget::rule::horizontal(1).style(theme::hairline),
            container(
                text("API workbench arrives in a later phase.")
                    .color(theme::text_faint())
                    .size(theme::TEXT_BODY_SM)
            )
            .center(Fill),
        ]
        .height(Fill)
        .into()
    }

    fn feed_view(&self) -> Element<'_, Message> {
        let header = text(format!("{} requests captured", self.requests.len()))
            .size(theme::TEXT_MICRO)
            .font(theme::FONT_MEDIUM)
            .color(theme::text_faint());

        let list: Element<'_, Message> = if self.requests.is_empty() {
            container(
                text("Captured webhooks will appear here in real time.")
                    .color(theme::text_faint())
                    .size(theme::TEXT_BODY_SM),
            )
            .center(Fill)
            .into()
        } else {
            let cards = self.requests.iter().map(|req| self.feed_card(req));
            scrollable(column(cards).spacing(theme::SPACE_3).padding(iced::Padding {
                top: 0.0,
                right: 16.0,
                bottom: 16.0,
                left: 16.0,
            }))
                .height(Fill)
                .into()
        };

        container(
            column![
                container(header).padding(iced::Padding {
                    top: 16.0,
                    right: 20.0,
                    bottom: 8.0,
                    left: 20.0,
                }),
                list,
            ]
            .height(Fill),
        )
        .width(320.0)
        .height(Fill)
        .into()
    }

    fn feed_card(&self, req: &CapturedRequest) -> Element<'_, Message> {
        let selected = self.selected_id.as_deref() == Some(req.id.as_str());

        let card = column![
            row![
                badge(theme::method_tone(&req.method), req.method.clone()),
                text(req.path.clone())
                    .size(theme::TEXT_BODY_SM)
                    .color(theme::text_strong())
                    .width(Fill),
                badge(theme::status_tone(req.status), req.status.to_string()),
            ]
            .spacing(theme::SPACE_3)
            .align_y(Center),
            row![
                text(req.received_at.clone())
                    .size(11.5)
                    .color(theme::text_muted()),
                text("·").size(11.5).color(theme::text_muted()),
                text(req.remote_addr.clone().unwrap_or_default())
                    .size(11.5)
                    .color(theme::text_muted()),
            ]
            .spacing(theme::SPACE_4),
        ]
        .spacing(6);

        let styled_card = container(card)
            .padding(theme::SPACE_4)
            .width(Fill)
            .style(theme::card_style(selected));

        button(styled_card)
            .on_press(Message::SelectRequest(req.id.clone()))
            .padding(0)
            .style(button::text)
            .into()
    }

    fn inspector_view<'a>(
        &'a self,
        selected: Option<&'a CapturedRequest>,
    ) -> Element<'a, Message> {
        let Some(req) = selected else {
            return container(
                text("Select a request to inspect it")
                    .color(theme::text_faint())
                    .size(theme::TEXT_BODY_SM),
            )
            .center(Fill)
            .width(Fill)
            .height(Fill)
            .into();
        };

        let header = column![
            row![
                badge(theme::method_tone(&req.method), req.method.clone()),
                badge(
                    if req.status >= 400 {
                        theme::Tone::Danger
                    } else {
                        theme::Tone::Success
                    },
                    req.status.to_string()
                ),
                text(format!(
                    "{} · {}",
                    req.received_at,
                    req.remote_addr.clone().unwrap_or_default()
                ))
                .size(11.5)
                .color(theme::text_muted()),
            ]
            .spacing(theme::SPACE_3)
            .align_y(Center),
            text(req.path.clone())
                .size(13.5)
                .color(theme::text_strong()),
            self.tabs_row(),
        ]
        .spacing(6)
        .padding(iced::Padding {
            top: 20.0,
            right: 20.0,
            bottom: 0.0,
            left: 20.0,
        });

        let content: Element<'_, Message> = match self.tab {
            InspectorTab::Headers => self.headers_view(req),
            InspectorTab::Body => self.body_view(req),
            InspectorTab::Diff => self.diff_view(),
        };

        container(
            column![
                header,
                iced::widget::rule::horizontal(1).style(theme::hairline),
                scrollable(container(content).padding(20)).height(Fill),
            ]
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn tabs_row(&self) -> Element<'_, Message> {
        let tab_button = |label: &'static str, tab: InspectorTab| {
            let is_active = self.tab == tab;
            button(text(label).size(theme::TEXT_BODY_SM))
                .padding(0)
                .style(move |_theme: &Theme, _status| button::Style {
                    background: None,
                    text_color: if is_active {
                        theme::text_strong()
                    } else {
                        theme::text_muted()
                    },
                    border: iced::Border {
                        color: if is_active {
                            theme::accent_solid()
                        } else {
                            iced::Color::TRANSPARENT
                        },
                        width: 2.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                })
                .on_press(Message::TabSelected(tab))
        };

        row![
            tab_button("Headers", InspectorTab::Headers),
            tab_button("Body", InspectorTab::Body),
            tab_button("Diff", InspectorTab::Diff),
        ]
        .spacing(22)
        .into()
    }

    fn headers_view(&self, req: &CapturedRequest) -> Element<'static, Message> {
        let auth = auth_badge(&req.headers);

        let rows: Vec<Element<'static, Message>> = req
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let border_bottom = i + 1 < req.headers.len();
                container(
                    column![
                        text(h.name.to_uppercase())
                            .size(10.5)
                            .color(theme::text_faint()),
                        text(h.value.clone())
                            .size(12.5)
                            .color(theme::text_body()),
                    ]
                    .spacing(3),
                )
                .padding([11, 16])
                .style(move |_theme: &Theme| iced::widget::container::Style {
                    border: iced::Border {
                        color: if border_bottom {
                            theme::border_hairline()
                        } else {
                            iced::Color::TRANSPARENT
                        },
                        width: if border_bottom { 1.0 } else { 0.0 },
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                })
                .width(Fill)
                .into()
            })
            .collect();

        column![
            badge(auth.0, auth.1),
            container(column(rows))
                .max_width(560.0)
                .style(theme::bordered_box),
        ]
        .spacing(theme::SPACE_5)
        .into()
    }

    fn body_view(&self, req: &CapturedRequest) -> Element<'static, Message> {
        let search_bar = container(
            row![
                text("/").size(14.0).color(theme::text_faint()),
                text_input("Search keys or values", &self.search)
                    .on_input(Message::SearchChanged)
                    .style(|_theme, _status| text_input::Style {
                        background: iced::Background::Color(iced::Color::TRANSPARENT),
                        border: iced::Border::default(),
                        icon: theme::text_faint(),
                        placeholder: theme::text_faint(),
                        value: theme::text_body(),
                        selection: theme::accent_quiet(),
                    }),
            ]
            .spacing(theme::SPACE_3)
            .align_y(Center),
        )
        .padding([9, 12])
        .max_width(380.0)
        .style(theme::inset_field_style);

        let body: Element<'static, Message> =
            match serde_json::from_slice::<serde_json::Value>(&req.body) {
                Ok(value) => json_view::view(&value, &self.expanded_json, &self.search),
                Err(_) => match std::str::from_utf8(&req.body) {
                    Ok(body_text) if !body_text.is_empty() => {
                        render_plain_text(body_text, &self.search)
                    }
                    Ok(_) => text("Empty body.").color(theme::text_faint()).into(),
                    Err(_) => text(format!("Binary body ({} bytes).", req.body.len()))
                        .color(theme::text_faint())
                        .into(),
                },
            };

        column![
            search_bar,
            container(body)
                .padding(18)
                .max_width(640.0)
                .style(|_theme: &Theme| iced::widget::container::Style {
                    background: Some(theme::surface_card().into()),
                    border: iced::Border {
                        color: theme::border_hairline(),
                        width: 1.0,
                        radius: theme::RADIUS_LG.into(),
                    },
                    ..Default::default()
                }),
        ]
        .spacing(theme::SPACE_5)
        .into()
    }

    fn diff_view(&self) -> Element<'_, Message> {
        let pairs: Vec<(String, String)> = self
            .requests
            .iter()
            .map(|r| {
                (
                    format!("{} {} · {}", r.method, r.path, r.received_at),
                    r.id.clone(),
                )
            })
            .collect();
        let options: Vec<String> = pairs.iter().map(|(label, _)| label.clone()).collect();
        let label_for_id = |id: &Option<String>| -> Option<String> {
            id.as_ref().and_then(|id| {
                pairs
                    .iter()
                    .find(|(_, pid)| pid == id)
                    .map(|(label, _)| label.clone())
            })
        };

        let pairs_a = pairs.clone();
        let picker_a = pick_list(options.clone(), label_for_id(&self.diff_a), move |label| {
            let id = pairs_a
                .iter()
                .find(|(l, _)| l == &label)
                .map(|(_, id)| id.clone())
                .unwrap_or_default();
            Message::DiffASelected(id)
        })
        .width(Fill);

        let pairs_b = pairs.clone();
        let picker_b = pick_list(options.clone(), label_for_id(&self.diff_b), move |label| {
            let id = pairs_b
                .iter()
                .find(|(l, _)| l == &label)
                .map(|(_, id)| id.clone())
                .unwrap_or_default();
            Message::DiffBSelected(id)
        })
        .width(Fill);

        let request_a = self
            .diff_a
            .as_ref()
            .and_then(|id| self.requests.iter().find(|r| &r.id == id));
        let request_b = self
            .diff_b
            .as_ref()
            .and_then(|id| self.requests.iter().find(|r| &r.id == id));

        let rows: Element<'_, Message> = match (request_a, request_b) {
            (Some(a), Some(b)) => container(diff_view::view(&a.body, &b.body))
                .max_width(640.0)
                .style(theme::bordered_box)
                .into(),
            _ => text("Pick two captures to compare.")
                .color(theme::text_faint())
                .size(theme::TEXT_BODY_SM)
                .into(),
        };

        column![
            row![picker_a, picker_b].spacing(theme::SPACE_3),
            rows,
        ]
        .spacing(theme::SPACE_5)
        .into()
    }
}

fn badge(tone: theme::Tone, label: impl Into<String>) -> Element<'static, Message> {
    container(
        text(label.into())
            .size(theme::TEXT_MICRO)
            .font(theme::FONT_MEDIUM),
    )
    .padding([0, theme::SPACE_3 as u16])
    .height(20.0)
    .align_y(Center)
    .style(theme::badge_style(tone))
    .into()
}

fn auth_badge(headers: &[HeaderPair]) -> (theme::Tone, &'static str) {
    let has = |name: &str| {
        headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case(name))
    };
    if has("authorization") || has("x-api-key") {
        (theme::Tone::Success, "AUTHENTICATED")
    } else if has("x-signature") {
        (theme::Tone::Accent, "SIGNED")
    } else {
        (theme::Tone::Warning, "UNVERIFIED")
    }
}

fn render_plain_text(body_text: &str, search: &str) -> Element<'static, Message> {
    let search_lower = search.to_lowercase();
    let lines = body_text
        .lines()
        .map(|line| {
            let matched = !search_lower.is_empty() && line.to_lowercase().contains(&search_lower);
            let label = text(line.to_string()).size(theme::TEXT_BODY_SM);
            if matched {
                label.color(theme::json::key()).into()
            } else {
                Element::from(label.color(theme::text_body()))
            }
        })
        .collect::<Vec<_>>();

    column(lines).spacing(1).into()
}

fn app_data_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|dir| dir.join("LiveTap"))
}

mod app;
mod diff_view;
mod json_view;
mod theme;

pub use app::Message;
use app::State;

fn main() -> iced::Result {
    iced::application(State::new, State::update, State::view)
        .subscription(State::subscription)
        .theme(State::theme)
        .default_font(theme::FONT_REGULAR)
        .font(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/JetBrainsMono-SemiBold.ttf").as_slice())
        .title("LiveTap")
        .run()
}

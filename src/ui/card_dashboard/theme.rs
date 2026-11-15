use ratatui::style::Color;

pub struct Theme;

impl Theme {
    pub const BG: Color = Color::Rgb(30, 30, 46);
    pub const CARD_BG: Color = Color::Rgb(24, 24, 37);
    pub const BORDER: Color = Color::Rgb(88, 91, 112);
    pub const BORDER_FOCUSED: Color = Color::Rgb(245, 194, 231);
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    pub const SUBTEXT: Color = Color::Rgb(166, 173, 200);
    pub const PINK: Color = Color::Rgb(245, 194, 231);
    pub const CYAN: Color = Color::Rgb(137, 220, 235);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
}

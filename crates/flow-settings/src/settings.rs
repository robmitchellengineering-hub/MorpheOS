//! What the Settings window shows and what a key or a click does to it. No Wayland and no
//! power-profiles-daemon here: the window is a list of the three power profiles, and every
//! input turns into at most one [`Action`] for the worker. So all of it is tested anywhere
//! Rust runs.

use flow_shell::{Colour, Frame, GLYPH_H, Rect};

/// The three power profiles, in the order they are listed. The profile's own name is what
/// `powerprofilesctl set <name>` takes.
pub const PROFILES: [(&str, &str); 3] = [
    ("power-saver", "Power saver"),
    ("balanced", "Balanced"),
    ("performance", "Performance"),
];

/// A key or a click, already decoded from the keyboard's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Enter,
    Escape,
    /// A press at a point in the window.
    Click {
        x: u32,
        y: u32,
    },
}

/// What the worker is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Set the chosen power profile (its profile name, as `powerprofilesctl` takes it).
    Set(String),
    Quit,
}

pub mod colours {
    use flow_shell::Colour;
    pub const BACKGROUND: Colour = Colour::rgb(0x0c, 0x10, 0x1a);
    pub const TITLE: Colour = Colour::rgb(0x18, 0x21, 0x2e);
    pub const TEXT: Colour = Colour::rgb(0xff, 0xff, 0xff);
    pub const DIM: Colour = Colour::rgb(0x7e, 0x87, 0x97);
    pub const SELECTED: Colour = Colour::rgb(0x0e, 0x6b, 0x34);
    pub const SEPARATOR: Colour = Colour::rgb(0x28, 0x31, 0x40);
    pub const ACTIVE: Colour = Colour::rgb(0x00, 0xff, 0x41);
}

pub const SCALE: u32 = 2;
const TITLE_H: u32 = 40;
const ROW_H: u32 = 44;
const PAD: u32 = 12;

/// The Settings window: the power-profile page, with one row selected.
#[derive(Debug, Clone)]
pub struct Settings {
    selected: usize,
    /// Which profile is active now (its index in [`PROFILES`]), so it reads as chosen.
    active: usize,
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            selected: 0,
            active: 1, // balanced is the default
        }
    }

    pub fn set_active(&mut self, name: &str) {
        if let Some(i) = PROFILES.iter().position(|(n, _)| *n == name) {
            self.active = i;
        }
    }

    pub fn handle(&mut self, input: Input) -> Action {
        match input {
            Input::Up => self.selected = self.selected.saturating_sub(1),
            Input::Down => {
                self.selected = (self.selected + 1).min(PROFILES.len() - 1);
            }
            Input::Enter => {
                return Action::Set(PROFILES[self.selected].0.to_string());
            }
            Input::Escape => return Action::Quit,
            Input::Click { x: _, y } => {
                if let Some(row) = row_at(y) {
                    self.selected = row;
                }
            }
        }
        Action::None
    }

    pub fn paint(&self, frame: &mut Frame) {
        let (w, h) = (frame.width(), frame.height());
        frame.clear(colours::BACKGROUND);
        frame.fill_rect(Rect::new(0, 0, w, TITLE_H), colours::TITLE);
        text(frame, "Settings", PAD, centre(0, TITLE_H), colours::TEXT);
        frame.fill_rect(Rect::new(0, TITLE_H - 1, w, 1), colours::SEPARATOR);

        text(frame, "Power", PAD, TITLE_H + PAD, colours::DIM);
        for (i, (_, label)) in PROFILES.iter().enumerate() {
            let y = TITLE_H + PAD + ROW_H + i as u32 * ROW_H;
            if i == self.selected {
                frame.fill_rect(Rect::new(0, y, w, ROW_H), colours::SELECTED);
            }
            text(frame, label, PAD + 8, centre(y, ROW_H), colours::TEXT);
            if i == self.active {
                text(
                    frame,
                    "on",
                    w - PAD - text_width("on") - 8,
                    centre(y, ROW_H),
                    colours::ACTIVE,
                );
            }
            if i + 1 < PROFILES.len() {
                frame.fill_rect(Rect::new(0, y + ROW_H - 1, w, 1), colours::SEPARATOR);
            }
        }
        let hint_y = h.saturating_sub(GLYPH_H * SCALE + PAD);
        text(
            frame,
            "Up/Down choose, Enter set, Esc close",
            PAD,
            hint_y,
            colours::DIM,
        );
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

fn row_at(y: u32) -> Option<usize> {
    let first = TITLE_H + PAD + ROW_H;
    if y < first {
        return None;
    }
    let row = ((y - first) / ROW_H) as usize;
    (row < PROFILES.len()).then_some(row)
}

fn text(frame: &mut Frame, s: &str, x: u32, y: u32, colour: Colour) {
    frame.text(s, x, y, SCALE, colour);
}

fn text_width(s: &str) -> u32 {
    s.chars().count() as u32 * flow_shell::GLYPH_ADVANCE * SCALE
}

fn centre(top: u32, band: u32) -> u32 {
    top + band.saturating_sub(GLYPH_H * SCALE) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_then_enter_sets_the_next_profile() {
        let mut s = Settings::new();
        assert_eq!(s.handle(Input::Down), Action::None);
        assert_eq!(s.handle(Input::Enter), Action::Set("balanced".to_string()));
    }

    #[test]
    fn up_clamps_at_the_top() {
        let mut s = Settings::new();
        assert_eq!(s.handle(Input::Up), Action::None);
        assert_eq!(
            s.handle(Input::Enter),
            Action::Set("power-saver".to_string())
        );
    }

    #[test]
    fn escape_quits_and_a_click_selects() {
        let mut s = Settings::new();
        assert_eq!(s.handle(Input::Escape), Action::Quit);
        // A click on the third row (performance) selects it.
        let y = TITLE_H + PAD + ROW_H + 2 * ROW_H + 10;
        assert_eq!(s.handle(Input::Click { x: 10, y }), Action::None);
        assert_eq!(
            s.handle(Input::Enter),
            Action::Set("performance".to_string())
        );
    }
}

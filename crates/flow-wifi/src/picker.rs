//! What the Wi-Fi picker shows and what a key or a click does to it.
//!
//! No Wayland and no NetworkManager here: the picker is a list of networks, a selection, a
//! password field and a status line, and every input turns into at most one [`Action`] for
//! the backend. So all of it is tested anywhere Rust runs (`cargo test -p flow-wifi`), and the
//! window and the D-Bus code only carry events in and actions out.

use flow_shell::{Colour, Frame, GLYPH_ADVANCE, GLYPH_H, Rect};

/// One network the radio can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    /// Signal strength, 0 to 100.
    pub strength: u8,
    /// It needs a password (WPA or WPA2 personal).
    pub secured: bool,
    /// The machine is connected to it now.
    pub active: bool,
}

/// What the picker is doing, shown on its status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Scanning,
    Ready,
    Connecting(String),
    Connected(String),
    Failed {
        ssid: String,
        why: Failure,
    },
    /// No Wi-Fi device, or NetworkManager could not be reached.
    Unavailable(String),
}

/// Why a connection did not come up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    WrongPassword,
    Other(String),
}

/// A key or a click, already decoded from the keyboard's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Enter,
    Escape,
    Backspace,
    Char(char),
    /// A press at a point in the window.
    Click {
        x: u32,
        y: u32,
    },
}

/// What the backend is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Connect {
        ssid: String,
        password: Option<String>,
    },
    Quit,
}

/// The longest WPA passphrase (IEEE 802.11i).
pub const MAX_PASSWORD: usize = 63;

/// The picker's colours, drawn from the brand palette (design/README.md): the near-black
/// `#05070d` depth, matrix green `#00ff41` for what is selected/joined, cyan for the live
/// "joining" state, and the whites/greys for text. Distinct from everything the shell draws,
/// so a test reading a screenshot finds the picker's parts by colour.
pub mod colours {
    use flow_shell::Colour;
    /// The panel, a touch above the brand's near-black so the window reads as a surface.
    pub const BACKGROUND: Colour = Colour::rgb(0x0c, 0x10, 0x1a);
    /// The title band, slightly raised above [`BACKGROUND`].
    pub const TITLE: Colour = Colour::rgb(0x18, 0x21, 0x2e);
    /// Text: the brand's white.
    pub const TEXT: Colour = Colour::rgb(0xff, 0xff, 0xff);
    /// Secondary text: the strength, the lock, the hints.
    pub const DIM: Colour = Colour::rgb(0x7e, 0x87, 0x97);
    /// The 1px dividers between the title, the rows and the status line.
    pub const SEPARATOR: Colour = Colour::rgb(0x28, 0x31, 0x40);
    /// The selected row: matrix green, darkened enough to hold white text.
    pub const SELECTED: Colour = Colour::rgb(0x0e, 0x6b, 0x34);
    /// The password field, inset in the brand's near-black.
    pub const FIELD: Colour = Colour::rgb(0x05, 0x07, 0x0d);
    /// The status line, by state: cyan while it works, matrix green when it lands, red when
    /// it fails, grey while it waits.
    pub const BUSY: Colour = Colour::rgb(0x06, 0xb6, 0xd4);
    pub const OK: Colour = Colour::rgb(0x00, 0xff, 0x41);
    pub const FAIL: Colour = Colour::rgb(0xd1, 0x3a, 0x4b);
    pub const IDLE: Colour = Colour::rgb(0x2e, 0x37, 0x4a);
}

/// Layout, in window pixels. The font is 5 x 7, drawn at [`SCALE`].
pub const SCALE: u32 = 2;
pub const TITLE_H: u32 = 40;
pub const ROW_H: u32 = 36;
pub const STATUS_H: u32 = 36;
pub const PAD: u32 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    List,
    Password { ssid: String, typed: String },
}

#[derive(Debug, Clone)]
pub struct Picker {
    networks: Vec<Network>,
    selected: usize,
    mode: Mode,
    status: Status,
}

impl Default for Picker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            networks: Vec::new(),
            selected: 0,
            mode: Mode::List,
            status: Status::Scanning,
        }
    }

    #[must_use]
    pub fn networks(&self) -> &[Network] {
        &self.networks
    }

    #[must_use]
    pub fn selected(&self) -> Option<&Network> {
        self.networks.get(self.selected)
    }

    #[must_use]
    pub fn status(&self) -> &Status {
        &self.status
    }

    /// The password being typed, while the picker asks for one.
    #[must_use]
    pub fn typing(&self) -> Option<(&str, &str)> {
        match &self.mode {
            Mode::Password { ssid, typed } => Some((ssid, typed)),
            Mode::List => None,
        }
    }

    /// A fresh scan. One row per name, at its strongest (a network with several access
    /// points shows once); hidden networks (no name) are left out. The connected network
    /// first, then the strongest; the selection stays on the network it was on.
    pub fn set_networks(&mut self, seen: Vec<Network>) {
        let keep = self.selected().map(|n| n.ssid.clone());
        let mut list: Vec<Network> = Vec::new();
        for n in seen.into_iter().filter(|n| !n.ssid.is_empty()) {
            match list.iter_mut().find(|m| m.ssid == n.ssid) {
                Some(m) => {
                    m.strength = m.strength.max(n.strength);
                    m.active |= n.active;
                    m.secured |= n.secured;
                }
                None => list.push(n),
            }
        }
        list.sort_by(|a, b| {
            b.active
                .cmp(&a.active)
                .then(b.strength.cmp(&a.strength))
                .then(a.ssid.cmp(&b.ssid))
        });
        self.selected = keep
            .and_then(|s| list.iter().position(|n| n.ssid == s))
            .unwrap_or(0);
        self.networks = list;
        if self.status == Status::Scanning {
            self.status = Status::Ready;
        }
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }

    /// One input. Returns what the backend must do, if anything.
    pub fn handle(&mut self, input: Input, height: u32) -> Action {
        if let Mode::Password { ssid, typed } = &mut self.mode {
            match input {
                Input::Char(c) if !c.is_control() && typed.chars().count() < MAX_PASSWORD => {
                    typed.push(c);
                }
                Input::Backspace => {
                    typed.pop();
                }
                Input::Escape => self.mode = Mode::List,
                // WPA needs 8 to 63 characters: a shorter one cannot be right, so say so
                // here rather than asking the network.
                Input::Enter if typed.chars().count() >= 8 => {
                    let action = Action::Connect {
                        ssid: ssid.clone(),
                        password: Some(std::mem::take(typed)),
                    };
                    self.status = Status::Connecting(ssid.clone());
                    self.mode = Mode::List;
                    return action;
                }
                Input::Enter => {
                    self.status = Status::Failed {
                        ssid: ssid.clone(),
                        why: Failure::Other("a Wi-Fi password has at least 8 characters".into()),
                    };
                }
                _ => {}
            }
            return Action::None;
        }
        match input {
            Input::Up => self.selected = self.selected.saturating_sub(1),
            Input::Down => {
                if self.selected + 1 < self.networks.len() {
                    self.selected += 1;
                }
            }
            Input::Escape => return Action::Quit,
            Input::Enter => return self.choose(),
            Input::Click { x: _, y } => {
                if let Some(row) = row_at(y, self.networks.len(), height) {
                    if row == self.selected {
                        return self.choose();
                    }
                    self.selected = row;
                }
            }
            _ => {}
        }
        Action::None
    }

    /// Enter on the selected network: ask for its password, or join it if it is open.
    fn choose(&mut self) -> Action {
        if matches!(self.status, Status::Connecting(_)) {
            return Action::None;
        }
        let Some(n) = self.selected().cloned() else {
            return Action::None;
        };
        if n.secured {
            self.mode = Mode::Password {
                ssid: n.ssid,
                typed: String::new(),
            };
            Action::None
        } else {
            self.status = Status::Connecting(n.ssid.clone());
            Action::Connect {
                ssid: n.ssid,
                password: None,
            }
        }
    }

    /// Draw the picker into a frame the size of its window.
    pub fn paint(&self, frame: &mut Frame) {
        let (w, h) = (frame.width(), frame.height());
        frame.clear(colours::BACKGROUND);
        // The title band, its label padded off the edges, and a hairline under it so the
        // title reads as a heading rather than a floating strip.
        frame.fill_rect(Rect::new(0, 0, w, TITLE_H), colours::TITLE);
        text(frame, "Wi-Fi", PAD, centre(0, TITLE_H), colours::TEXT);
        frame.fill_rect(Rect::new(0, TITLE_H - 1, w, 1), colours::SEPARATOR);

        let (status_text, status_colour) = self.status_line();
        let status_y = h.saturating_sub(STATUS_H);
        frame.fill_rect(Rect::new(0, status_y, w, STATUS_H), status_colour);
        frame.fill_rect(Rect::new(0, status_y, w, 1), colours::SEPARATOR);
        text(
            frame,
            &status_text,
            PAD,
            centre(status_y, STATUS_H),
            colours::TEXT,
        );

        if let Mode::Password { ssid, typed } = &self.mode {
            let y = TITLE_H + PAD;
            text(
                frame,
                &format!("Password for {ssid}"),
                PAD,
                y,
                colours::TEXT,
            );
            let field = Rect::new(PAD, y + ROW_H, w.saturating_sub(2 * PAD), ROW_H);
            frame.fill_rect(field, colours::FIELD);
            // Never the password itself: one mark per character, and a cursor.
            let marks: String = "*".repeat(typed.chars().count());
            text(
                frame,
                &format!("{marks}_"),
                field.x + PAD / 2,
                centre(field.y, ROW_H),
                colours::TEXT,
            );
            text(
                frame,
                "Enter to join, Esc to go back",
                PAD,
                y + 2 * ROW_H + PAD,
                colours::DIM,
            );
            return;
        }

        if self.networks.is_empty() {
            let msg = if self.status == Status::Scanning {
                "Looking for networks..."
            } else {
                "No networks found"
            };
            text(frame, msg, PAD, TITLE_H + PAD, colours::DIM);
            return;
        }
        for (i, n) in self.networks.iter().enumerate().take(visible_rows(h)) {
            let y = TITLE_H + i as u32 * ROW_H;
            if i == self.selected {
                frame.fill_rect(Rect::new(0, y, w, ROW_H), colours::SELECTED);
            }
            // A hairline between rows; the status line draws its own above it.
            if i + 1 < visible_rows(h) {
                frame.fill_rect(Rect::new(0, y + ROW_H - 1, w, 1), colours::SEPARATOR);
            }
            text(frame, &n.ssid, PAD, centre(y, ROW_H), colours::TEXT);
            let mut right = format!("{:>3}%", n.strength);
            if n.secured {
                right.push_str(" lock");
            }
            if n.active {
                right.push_str(" on");
            }
            let rw = text_width(&right);
            text(
                frame,
                &right,
                w.saturating_sub(PAD + rw),
                centre(y, ROW_H),
                colours::DIM,
            );
        }
    }

    fn status_line(&self) -> (String, Colour) {
        match &self.status {
            Status::Scanning => ("Scanning...".into(), colours::BUSY),
            Status::Ready => ("Choose a network, then Enter".into(), colours::IDLE),
            Status::Connecting(s) => (format!("Joining {s}..."), colours::BUSY),
            Status::Connected(s) => (format!("Connected to {s}"), colours::OK),
            Status::Failed {
                ssid,
                why: Failure::WrongPassword,
            } => (format!("Wrong password for {ssid}"), colours::FAIL),
            Status::Failed {
                ssid,
                why: Failure::Other(why),
            } => (format!("{ssid}: {why}"), colours::FAIL),
            Status::Unavailable(why) => (why.clone(), colours::FAIL),
        }
    }
}

fn visible_rows(height: u32) -> usize {
    (height.saturating_sub(TITLE_H + STATUS_H) / ROW_H) as usize
}

/// Which network row a point at `y` is on.
fn row_at(y: u32, rows: usize, height: u32) -> Option<usize> {
    let row = (y.checked_sub(TITLE_H)? / ROW_H) as usize;
    (row < rows.min(visible_rows(height))).then_some(row)
}

fn text(frame: &mut Frame, s: &str, x: u32, y: u32, colour: Colour) {
    frame.text(s, x, y, SCALE, colour);
}

fn text_width(s: &str) -> u32 {
    s.chars().count() as u32 * GLYPH_ADVANCE * SCALE
}

/// The y at which a line of text sits centred in a band.
fn centre(top: u32, band: u32) -> u32 {
    top + band.saturating_sub(GLYPH_H * SCALE) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(ssid: &str, strength: u8, secured: bool) -> Network {
        Network {
            ssid: ssid.into(),
            strength,
            secured,
            active: false,
        }
    }

    const H: u32 = 400;

    fn typed(p: &mut Picker, s: &str) {
        for c in s.chars() {
            assert_eq!(p.handle(Input::Char(c), H), Action::None);
        }
    }

    #[test]
    fn the_list_is_one_row_per_name_strongest_first_with_hidden_networks_left_out() {
        let mut p = Picker::new();
        p.set_networks(vec![
            net("Cafe", 40, false),
            net("Home", 60, true),
            net("", 99, true),
            net("Home", 80, true),
        ]);
        let names: Vec<_> = p
            .networks()
            .iter()
            .map(|n| (n.ssid.as_str(), n.strength))
            .collect();
        assert_eq!(names, [("Home", 80), ("Cafe", 40)]);
        assert_eq!(p.status(), &Status::Ready);
    }

    #[test]
    fn the_connected_network_comes_first_and_the_selection_survives_a_rescan() {
        let mut p = Picker::new();
        p.set_networks(vec![net("A", 90, false), net("B", 50, false)]);
        p.handle(Input::Down, H);
        assert_eq!(p.selected().unwrap().ssid, "B");
        let mut b = net("B", 50, false);
        b.active = true;
        p.set_networks(vec![net("A", 95, false), b, net("C", 70, false)]);
        assert_eq!(p.networks()[0].ssid, "B");
        assert_eq!(p.selected().unwrap().ssid, "B");
    }

    #[test]
    fn a_secured_network_asks_for_its_password_and_joins_with_it() {
        let mut p = Picker::new();
        p.set_networks(vec![net("Home", 80, true)]);
        assert_eq!(p.handle(Input::Enter, H), Action::None);
        assert_eq!(p.typing(), Some(("Home", "")));
        typed(&mut p, "correct horsex");
        p.handle(Input::Backspace, H);
        assert_eq!(
            p.handle(Input::Enter, H),
            Action::Connect {
                ssid: "Home".into(),
                password: Some("correct horse".into())
            }
        );
        assert_eq!(p.status(), &Status::Connecting("Home".into()));
        assert_eq!(p.typing(), None, "the password is not kept on screen");
    }

    #[test]
    fn a_password_too_short_for_wpa_is_refused_without_asking_the_network() {
        let mut p = Picker::new();
        p.set_networks(vec![net("Home", 80, true)]);
        p.handle(Input::Enter, H);
        typed(&mut p, "short");
        assert_eq!(p.handle(Input::Enter, H), Action::None);
        assert!(matches!(p.status(), Status::Failed { .. }));
        assert_eq!(p.typing(), Some(("Home", "short")), "still typing");
    }

    #[test]
    fn an_open_network_joins_at_once_and_escape_backs_out_or_quits() {
        let mut p = Picker::new();
        p.set_networks(vec![net("Home", 80, true), net("Cafe", 40, false)]);
        p.handle(Input::Enter, H);
        assert_eq!(p.handle(Input::Escape, H), Action::None);
        assert_eq!(p.typing(), None);
        p.handle(Input::Down, H);
        assert_eq!(
            p.handle(Input::Enter, H),
            Action::Connect {
                ssid: "Cafe".into(),
                password: None
            }
        );
        assert_eq!(
            p.handle(Input::Enter, H),
            Action::None,
            "not twice while joining"
        );
        assert_eq!(p.handle(Input::Escape, H), Action::Quit);
    }

    #[test]
    fn a_click_selects_a_row_and_a_second_click_chooses_it() {
        let mut p = Picker::new();
        p.set_networks(vec![net("A", 90, false), net("B", 50, false)]);
        let row_b = TITLE_H + ROW_H + ROW_H / 2;
        assert_eq!(p.handle(Input::Click { x: 30, y: row_b }, H), Action::None);
        assert_eq!(p.selected().unwrap().ssid, "B");
        assert_eq!(
            p.handle(Input::Click { x: 30, y: row_b }, H),
            Action::Connect {
                ssid: "B".into(),
                password: None
            }
        );
        // Below the last row, and on the title: nothing.
        let mut q = Picker::new();
        q.set_networks(vec![net("A", 90, false)]);
        assert_eq!(
            q.handle(
                Input::Click {
                    x: 30,
                    y: TITLE_H + 3 * ROW_H
                },
                H
            ),
            Action::None
        );
        assert_eq!(q.handle(Input::Click { x: 30, y: 5 }, H), Action::None);
    }

    fn count(frame: &Frame, c: Colour) -> usize {
        frame.pixels().iter().filter(|p| **p == c.to_argb()).count()
    }

    #[test]
    fn the_picture_shows_the_selection_the_status_and_never_the_password() {
        let mut p = Picker::new();
        p.set_networks(vec![net("Home", 80, true), net("Cafe", 40, false)]);
        let mut f = Frame::new(480, H);
        p.paint(&mut f);
        // The selected row is a band across the window, minus the text drawn on it.
        let sel = count(&f, colours::SELECTED);
        assert!(
            sel > 480 * ROW_H as usize * 8 / 10,
            "selected row: {sel} px"
        );
        assert_eq!(f.pixel(1, TITLE_H + 1), Some(colours::SELECTED));
        assert_eq!(f.pixel(1, TITLE_H + ROW_H + 1), Some(colours::BACKGROUND));
        // Moving the selection moves the band.
        p.handle(Input::Down, H);
        p.paint(&mut f);
        assert_eq!(f.pixel(1, TITLE_H + 1), Some(colours::BACKGROUND));
        assert_eq!(f.pixel(1, TITLE_H + ROW_H + 1), Some(colours::SELECTED));

        // The status line's colour says how joining went.
        for (status, colour) in [
            (Status::Connecting("Home".into()), colours::BUSY),
            (Status::Connected("Home".into()), colours::OK),
            (
                Status::Failed {
                    ssid: "Home".into(),
                    why: Failure::WrongPassword,
                },
                colours::FAIL,
            ),
        ] {
            p.set_status(status);
            p.paint(&mut f);
            assert_eq!(f.pixel(1, H - 2), Some(colour));
            assert!(count(&f, colour) > 480 * STATUS_H as usize * 8 / 10);
        }

        // Typing a password draws one mark per character, and the same picture for any
        // password of that length: what is typed is not on the screen.
        p.handle(Input::Up, H);
        p.handle(Input::Enter, H);
        typed(&mut p, "abcdefgh");
        let mut one = Frame::new(480, H);
        p.paint(&mut one);
        let mut q = p.clone();
        for _ in 0..8 {
            q.handle(Input::Backspace, H);
        }
        typed(&mut q, "zyxwvuts");
        let mut two = Frame::new(480, H);
        q.paint(&mut two);
        assert_eq!(one.pixels(), two.pixels());
        assert!(count(&one, colours::FIELD) > 0);
    }
}

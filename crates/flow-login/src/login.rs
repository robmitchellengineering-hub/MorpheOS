//! What the sign-in screen shows and what a key or a click does to it. No Wayland and no
//! account store here: the screen is a name field and a password field (plus a confirm on a
//! first boot), and every input turns into at most one [`Action`] for the window's worker.
//! So all of it is tested anywhere Rust runs.

use flow_shell::{Colour, Frame, GLYPH_H, Rect};

/// A key or a click, already decoded from the keyboard's layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Enter,
    Escape,
    Backspace,
    Tab,
    Char(char),
    /// A press at a point in the window.
    Click {
        x: u32,
        y: u32,
    },
}

/// What the window is asked to do, handed to its worker (the account store).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// First boot: make the account with this name and password.
    Create {
        name: String,
        password: String,
    },
    /// Later boot: check this name and password.
    SignIn {
        name: String,
        password: String,
    },
    Quit,
}

/// What the status line says, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Waiting for the person (first boot: type a name and password; else: sign in).
    Idle,
    /// The password is too short to be a password.
    TooShort,
    /// First boot: the two passwords do not match.
    Mismatch,
    /// Sign-in: the name or password did not match the account.
    Wrong,
    /// The account could not be written or read.
    Store(String),
    /// The worker is checking a sign-in.
    Checking,
}

/// The minimum password length the screen accepts.
pub const MIN_PASSWORD: usize = 8;

/// The brand colours, drawn from the shell's palette so the sign-in screen belongs to the
/// same machine (design/README.md): the near-black depth, matrix green for the accent, and
/// the greys for text and fields.
pub mod colours {
    use flow_shell::Colour;
    pub const BACKGROUND: Colour = Colour::rgb(0x05, 0x07, 0x0d);
    pub const CARD: Colour = Colour::rgb(0x0c, 0x10, 0x1a);
    pub const TEXT: Colour = Colour::rgb(0xff, 0xff, 0xff);
    pub const DIM: Colour = Colour::rgb(0x7e, 0x87, 0x97);
    pub const FIELD: Colour = Colour::rgb(0x0a, 0x0d, 0x14);
    pub const FOCUS: Colour = Colour::rgb(0x00, 0xff, 0x41);
    pub const FAIL: Colour = Colour::rgb(0xd1, 0x3a, 0x4b);
    pub const OK: Colour = Colour::rgb(0x00, 0xff, 0x41);
    pub const SEPARATOR: Colour = Colour::rgb(0x28, 0x31, 0x40);
}

/// Layout, in window pixels, at [`SCALE`].
pub const SCALE: u32 = 3;
const PAD: u32 = 24;
const FIELD_H: u32 = 44;
const FIELD_GAP: u32 = 12;
const BRAND_H: u32 = 120;

/// Which field the cursor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Password,
    Confirm,
}

#[derive(Debug, Clone)]
struct Mode {
    first_boot: bool,
    name: String,
    password: String,
    confirm: String,
    field: Field,
    status: Status,
}

#[derive(Debug, Clone)]
pub struct Login {
    mode: Mode,
}

impl Login {
    /// The screen as it should appear this boot: a first boot asks for a name and password
    /// (twice); every later boot asks for a password only, next to the saved name.
    pub fn new(first_boot: bool, saved_name: Option<&str>) -> Self {
        Login {
            mode: Mode {
                first_boot,
                name: saved_name.unwrap_or_default().to_string(),
                password: String::new(),
                confirm: String::new(),
                field: if first_boot {
                    Field::Name
                } else {
                    Field::Password
                },
                status: Status::Idle,
            },
        }
    }

    /// A key or a click; what the worker should do about it, if anything.
    pub fn handle(&mut self, input: Input) -> Action {
        match input {
            // Esc clears the field in hand rather than quitting: the screen must not be
            // skippable — it exits only when a sign-in or account creation succeeds.
            Input::Escape => self.clear_field(),
            Input::Char(c) if !c.is_control() => {
                self.type_char(c);
            }
            // Control characters (already decoded to Backspace/Tab/Enter/etc., or a stray
            // one) do nothing.
            Input::Char(_) => {}
            Input::Backspace => self.backspace(),
            Input::Tab | Input::Down => self.next_field(),
            Input::Up => self.prev_field(),
            Input::Enter => return self.submit(),
            Input::Click { x, y } => {
                if let Some(field) = self.field_at(x, y) {
                    self.mode.field = field;
                }
            }
        }
        Action::None
    }

    fn type_char(&mut self, c: char) {
        let m = &mut self.mode;
        let value = match m.field {
            Field::Name => &mut m.name,
            Field::Password => &mut m.password,
            Field::Confirm => &mut m.confirm,
        };
        // A name and a password are both short; no need to be unbounded.
        if value.chars().count() < 64 {
            value.push(c);
        }
        m.status = Status::Idle;
    }

    fn backspace(&mut self) {
        let m = &mut self.mode;
        let value = match m.field {
            Field::Name => &mut m.name,
            Field::Password => &mut m.password,
            Field::Confirm => &mut m.confirm,
        };
        value.pop();
        m.status = Status::Idle;
    }

    /// Esc clears the field in hand (a soft reset), never the whole screen.
    fn clear_field(&mut self) {
        let m = &mut self.mode;
        let value = match m.field {
            Field::Name => &mut m.name,
            Field::Password => &mut m.password,
            Field::Confirm => &mut m.confirm,
        };
        value.clear();
        m.status = Status::Idle;
    }

    fn next_field(&mut self) {
        let m = &mut self.mode;
        m.field = match m.field {
            Field::Name => Field::Password,
            Field::Password if m.first_boot => Field::Confirm,
            Field::Password | Field::Confirm => Field::Name,
        };
    }

    fn prev_field(&mut self) {
        let m = &mut self.mode;
        m.field = match m.field {
            Field::Name if m.first_boot => Field::Confirm,
            Field::Name => Field::Password,
            Field::Password => Field::Name,
            Field::Confirm => Field::Password,
        };
    }

    fn submit(&mut self) -> Action {
        let m = &self.mode;
        if m.first_boot {
            if m.name.trim().is_empty() {
                return Action::None;
            }
            if m.password.chars().count() < MIN_PASSWORD {
                self.mode.status = Status::TooShort;
                return Action::None;
            }
            if m.confirm != m.password {
                self.mode.status = Status::Mismatch;
                return Action::None;
            }
            Action::Create {
                name: m.name.trim().to_string(),
                password: m.password.clone(),
            }
        } else {
            if m.password.is_empty() {
                return Action::None;
            }
            Action::SignIn {
                name: m.name.trim().to_string(),
                password: m.password.clone(),
            }
        }
    }

    /// The window's worker calls this with the result of a sign-in attempt.
    pub fn set_status(&mut self, status: Status) {
        self.mode.status = status;
    }

    /// The screen's y origin for a field band, and its height, so a click can choose one.
    fn field_at(&self, x: u32, y: u32) -> Option<Field> {
        for (i, field) in self.field_rows().iter().enumerate() {
            let top = BRAND_H + 40 + i as u32 * (FIELD_H + FIELD_GAP);
            if (top..top + FIELD_H).contains(&y) && (PAD..PAD + 360).contains(&x) {
                return Some(*field);
            }
        }
        None
    }

    fn field_rows(&self) -> Vec<Field> {
        if self.mode.first_boot {
            vec![Field::Name, Field::Password, Field::Confirm]
        } else {
            vec![Field::Name, Field::Password]
        }
    }

    pub fn paint(&self, frame: &mut Frame) {
        let (w, h) = (frame.width(), frame.height());
        frame.clear(colours::BACKGROUND);
        // The brand mark: a matrix-green "M" block above the title, so the screen reads as
        // MorpheOS before a word does.
        let mx = w / 2 - 24;
        let my = BRAND_H - 52;
        frame.fill_rect(Rect::new(mx, my, 48, 6), colours::FOCUS);
        frame.fill_rect(Rect::new(mx, my, 6, 40), colours::FOCUS);
        frame.fill_rect(Rect::new(mx + 42, my, 6, 40), colours::FOCUS);
        frame.fill_rect(Rect::new(mx + 21, my + 16, 6, 24), colours::FOCUS);

        let title = if self.mode.first_boot {
            "Create your account"
        } else {
            "Sign in"
        };
        text(frame, "MorpheOS", PAD, 24, colours::TEXT);
        text(frame, title, PAD, 24 + GLYPH_H * SCALE + 12, colours::DIM);

        let rows = self.field_rows();
        for (i, field) in rows.iter().enumerate() {
            let top = BRAND_H + 40 + i as u32 * (FIELD_H + FIELD_GAP);
            let label = match field {
                Field::Name => "Name",
                Field::Password => "Password",
                Field::Confirm => "Confirm password",
            };
            text(frame, label, PAD, top - GLYPH_H * SCALE - 4, colours::DIM);
            let rect = Rect::new(PAD, top, 360, FIELD_H);
            frame.fill_rect(rect, colours::FIELD);
            if self.mode.field == *field {
                frame.fill_rect(Rect::new(rect.x, rect.y, 3, rect.h), colours::FOCUS);
            }
            let value = match field {
                Field::Name => &self.mode.name,
                Field::Password => &self.mode.password,
                Field::Confirm => &self.mode.confirm,
            };
            // Passwords show one mark per character, never the password itself.
            let shown: String = if *field == Field::Name {
                value.clone()
            } else {
                "*".repeat(value.chars().count())
            };
            let shown = if self.mode.field == *field {
                format!("{shown}_")
            } else {
                shown
            };
            text(
                frame,
                &shown,
                rect.x + 14,
                centre(rect.y, FIELD_H),
                colours::TEXT,
            );
            if i + 1 < rows.len() {
                frame.fill_rect(
                    Rect::new(PAD, top + FIELD_H + FIELD_GAP / 2 - 1, 360, 1),
                    colours::SEPARATOR,
                );
            }
        }

        let status = self.status_line();
        let sy = h.saturating_sub(GLYPH_H * SCALE + 24);
        text(frame, &status.0, PAD, sy, status.1);

        let hint = if self.mode.first_boot {
            "Enter to create, Esc to quit"
        } else {
            "Enter to sign in, Esc to quit"
        };
        text(frame, hint, PAD, sy + GLYPH_H * SCALE + 8, colours::DIM);
    }

    fn status_line(&self) -> (String, Colour) {
        match &self.mode.status {
            Status::Idle => (String::new(), colours::DIM),
            Status::TooShort => (
                format!("Use at least {MIN_PASSWORD} characters"),
                colours::FAIL,
            ),
            Status::Mismatch => ("The passwords do not match".into(), colours::FAIL),
            Status::Wrong => ("Wrong name or password".into(), colours::FAIL),
            Status::Store(why) => (why.clone(), colours::FAIL),
            Status::Checking => ("Checking...".into(), colours::OK),
        }
    }
}

fn text(frame: &mut Frame, s: &str, x: u32, y: u32, colour: Colour) {
    frame.text(s, x, y, SCALE, colour);
}

/// The y at which a line of text sits centred in a band.
fn centre(top: u32, band: u32) -> u32 {
    top + band.saturating_sub(GLYPH_H * SCALE) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_boot() -> Login {
        Login::new(true, None)
    }

    fn sign_in(name: &str) -> Login {
        Login::new(false, Some(name))
    }

    #[test]
    fn first_boot_walks_name_then_password_then_confirm_and_creates() {
        let mut l = first_boot();
        for c in "morph".chars() {
            assert_eq!(l.handle(Input::Char(c)), Action::None);
        }
        assert_eq!(l.handle(Input::Tab), Action::None);
        for c in "hunter22".chars() {
            assert_eq!(l.handle(Input::Char(c)), Action::None);
        }
        assert_eq!(l.handle(Input::Tab), Action::None);
        for c in "hunter22".chars() {
            assert_eq!(l.handle(Input::Char(c)), Action::None);
        }
        assert_eq!(
            l.handle(Input::Enter),
            Action::Create {
                name: "morph".into(),
                password: "hunter22".into()
            }
        );
    }

    #[test]
    fn a_mismatched_confirm_refuses_and_reports_it() {
        let mut l = first_boot();
        for c in "morph".chars() {
            let _ = l.handle(Input::Char(c));
        }
        let _ = l.handle(Input::Tab);
        for c in "hunter22".chars() {
            let _ = l.handle(Input::Char(c));
        }
        let _ = l.handle(Input::Tab);
        for c in "hunter23".chars() {
            let _ = l.handle(Input::Char(c));
        }
        assert_eq!(l.handle(Input::Enter), Action::None);
        assert!(matches!(l.mode.status, Status::Mismatch));
    }

    #[test]
    fn a_short_password_refuses() {
        let mut l = first_boot();
        for c in "morph".chars() {
            let _ = l.handle(Input::Char(c));
        }
        let _ = l.handle(Input::Tab);
        for c in "short".chars() {
            let _ = l.handle(Input::Char(c));
        }
        let _ = l.handle(Input::Tab);
        for c in "short".chars() {
            let _ = l.handle(Input::Char(c));
        }
        assert_eq!(l.handle(Input::Enter), Action::None);
        assert!(matches!(l.mode.status, Status::TooShort));
    }

    #[test]
    fn sign_in_starts_on_the_password_and_signs_in() {
        let mut l = sign_in("morph");
        assert_eq!(l.mode.field, Field::Password);
        for c in "hunter22".chars() {
            let _ = l.handle(Input::Char(c));
        }
        assert_eq!(
            l.handle(Input::Enter),
            Action::SignIn {
                name: "morph".into(),
                password: "hunter22".into()
            }
        );
    }

    #[test]
    fn escape_clears_the_field_in_hand_and_never_quits() {
        let mut l = first_boot();
        for c in "morph".chars() {
            let _ = l.handle(Input::Char(c));
        }
        // Esc clears the name, and the screen is still up (no Action::Quit exists now).
        assert_eq!(l.handle(Input::Escape), Action::None);
        assert!(l.mode.name.is_empty());
    }
}

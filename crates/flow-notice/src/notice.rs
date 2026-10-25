//! What the notice window shows: one message, in one colour.
//!
//! No Wayland here: a notice is a message and a colour, and [`Notice::paint`] puts both on a
//! [`Frame`]. The one colour is the whole accent — the title band and the hairline under it —
//! and it is what a test reading a screendump finds the notice by (AGENTS.md H20).

use flow_shell::{Colour, Frame, GLYPH_ADVANCE, GLYPH_H, Rect};

/// The notice's colours. The three kind colours ([`WARN`], [`OK`], [`FAIL`]) are the accent a
/// caller picks and a screendump reads the notice by; the rest is the window's chrome, shared
/// by every notice whatever its colour.
pub mod colours {
    use flow_shell::Colour;
    /// The panel behind the message, a touch above the shell's near-black.
    pub const BACKGROUND: Colour = Colour::rgb(0x13, 0x16, 0x1c);
    /// Text: the brand's white.
    pub const TEXT: Colour = Colour::rgb(0xff, 0xff, 0xff);
    /// The hint under the message.
    pub const DIM: Colour = Colour::rgb(0x8a, 0x93, 0xa3);
    /// A warning: the disk is still full (S7), or a change was rolled back (S8).
    pub const WARN: Colour = Colour::rgb(0xff, 0x6a, 0x00);
    /// Good news: the guard got the disk back above the floor.
    pub const OK: Colour = Colour::rgb(0x00, 0xe0, 0x8a);
    /// Something needs attention and could not be fixed by itself.
    pub const FAIL: Colour = Colour::rgb(0xff, 0x2d, 0x55);
}

/// The accent colour a name picks out, for `flow-notice --colour <name>`.
#[must_use]
pub fn colour_by_name(name: &str) -> Option<Colour> {
    match name {
        "warn" => Some(colours::WARN),
        "ok" => Some(colours::OK),
        "fail" => Some(colours::FAIL),
        _ => None,
    }
}

/// Layout, in window pixels. The font is 5 x 7, drawn at [`SCALE`].
pub const SCALE: u32 = 2;
pub const TITLE_H: u32 = 40;
pub const PAD: u32 = 12;
/// The height of the title band plus the space a message line sits in.
pub const LINE_H: u32 = 26;

/// One notice: a message and the one colour it is shown in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    message: String,
    colour: Colour,
}

impl Notice {
    #[must_use]
    pub fn new(message: impl Into<String>, colour: Colour) -> Self {
        Self {
            message: message.into(),
            colour,
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn colour(&self) -> Colour {
        self.colour
    }

    /// Draw the notice into a frame the size of its window.
    ///
    /// The title band and the hairline under it are the accent [`Self::colour`], so a
    /// screendump reads the notice by that colour; the message is wrapped to the window's
    /// width, so a long sentence (the guard's "how full, what was freed") is not clipped.
    pub fn paint(&self, frame: &mut Frame) {
        let (w, _h) = (frame.width(), frame.height());
        frame.clear(colours::BACKGROUND);
        frame.fill_rect(Rect::new(0, 0, w, TITLE_H), self.colour);
        text(frame, "Notice", PAD, centre(0, TITLE_H), colours::TEXT);
        frame.fill_rect(Rect::new(0, TITLE_H - 1, w, 1), colours::BACKGROUND);

        // The message, wrapped so it always fits whatever width the window was given.
        let per_line = chars_per_line(w);
        let mut y = TITLE_H + PAD;
        for line in wrap(&self.message, per_line) {
            text(frame, &line, PAD, y, colours::TEXT);
            y += LINE_H;
        }
        text(frame, "Press a key to dismiss", PAD, y + PAD, colours::DIM);
    }
}

/// How many characters fit on one line at the window's width, at the notice's scale.
fn chars_per_line(width: u32) -> usize {
    let inner = width.saturating_sub(2 * PAD);
    (inner / (GLYPH_ADVANCE * SCALE)) as usize
}

/// Split `s` into lines no longer than `per_line` characters, breaking on spaces where it can
/// and mid-word where a single word is longer than a line. Empty input yields one empty line
/// (the notice still shows something).
fn wrap(s: &str, per_line: usize) -> Vec<String> {
    let per_line = per_line.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for mut word in s.split(' ') {
        let sep = if cur.is_empty() { "" } else { " " };
        if !cur.is_empty() && cur.chars().count() + sep.len() + word.chars().count() > per_line {
            lines.push(std::mem::take(&mut cur));
        }
        while word.chars().count() > per_line {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            let (head, tail) = split_at(word, per_line);
            lines.push(head.to_string());
            word = tail;
        }
        if !word.is_empty() {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Split a word at a character boundary, `n` characters and the rest.
fn split_at(s: &str, n: usize) -> (&str, &str) {
    let idx = s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len());
    s.split_at(idx)
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

    fn count(frame: &Frame, c: Colour) -> usize {
        frame.pixels().iter().filter(|p| **p == c.to_argb()).count()
    }

    #[test]
    fn the_picture_shows_the_accent_band_and_never_clips_a_long_message() {
        let n = Notice::new(
            "free space 2% is below the 10% floor; pruned 3 old versions; still below the floor",
            colours::WARN,
        );
        let mut f = Frame::new(480, 180);
        n.paint(&mut f);
        // The whole title band is the accent colour, and the hairline under it is not.
        assert_eq!(f.pixel(1, 1), Some(colours::WARN));
        assert!(count(&f, colours::WARN) > 480 * TITLE_H as usize * 8 / 10);
        // The message (wrapped, not clipped) is drawn somewhere below the band.
        assert!(count(&f, colours::TEXT) > 0);
    }

    #[test]
    fn colour_by_name_picks_the_three_kinds() {
        assert_eq!(colour_by_name("warn"), Some(colours::WARN));
        assert_eq!(colour_by_name("ok"), Some(colours::OK));
        assert_eq!(colour_by_name("fail"), Some(colours::FAIL));
        assert_eq!(colour_by_name("nope"), None);
    }

    #[test]
    fn wrapping_breaks_on_spaces_and_on_long_words() {
        assert_eq!(wrap("one two three", 8), ["one two", "three"]);
        assert_eq!(
            wrap("a verylongword indeed", 8),
            ["a", "verylong", "word", "indeed"]
        );
        assert_eq!(wrap("", 8), [""]);
    }
}

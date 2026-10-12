//! A session's terminal surface: the text it has accumulated, and where it is drawn.
//!
//! **What this is, and what it is not.** A terminal surface is the honest first step of a
//! terminal: a buffer that holds the characters a session is given, and a cursor. It is
//! **not** a pty and **not** a real terminal emulator — there is no shell behind it, no
//! escape sequences, no scrollback, and a line longer than the surface simply clips
//! rather than wrapping. Those are later, and this record says so instead of letting a
//! rectangle labelled "terminal" stand in for one. What is real here is the increment: a
//! keypress changes the buffer, and the painted frame reflects the buffer.

use crate::font;
use crate::{Frame, GLYPH_ADVANCE, GLYPH_H, GLYPH_W, LINE_ADVANCE, Rect, TEXT_PAD, palette};

/// The text one session has accumulated, and where its cursor sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminal {
    /// The lines, top to bottom. No line ends in a newline character; the line break is
    /// the boundary between elements.
    lines: Vec<Vec<char>>,
    /// Cursor column, in characters, on the current line.
    col: usize,
    /// Cursor row, in lines.
    row: usize,
}

impl Terminal {
    /// A terminal with one empty line and the cursor at its start.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            col: 0,
            row: 0,
        }
    }

    /// Append a character at the cursor and advance past it.
    ///
    /// The character is *held* even when it has no glyph in the font, so text is never
    /// silently dropped; what cannot be drawn is simply not drawn.
    pub fn insert(&mut self, c: char) {
        self.lines[self.row].insert(self.col, c);
        self.col += 1;
    }

    /// Delete the character left of the cursor. At the start of a line this does nothing:
    /// joining lines is a real terminal's job and is not claimed here.
    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.lines[self.row].remove(self.col);
        }
    }

    /// Start a new line: everything right of the cursor moves down, and the cursor moves
    /// to the start of the new line.
    pub fn newline(&mut self) {
        let rest = self.lines[self.row].split_off(self.col);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    /// The lines, each as its characters in order.
    #[must_use]
    pub fn lines(&self) -> &[Vec<char>] {
        &self.lines
    }

    /// The cursor position as `(column, row)`.
    #[must_use]
    pub const fn cursor(&self) -> (usize, usize) {
        (self.col, self.row)
    }

    /// The whole buffer as one string, lines joined with newlines.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

/// Paint a session's terminal into its surface: the text, and the cursor if it has focus.
///
/// The cursor is painted last so it is always visible as a block, and it is painted only
/// on the focused session — one cursor in the frame, at the place input goes. The fills
/// are clipped by [`Frame`], so a surface smaller than a cell or a cursor past the edge is
/// a no-op, not an error.
pub(crate) fn paint(frame: &mut Frame, terminal: &Terminal, surface: Rect, focused: bool) {
    let origin_x = surface.x.saturating_add(TEXT_PAD);
    let origin_y = surface.y.saturating_add(TEXT_PAD);

    for (row, line) in terminal.lines().iter().enumerate() {
        for (col, ch) in line.iter().enumerate() {
            let x = origin_x.saturating_add(col as u32 * GLYPH_ADVANCE);
            let y = origin_y.saturating_add(row as u32 * LINE_ADVANCE);
            paint_glyph(frame, *ch, x, y);
        }
    }

    if focused {
        let (col, row) = terminal.cursor();
        let x = origin_x.saturating_add(col as u32 * GLYPH_ADVANCE);
        let y = origin_y.saturating_add(row as u32 * LINE_ADVANCE);
        frame.fill_rect(Rect::new(x, y, GLYPH_W, GLYPH_H), palette::ACCENT);
    }
}

/// Draw one glyph at `(x, y)`, one pixel at a time so the frame clips it.
fn paint_glyph(frame: &mut Frame, ch: char, x: u32, y: u32) {
    for (col, byte) in font::glyph(ch).iter().enumerate() {
        for row in 0..GLYPH_H {
            if byte & (1 << row) != 0 {
                frame.fill_rect(Rect::new(x + col as u32, y + row, 1, 1), palette::TEXT);
            }
        }
    }
}

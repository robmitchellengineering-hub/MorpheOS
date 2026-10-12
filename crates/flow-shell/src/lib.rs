//! The Flow shell's own frame.
//!
//! **What this is:** the first slice of the Flow shell ([ADR 0010]) reduced to the one
//! thing that can be checked without a GPU, a display server or a client. A session is
//! laid out, a frame is painted, and **the frame is only evidence if something was
//! actually drawn on it** — that assertion replaces "a systemd unit says `active`". It is
//! a build check and nothing more, and it is deliberately NOT named after anybody: the
//! name of the person a machine belongs to belongs **in the machine**, which is what the
//! `.rob` mark is (`morph-manifest`), not in CI.
//!
//! **Why no graphics library rasterises this.** The shell's own chrome is the bar, the
//! session surfaces and one focus indicator — rectangle fills and nothing more. The
//! obvious dependency for that is the `pixman` crate, and it is the wrong one at this
//! stage for a reason worth writing down: `pixman-sys` links against a **system**
//! `libpixman-1`, which a stock macOS does not have, so the crate *compiles* under
//! `cargo check` and then **fails to link** on the machine this shell is being built on.
//! A dependency that cannot link on a developer's laptop buys nothing for three rectangle
//! fills. So this crate has **no dependencies at all**, and it is testable wherever Rust
//! runs — which is the property the shell's proof actually needs. Compositing real client
//! buffers is Smithay's job, later, on Linux, where those libraries exist.
//!
//! **What is now present, and what is still absent.** Input is here as a model, not a
//! backend: a [`Key`] or a [`Pointer`] lands on a session through [`Shell::handle`] — a
//! press or Tab moves focus, and a character goes into the focused session's
//! [`Terminal`], a buffer that accumulates text and is painted with a cursor. "The user
//! typed this" is now a picture rather than a claim. The session a shell lays out is no
//! longer defined here: it is a [`flow_session::Session`] — identity, label, feel, and what
//! it is opening — carried inside a [`SessionView`] that adds only the two things the shell
//! owns, where the session is drawn and the text it has accumulated. Per-feel *layout* is
//! the next slice (ADR 0013); for now the shell carries the feel and paints the one frame.
//! Still absent: the Intent Bar, multi-display, capsules, the Device Broker, a real Wayland
//! client, and any pty. The terminal is a text buffer with a cursor, not a shell behind a
//! tty — it is honest as long as it is called that.
//!
//! [ADR 0010]: ../../docs/adr/0010-flow-shell-first.md
//! [ADR 0011]: ../../docs/adr/0011-the-humans-mark.md

use std::collections::BTreeSet;

mod font;
mod input;
mod png;
mod terminal;

// The session model lives in `flow-session` (ADR 0010 step 4); this crate is one consumer of
// it. `Session` and `SessionId` are re-exported so a caller of the shell meets exactly one
// session concept; `Feel`/`Openable`/`OpenableKind` ride along because they appear on the
// model the shell carries and a caller must be able to name them without reaching past the
// shell for the model crate.
pub use flow_session::{Feel, Openable, OpenableKind, Session, SessionId};
pub use input::{Event, Key, Pointer};
pub use terminal::Terminal;

/// The reference frame size. A real display negotiates its own; this is the size the
/// shell lays out for when nothing has told it otherwise.
pub const DEFAULT_WIDTH: u32 = 1280;
/// See [`DEFAULT_WIDTH`].
pub const DEFAULT_HEIGHT: u32 = 800;
/// Height of the session bar across the top of the frame.
pub const BAR_HEIGHT: u32 = 32;
/// Space between the frame edge and the content, and between sessions.
pub const MARGIN: u32 = 24;
/// Width of the focus indicator drawn on the bar.
pub const FOCUS_MARK_WIDTH: u32 = 96;
/// Width of one terminal glyph.
pub const GLYPH_W: u32 = 5;
/// Height of one terminal glyph.
pub const GLYPH_H: u32 = 7;
/// Horizontal advance of one terminal character: the glyph plus a pixel of spacing.
pub const GLYPH_ADVANCE: u32 = GLYPH_W + 1;
/// Vertical advance of one terminal line: the glyph plus a pixel of spacing.
pub const LINE_ADVANCE: u32 = GLYPH_H + 1;
/// Space between a session's surface edge and its terminal text.
pub const TEXT_PAD: u32 = 6;

/// A colour in the frame, stored as ARGB - the layout a compositor and a PPM both use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour(u32);

impl Colour {
    /// Build a colour from its parts, including alpha.
    #[must_use]
    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Self(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    /// Build an opaque colour from its parts.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::argb(0xff, r, g, b)
    }

    /// The colour as one 32-bit word.
    #[must_use]
    pub const fn to_argb(self) -> u32 {
        self.0
    }

    /// The parts back out, as `(a, r, g, b)`.
    #[must_use]
    pub const fn channels(self) -> (u8, u8, u8, u8) {
        (
            ((self.0 >> 24) & 0xff) as u8,
            ((self.0 >> 16) & 0xff) as u8,
            ((self.0 >> 8) & 0xff) as u8,
            (self.0 & 0xff) as u8,
        )
    }
}

/// The shell's palette, in one place so a frame can be read as what it is.
pub mod palette {
    use super::Colour;

    /// Behind everything.
    pub const BACKGROUND: Colour = Colour::rgb(0x10, 0x12, 0x16);
    /// The session bar.
    pub const BAR: Colour = Colour::rgb(0x1b, 0x1f, 0x27);
    /// A session surface that does not have focus.
    pub const SURFACE: Colour = Colour::rgb(0x22, 0x27, 0x30);
    /// A session surface that does.
    pub const SURFACE_FOCUSED: Colour = Colour::rgb(0x2d, 0x34, 0x40);
    /// The focus indicator and the cursor — the bright things that mark where the person is.
    pub const ACCENT: Colour = Colour::rgb(0x5a, 0xa8, 0xff);
    /// The terminal's text.
    pub const TEXT: Colour = Colour::rgb(0xd8, 0xde, 0xe9);
}

/// An axis-aligned rectangle, in pixels, with the origin at the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Distance from the left edge.
    pub x: u32,
    /// Distance from the top edge.
    pub y: u32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl Rect {
    /// Build a rectangle.
    #[must_use]
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// One pixel past the right edge.
    #[must_use]
    pub const fn right(&self) -> u32 {
        self.x.saturating_add(self.w)
    }

    /// One pixel past the bottom edge.
    #[must_use]
    pub const fn bottom(&self) -> u32 {
        self.y.saturating_add(self.h)
    }

    /// True when the rectangle covers no pixels.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// The overlap with another rectangle, or `None` when they do not touch.
    ///
    /// This is what makes an out-of-bounds fill harmless rather than an error: the
    /// frame clips what it is asked to draw.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            return None;
        }
        Some(Self::new(x, y, right - x, bottom - y))
    }

    /// True when the point is inside the rectangle.
    ///
    /// This is what a pointer press is hit-tested against, so "inside" follows the same
    /// half-open convention as [`Self::intersect`]: the right and bottom edges are not
    /// included.
    #[must_use]
    pub const fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// A painted frame: ARGB pixels, row-major, top left first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl Frame {
    /// An unpainted frame, all zeroes.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let count = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![0; count],
        }
    }

    /// The frame's width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The frame's height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The whole frame as the rectangle that covers it.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// The raw pixels, ARGB.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Cover the whole frame in one colour.
    pub fn clear(&mut self, colour: Colour) {
        self.pixels.fill(colour.to_argb());
    }

    /// Paint a rectangle, clipped to the frame.
    ///
    /// An out-of-bounds or empty rectangle is a no-op, so a caller cannot fail by asking
    /// for something off-screen: the frame clips what it is asked to draw. There is no
    /// error case to report, which is why this returns nothing rather than a `Result`.
    pub fn fill_rect(&mut self, rect: Rect, colour: Colour) {
        let Some(clipped) = rect.intersect(&self.bounds()) else {
            return;
        };
        let value = colour.to_argb();
        let stride = self.width as usize;
        for y in clipped.y..clipped.bottom() {
            let start = (y as usize) * stride + (clipped.x as usize);
            let end = start + (clipped.w as usize);
            if let Some(row) = self.pixels.get_mut(start..end) {
                row.fill(value);
            }
        }
    }

    /// The colour at a pixel, or `None` when it is outside the frame.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<Colour> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize) * (self.width as usize) + (x as usize);
        self.pixels.get(index).copied().map(Colour)
    }

    /// How many different colours the frame actually contains.
    ///
    /// This is the measurement the frame check is built on, and it reads the whole frame on
    /// purpose: a sample would let a mostly-blank screen pass.
    #[must_use]
    pub fn distinct_colours(&self) -> usize {
        self.pixels.iter().copied().collect::<BTreeSet<u32>>().len()
    }

    /// The frame as a binary PPM (P6), so it can be looked at without an image crate.
    #[must_use]
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3 + 32);
        out.extend_from_slice(format!("P6\n{} {}\n255\n", self.width, self.height).as_bytes());
        for pixel in &self.pixels {
            let (_, r, g, b) = Colour(*pixel).channels();
            out.extend_from_slice(&[r, g, b]);
        }
        out
    }
}

/// The shell's view of one session: the authoritative [`Session`] model, plus the two
/// things only the shell knows — where it is drawn, and the text it has accumulated.
///
/// The session's identity, label, feel and openables all live on `model` (in `flow-session`);
/// this struct adds nothing to what a session *is*. That is what "the shell is one consumer of
/// the session model" means in code: the shell lays out and paints the model, it does not
/// redefine it. There is no second `Session` here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionView {
    /// The session model: identity, label, feel, and what it is opening.
    pub model: Session,
    /// Where it is drawn. Laid out by the shell, not by the session.
    pub surface: Rect,
    /// The text the session has accumulated. Private: input reaches it only through
    /// [`Shell::handle`], so no caller can paint text the shell did not route.
    terminal: Terminal,
}

impl SessionView {
    /// The session's terminal buffer, read-only.
    #[must_use]
    pub const fn terminal(&self) -> &Terminal {
        &self.terminal
    }
}

/// The shell: the sessions that exist, which one has focus, and how they are laid out.
#[derive(Debug, Clone)]
pub struct Shell {
    width: u32,
    height: u32,
    sessions: Vec<SessionView>,
    focused: Option<SessionId>,
    next_id: u64,
}

impl Shell {
    /// A shell with no sessions, laid out for the given frame size.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            sessions: Vec::new(),
            focused: None,
            next_id: 1,
        }
    }

    /// The frame size this shell lays out for.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The sessions, in layout order, each as the model plus the shell's presentation.
    #[must_use]
    pub fn sessions(&self) -> &[SessionView] {
        &self.sessions
    }

    /// Which session has focus, if any.
    #[must_use]
    pub const fn focused(&self) -> Option<SessionId> {
        self.focused
    }

    /// Add a session and give it focus. Returns its id.
    ///
    /// The shell has not been told what the session opens, so the model takes the default
    /// feel ([`Feel::default`], ADR 0013 Decision 2). A session described by what it opens is
    /// [`Shell::add_session_opening`], whose feel is inferred.
    pub fn add_session(&mut self, title: &str) -> SessionId {
        let model = Session::new(SessionId(self.next_id), title);
        self.next_id += 1;
        self.push_session(model)
    }

    /// Add a session described by what it opens, and give it focus. Returns its id.
    ///
    /// The feel is inferred from the openables (ADR 0013 Decision 7) rather than left at the
    /// default: a lone legacy app becomes Classic, a task with tools Session + shelf, nothing
    /// app-shaped Flow. This is the path the shell takes once it knows what a session opens.
    pub fn add_session_opening(&mut self, title: &str, openables: Vec<Openable>) -> SessionId {
        let model = Session::opening(SessionId(self.next_id), title, openables);
        self.next_id += 1;
        self.push_session(model)
    }

    /// Lay a fully-built session model out, give it focus, and return its id.
    fn push_session(&mut self, model: Session) -> SessionId {
        let id = model.id();
        self.sessions.push(SessionView {
            model,
            surface: Rect::new(0, 0, 0, 0),
            terminal: Terminal::new(),
        });
        self.focused = Some(id);
        self.relayout();
        id
    }

    /// Forget a session. Returns whether it existed.
    ///
    /// Focus moves to the last remaining session, because a shell with sessions and no
    /// focus has nothing to show, and that is a state a person can reach by closing what
    /// they were using.
    pub fn close_session(&mut self, id: SessionId) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.model.id() != id);
        if self.sessions.len() == before {
            return false;
        }
        if self.focused == Some(id) {
            self.focused = self.sessions.last().map(|s| s.model.id());
        }
        self.relayout();
        true
    }

    /// Give a session focus. Returns whether it exists.
    pub fn focus(&mut self, id: SessionId) -> bool {
        if self.sessions.iter().any(|s| s.model.id() == id) {
            self.focused = Some(id);
            true
        } else {
            false
        }
    }

    /// Route one input event to the session it lands on.
    ///
    /// Returns whether the event reached a session. That is the *routing* result, not the
    /// proof — what the event changed is asserted on the painted frame in the tests, so a
    /// method that returns `true` while drawing nothing cannot pass. An event with no
    /// session to receive it returns `false` and changes nothing.
    pub fn handle(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Pointer(pointer) => self.handle_pointer(pointer),
        }
    }

    /// A key goes to the focused session, or to focus itself for Tab.
    fn handle_key(&mut self, key: Key) -> bool {
        match key {
            Key::Tab => self.focus_next(),
            Key::Char(c) => self.with_focused(|s| s.terminal.insert(c)),
            Key::Backspace => self.with_focused(|s| s.terminal.backspace()),
            Key::Enter => self.with_focused(|s| s.terminal.newline()),
        }
    }

    /// A press gives focus to the session whose surface contains it.
    fn handle_pointer(&mut self, pointer: Pointer) -> bool {
        match pointer {
            Pointer::Press { x, y } => {
                if let Some(view) = self.sessions.iter().find(|s| s.surface.contains(x, y)) {
                    self.focused = Some(view.model.id());
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Move focus to the next session, wrapping. Returns whether a session got focus.
    fn focus_next(&mut self) -> bool {
        if self.sessions.is_empty() {
            return false;
        }
        let index = self
            .focused
            .and_then(|id| self.sessions.iter().position(|s| s.model.id() == id))
            .map_or(0, |i| (i + 1) % self.sessions.len());
        self.focused = Some(self.sessions[index].model.id());
        true
    }

    /// Apply a change to the focused session's terminal, if there is one.
    ///
    /// `false` when nothing is focused — the honest "there was no terminal to give this
    /// to", which the negative control in the tests asserts must not fake a pass.
    fn with_focused(&mut self, change: impl FnOnce(&mut SessionView)) -> bool {
        let Some(id) = self.focused else {
            return false;
        };
        let Some(view) = self.sessions.iter_mut().find(|s| s.model.id() == id) else {
            return false;
        };
        change(view);
        true
    }

    /// The area the session surfaces share: inside the margin, below the bar.
    #[must_use]
    pub fn content_area(&self) -> Rect {
        let top = BAR_HEIGHT.saturating_add(MARGIN);
        Rect::new(
            MARGIN,
            top,
            self.width.saturating_sub(MARGIN * 2),
            self.height.saturating_sub(top).saturating_sub(MARGIN),
        )
    }

    /// Stack the sessions vertically, with a margin between them.
    ///
    /// Deliberately the simplest layout that gives every session a real surface. The
    /// shapes a session can take — and whether it can be moved — are later records.
    fn relayout(&mut self) {
        let area = self.content_area();
        let count = self.sessions.len() as u32;
        if count == 0 {
            return;
        }
        let gap = MARGIN;
        let total_gap = gap.saturating_mul(count.saturating_sub(1));
        let each = area.h.saturating_sub(total_gap) / count;
        for (index, view) in self.sessions.iter_mut().enumerate() {
            let index = index as u32;
            view.surface = Rect::new(area.x, area.y + index * (each + gap), area.w, each);
        }
    }

    /// Paint the shell.
    ///
    /// The fills are clipped, so the only way one fails is a bug in this file rather than
    /// a bad request; a test asserts a fill really does change pixels, so an ignored
    /// error cannot hide.
    #[must_use]
    pub fn paint(&self) -> Frame {
        let mut frame = Frame::new(self.width, self.height);
        frame.clear(palette::BACKGROUND);
        frame.fill_rect(Rect::new(0, 0, self.width, BAR_HEIGHT), palette::BAR);
        if self.focused.is_some() {
            frame.fill_rect(
                Rect::new(MARGIN, 8, FOCUS_MARK_WIDTH, BAR_HEIGHT - 16),
                palette::ACCENT,
            );
        }
        for view in &self.sessions {
            let focused = self.focused == Some(view.model.id());
            let colour = if focused {
                palette::SURFACE_FOCUSED
            } else {
                palette::SURFACE
            };
            frame.fill_rect(view.surface, colour);
            terminal::paint(&mut frame, &view.terminal, view.surface, focused);
        }
        frame
    }
}

/// Whether a frame is evidence: how many colours it holds, and whether that is enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameEvidence {
    /// How many distinct colours the frame contains.
    pub distinct_colours: usize,
    /// How many pixels were looked at.
    pub pixels: usize,
}

impl FrameEvidence {
    /// Whether the frame is evidence that something was drawn.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.distinct_colours > 1
    }

    /// A sentence a human or a log can read, saying what was measured.
    #[must_use]
    pub fn verdict(&self) -> String {
        if self.passed() {
            format!(
                "PASS: {} distinct colours across {} pixels - the frame has content",
                self.distinct_colours, self.pixels
            )
        } else {
            format!(
                "FAIL: {} distinct colour(s) across {} pixels - a blank screen is not a rendered shell",
                self.distinct_colours, self.pixels
            )
        }
    }
}

/// Whether a frame is evidence that something was drawn on it.
///
/// One colour is a blank screen with extra steps, and "the unit is active" / "the file
/// exists" / "exit code 0" are the same mistake in different clothes (ADR 0010 asks for a
/// frame, not a status). The first version asserts the weakest useful thing — *more than
/// one colour* — because a check nobody can pass teaches everyone to ignore the colour.
#[must_use]
pub fn frame_evidence(frame: &Frame) -> FrameEvidence {
    FrameEvidence {
        distinct_colours: frame.distinct_colours(),
        pixels: frame.pixels().len(),
    }
}

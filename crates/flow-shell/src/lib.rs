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
//! owns, where the session is drawn and the text it has accumulated.
//!
//! The per-feel slices of ADR 0013 are here. When the focused session's feel is
//! [`Feel::SessionShelf`], [`Shell::paint`] draws that feel's two pieces of chrome — a
//! **shelf** listing the focused session's [`Openable`]s (a thin strip of its capsules, not
//! an app grid) and an **intent bar** that is a search field, not an intent engine. A
//! character typed while the intent bar is the input target builds a query; Enter commits
//! it through [`match_query`], a deterministic case-insensitive match over session labels
//! and openable names that moves focus to the first match — or, with none, paints "no
//! match" rather than inventing a result. When the focused session's feel is
//! [`Feel::Flow`], [`Shell::paint`] lays the sessions out as **tiles** — large, labelled by
//! the session's name in large type, no shelf and no app grid — and a press on a tile
//! opens it by making it the hero (focus, space, and the focus border). Classic's windows
//! remain the later slice, and Classic keeps the one frame this crate already paints.
//!
//! Still absent: multi-display, capsules, the Device Broker, a real Wayland client, and any
//! pty. The terminal is a text buffer with a cursor, not a shell behind a tty — it is
//! honest as long as it is called that.
//!
//! [ADR 0010]: ../../docs/adr/0010-flow-shell-first.md
//! [ADR 0011]: ../../docs/adr/0011-the-humans-mark.md

use std::collections::BTreeSet;

mod font;
mod input;
mod png;
mod shelf;
mod terminal;
mod tiles;

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
/// Height of the intent bar — the Session + shelf feel's search field, under the top bar.
pub const INTENT_BAR_HEIGHT: u32 = 28;
/// Height of the shelf — the thin strip of the focused session's openables at the bottom.
pub const SHELF_HEIGHT: u32 = 40;
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
/// How much larger than the terminal font the Flow feel's tile label is drawn: each
/// font pixel of the 5×7 glyph is scaled to this many pixels. This is the feel's
/// "large type" (ADR 0013), and it is asserted — not just documented — by the Flow tests.
pub const LARGE_TYPE_SCALE: u32 = 3;
/// Width of the focus chrome border drawn around the focused Flow tile — the mark that
/// says "this tile has been opened".
pub const FOCUS_BORDER: u32 = 3;

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
    /// What is drawn on the bar (the Wi-Fi button's bars). Its own colour, not TEXT: tests
    /// find the pointer by TEXT, and a pointer that seemed drawn before the mouse moved
    /// failed `test-shell.sh` when the icon used it.
    pub const BAR_ICON: Colour = Colour::rgb(0x9a, 0xa4, 0xb8);
    /// A session surface that does not have focus.
    pub const SURFACE: Colour = Colour::rgb(0x22, 0x27, 0x30);
    /// A session surface that does.
    pub const SURFACE_FOCUSED: Colour = Colour::rgb(0x2d, 0x34, 0x40);
    /// The focus indicator and the cursor — the bright things that mark where the person is.
    pub const ACCENT: Colour = Colour::rgb(0x5a, 0xa8, 0xff);
    /// The terminal's text.
    pub const TEXT: Colour = Colour::rgb(0xd8, 0xde, 0xe9);
    /// The intent bar (the search field) background.
    pub const INTENT_BAR: Colour = Colour::rgb(0x1f, 0x24, 0x2e);
    /// The shelf that lists the focused session's openables.
    pub const SHELF: Colour = Colour::rgb(0x15, 0x19, 0x1f);
    /// A query that matched nothing, so the frame says so rather than faking a result.
    pub const NO_MATCH: Colour = Colour::rgb(0xe0, 0x5f, 0x5f);
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

    /// A frame from pixels painted elsewhere: ARGB, row-major, top left first. `None` when
    /// the count does not match the size.
    ///
    /// The compositor (`crates/flow-compositor`) renders with Smithay and hands its output
    /// here, so its frames go through the same PNG writer and evidence check as this crate's.
    #[must_use]
    pub fn from_pixels(width: u32, height: u32, pixels: Vec<u32>) -> Option<Self> {
        let count = (width as usize).checked_mul(height as usize)?;
        (pixels.len() == count).then_some(Self {
            width,
            height,
            pixels,
        })
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

    /// Draw one line of text with the shell's bitmap font, each font pixel `scale` pixels
    /// square, clipped to the frame. For the shell's own clients (the Wi-Fi picker), so
    /// they draw text the way the shell does without a font engine.
    pub fn text(&mut self, text: &str, x: u32, y: u32, scale: u32, colour: Colour) {
        let scale = scale.max(1);
        for (i, ch) in text.chars().enumerate() {
            let cx = x.saturating_add(i as u32 * GLYPH_ADVANCE * scale);
            for (col, byte) in font::glyph(ch).iter().enumerate() {
                for row in 0..GLYPH_H {
                    if byte & (1 << row) != 0 {
                        let px = cx.saturating_add(col as u32 * scale);
                        let py = y.saturating_add(row * scale);
                        self.fill_rect(Rect::new(px, py, scale, scale), colour);
                    }
                }
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
    /// The text in the intent bar — the Session + shelf feel's search field.
    intent_query: String,
    /// Whether the intent bar, rather than the focused terminal, receives keys.
    intent_active: bool,
    /// Whether the last committed query matched nothing.
    intent_no_match: bool,
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
            intent_query: String::new(),
            intent_active: false,
            intent_no_match: false,
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

    /// The text in the intent bar — the Session + shelf feel's search field.
    ///
    /// This is the query the person has typed, not a result: matching it against what
    /// exists is [`match_query`], which runs only when the query is committed.
    #[must_use]
    pub fn intent_query(&self) -> &str {
        &self.intent_query
    }

    /// Whether the intent bar, rather than the focused terminal, is receiving keys.
    ///
    /// Two input targets, one model of them: a press inside the intent bar makes it the
    /// target (so characters build the query), a press inside a session surface returns
    /// keys to that session's terminal, and a match or Tab hands them back too.
    #[must_use]
    pub const fn is_intent_active(&self) -> bool {
        self.intent_active
    }

    /// Whether the last committed query matched nothing.
    #[must_use]
    pub const fn intent_no_match(&self) -> bool {
        self.intent_no_match
    }

    /// The intent bar's rectangle — the search field under the top bar.
    ///
    /// Empty when the focused session's feel is not [`Feel::SessionShelf`]: the field is
    /// that feel's chrome, not a global fixture (ADR 0013: the feel is per-session).
    #[must_use]
    pub fn intent_bar_rect(&self) -> Rect {
        if !self.session_shelf_chrome() {
            return Rect::new(0, 0, 0, 0);
        }
        Rect::new(0, BAR_HEIGHT, self.width, INTENT_BAR_HEIGHT)
    }

    /// The shelf's rectangle — the thin strip of the focused session's openables.
    ///
    /// Empty for the other feels, like [`Self::intent_bar_rect`].
    #[must_use]
    pub fn shelf_rect(&self) -> Rect {
        if !self.session_shelf_chrome() {
            return Rect::new(0, 0, 0, 0);
        }
        Rect::new(
            0,
            self.height.saturating_sub(SHELF_HEIGHT),
            self.width,
            SHELF_HEIGHT,
        )
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
    ///
    /// Focus decides the feel on screen (the focused session's feel is the chrome), so the
    /// layout is recomputed when it changes.
    pub fn focus(&mut self, id: SessionId) -> bool {
        if self.sessions.iter().any(|s| s.model.id() == id) {
            self.focused = Some(id);
            self.relayout();
            true
        } else {
            false
        }
    }

    /// Route one input event to where it lands: a session, or the intent bar.
    ///
    /// Returns whether the event was consumed. That is the *routing* result, not the
    /// proof — what the event changed is asserted on the painted frame in the tests, so a
    /// method that returns `true` while drawing nothing cannot pass. An event with no
    /// session or intent bar to receive it returns `false` and changes nothing.
    pub fn handle(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Pointer(pointer) => self.handle_pointer(pointer),
        }
    }

    /// A key goes to the intent bar when it is the input target, else to the focused
    /// session — or to focus itself for Tab.
    fn handle_key(&mut self, key: Key) -> bool {
        if self.intent_active {
            return self.handle_intent_key(key);
        }
        match key {
            Key::Tab => self.focus_next(),
            Key::Char(c) => self.with_focused(|s| s.terminal.insert(c)),
            Key::Backspace => self.with_focused(|s| s.terminal.backspace()),
            Key::Enter => self.with_focused(|s| s.terminal.newline()),
        }
    }

    /// A key goes to the intent bar: characters build the query, Enter commits it.
    fn handle_intent_key(&mut self, key: Key) -> bool {
        match key {
            Key::Char(c) => {
                self.intent_query.push(c);
                // A new character makes the previous "no match" verdict stale.
                self.intent_no_match = false;
                true
            }
            Key::Backspace => {
                self.intent_query.pop();
                self.intent_no_match = false;
                true
            }
            Key::Enter => match match_query(&self.intent_query, &self.sessions) {
                Some(id) => {
                    // The query returns a result: focus moves to it, and the intent bar
                    // hands the keys back to that session's terminal.
                    self.focused = Some(id);
                    self.intent_no_match = false;
                    self.intent_active = false;
                    self.relayout();
                    true
                }
                None => {
                    // No match is a fact to show, never a result to invent.
                    self.intent_no_match = true;
                    true
                }
            },
            Key::Tab => {
                // Leave the search field for the focused session's terminal.
                self.intent_active = false;
                self.intent_no_match = false;
                true
            }
        }
    }

    /// A press gives focus to the session whose surface contains it, or to the intent bar.
    fn handle_pointer(&mut self, pointer: Pointer) -> bool {
        match pointer {
            Pointer::Press { x, y } => {
                if self.intent_bar_rect().contains(x, y) {
                    self.intent_active = true;
                    return true;
                }
                if let Some(view) = self.sessions.iter().find(|s| s.surface.contains(x, y)) {
                    self.focused = Some(view.model.id());
                    self.intent_active = false;
                    self.relayout();
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
        self.relayout();
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

    /// The feel the focused session carries, if there is one.
    ///
    /// This is what decides the chrome on screen: the focused session's feel is the feel
    /// being presented (ADR 0013), not a global switch.
    fn focused_feel(&self) -> Option<Feel> {
        self.focused.and_then(|id| {
            self.sessions
                .iter()
                .find(|s| s.model.id() == id)
                .map(|s| s.model.feel())
        })
    }

    /// Whether the frame shows the Session + shelf chrome: intent bar above, shelf below.
    fn session_shelf_chrome(&self) -> bool {
        self.focused_feel() == Some(Feel::SessionShelf)
    }

    /// Whether the frame shows the Flow chrome: sessions as tiles, no shelf or intent bar.
    fn flow_chrome(&self) -> bool {
        self.focused_feel() == Some(Feel::Flow)
    }

    /// The area the session surfaces share: inside the margin, below the bar — and, for
    /// the Session + shelf feel, below the intent bar and above the shelf.
    #[must_use]
    pub fn content_area(&self) -> Rect {
        let shelf_chrome = self.session_shelf_chrome();
        let top = if shelf_chrome {
            BAR_HEIGHT
                .saturating_add(INTENT_BAR_HEIGHT)
                .saturating_add(MARGIN)
        } else {
            BAR_HEIGHT.saturating_add(MARGIN)
        };
        let bottom = if shelf_chrome {
            SHELF_HEIGHT.saturating_add(MARGIN)
        } else {
            MARGIN
        };
        Rect::new(
            MARGIN,
            top,
            self.width.saturating_sub(MARGIN * 2),
            self.height.saturating_sub(top).saturating_sub(bottom),
        )
    }

    /// Lay the sessions out for the feel being presented: tiles for Flow, the vertical
    /// stack for Session + shelf and Classic.
    fn relayout(&mut self) {
        if self.flow_chrome() {
            self.relayout_flow();
        } else {
            self.relayout_stack();
        }
    }

    /// The Flow feel lays sessions out as tiles, the focused one as the hero (opened).
    fn relayout_flow(&mut self) {
        let area = self.content_area();
        let count = self.sessions.len();
        let focused_index = self
            .focused
            .and_then(|id| self.sessions.iter().position(|s| s.model.id() == id));
        let rects = tiles::tile_layout(area, count, focused_index);
        for (view, rect) in self.sessions.iter_mut().zip(rects) {
            view.surface = rect;
        }
    }

    /// Stack the sessions vertically, with a margin between them.
    ///
    /// Deliberately the simplest layout that gives every session a real surface. The
    /// shapes a session can take — and whether it can be moved — are later records.
    fn relayout_stack(&mut self) {
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
        if self.session_shelf_chrome() {
            shelf::paint_intent_bar(
                &mut frame,
                self.intent_bar_rect(),
                &self.intent_query,
                self.intent_active,
                self.intent_no_match,
            );
        }
        for view in &self.sessions {
            let focused = self.focused == Some(view.model.id());
            if self.flow_chrome() {
                tiles::paint_tile(
                    &mut frame,
                    view.model.label(),
                    view.model.openables(),
                    view.surface,
                    focused,
                );
            } else {
                let colour = if focused {
                    palette::SURFACE_FOCUSED
                } else {
                    palette::SURFACE
                };
                frame.fill_rect(view.surface, colour);
                terminal::paint(&mut frame, &view.terminal, view.surface, focused);
            }
        }
        if self.session_shelf_chrome() {
            let openables = self.focused.and_then(|id| {
                self.sessions
                    .iter()
                    .find(|s| s.model.id() == id)
                    .map(|s| s.model.openables())
            });
            shelf::paint_shelf(&mut frame, self.shelf_rect(), openables);
        }
        frame
    }
}

/// The deterministic match the intent bar commits a query through.
///
/// ADR 0013's "a query returns a result" is this and nothing more: a case-insensitive
/// substring match over session labels and openable names, first session in layout order
/// whose label or one of whose openable labels contains the query. A blank query matches
/// nothing, and a query no session matches returns `None` — the shell paints "no match"
/// rather than inventing one. There is deliberately no scoring, ranking, or model behind
/// it: the same honest, deterministic rule as `flow_session::infer_feel`.
#[must_use]
pub fn match_query(query: &str, sessions: &[SessionView]) -> Option<SessionId> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }
    sessions.iter().find_map(|view| {
        let label_matches = view.model.label().to_lowercase().contains(&needle);
        let openable_matches = view
            .model
            .openables()
            .iter()
            .any(|o| o.label.to_lowercase().contains(&needle));
        (label_matches || openable_matches).then_some(view.model.id())
    })
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

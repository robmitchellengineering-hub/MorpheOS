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
//! **What is deliberately absent:** the Intent Bar, multi-display, capsules, the Device
//! Broker, input, and any actual Wayland client. The first slice is *a frame, not a
//! framework*. A terminal does not appear in this frame yet — its surface is a rectangle
//! standing in for one, which is honest as long as it is called that.
//!
//! [ADR 0010]: ../../docs/adr/0010-flow-shell-first.md
//! [ADR 0011]: ../../docs/adr/0011-the-humans-mark.md

use std::collections::BTreeSet;

mod png;

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
    /// The focus indicator, and the only bright thing in the frame.
    pub const ACCENT: Colour = Colour::rgb(0x5a, 0xa8, 0xff);
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

/// What a session is called, for as long as the shell is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// One session: an intent, and the surface it is being shown on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Stable for the life of the session.
    pub id: SessionId,
    /// What the session is for, as the bar shows it.
    pub title: String,
    /// Where it is drawn. Laid out by the shell, not by the session.
    pub surface: Rect,
}

/// The shell: the sessions that exist, which one has focus, and how they are laid out.
#[derive(Debug, Clone)]
pub struct Shell {
    width: u32,
    height: u32,
    sessions: Vec<Session>,
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

    /// The sessions, in layout order.
    #[must_use]
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// Which session has focus, if any.
    #[must_use]
    pub const fn focused(&self) -> Option<SessionId> {
        self.focused
    }

    /// Add a session and give it focus. Returns its id.
    pub fn add_session(&mut self, title: &str) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id += 1;
        self.sessions.push(Session {
            id,
            title: title.to_owned(),
            surface: Rect::new(0, 0, 0, 0),
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
        self.sessions.retain(|s| s.id != id);
        if self.sessions.len() == before {
            return false;
        }
        if self.focused == Some(id) {
            self.focused = self.sessions.last().map(|s| s.id);
        }
        self.relayout();
        true
    }

    /// Give a session focus. Returns whether it exists.
    pub fn focus(&mut self, id: SessionId) -> bool {
        if self.sessions.iter().any(|s| s.id == id) {
            self.focused = Some(id);
            true
        } else {
            false
        }
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
        for (index, session) in self.sessions.iter_mut().enumerate() {
            let index = index as u32;
            session.surface = Rect::new(area.x, area.y + index * (each + gap), area.w, each);
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
        for session in &self.sessions {
            let colour = if self.focused == Some(session.id) {
                palette::SURFACE_FOCUSED
            } else {
                palette::SURFACE
            };
            frame.fill_rect(session.surface, colour);
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

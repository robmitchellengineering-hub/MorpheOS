//! Window management for the Classic feel (ADR 0013, item 3), as pure logic.
//!
//! WHY THIS IS SEPARATE FROM THE COMPOSITOR. Where windows go, which one is under the
//! pointer, which one has focus, and what happens when one closes are **decisions**. They
//! belong where they can be tested on any machine in microseconds, with no Wayland, no
//! renderer and no Smithay. The compositor (`headless.rs`, later the DRM backend) only
//! carries them out. Extending window management (move, fullscreen, tiling) means extending
//! this module and its tests first.
//!
//! Layout: windows are stacked down the screen in the order they opened, `MARGIN` apart, at
//! their own size, but never larger than the screen ([`fit`]); one that would reach past the
//! bottom starts a new stack at the top, `CASCADE` further in. A window that opens comes to
//! the front. A window a person has moved stays where they put it. The focused window is drawn with a border of `BORDER` pixels.
//!
//! What a person can do with them (E2, then E9 for real input):
//!   * click a window to focus it, and it comes to the front;
//!   * drag one by its edge (the `GRAB` pixels round it), or anywhere on it while holding
//!     Super, or by its own title bar (a client asks to be moved: [`Manager::begin_move`]);
//!   * make the focused one fullscreen and back (Super+F, or the client asks, as Firefox
//!     does on F11): it covers the screen, with no border;
//!   * switch to the next window (Alt+Tab, [`Manager::cycle`]), which comes to the front.
//!
//! Which window is in front is the order windows were last raised; [`Manager::layout`]
//! lists them back to front.

/// Gap between windows and the screen edges, in pixels.
pub const MARGIN: i32 = 16;
/// How far right and down a new stack starts from the last one, when a window would open
/// below the bottom of the screen ([`Manager::layout`]).
pub const CASCADE: i32 = 2 * MARGIN;
/// Width of the focus border drawn around the focused window, in pixels.
pub const BORDER: i32 = 3;
/// The strip round a window, outside it, that a press grabs to move the window: half the
/// gap between windows, so a press there belongs to one window only.
pub const GRAB: i32 = MARGIN / 2;
/// How much of a moved window stays on the screen, in each direction, so a window cannot be
/// dragged out of reach.
pub const KEEP: i32 = 32;
/// The smallest a window may be resized to, in either direction.
pub const MIN_SIZE: i32 = 64;

/// The bar along the bottom of the screen (doc 20, E5): the shell's own, drawn over every
/// window but a fullscreen one. Windows are sized and placed to leave it clear.
pub const BAR_HEIGHT: i32 = 32;
/// The Wi-Fi button at the bar's right-hand end: a square this big, `BUTTON_INSET` from the
/// bar's right edge and centred in its height.
pub const BUTTON: i32 = 24;
pub const BUTTON_INSET: i32 = 8;

/// The bar on a screen of `screen_w` x `screen_h`, as (x, y, w, h).
#[must_use]
pub fn bar(screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    (0, screen_h - BAR_HEIGHT, screen_w, BAR_HEIGHT)
}

/// The Wi-Fi button on that screen, as (x, y, w, h).
#[must_use]
pub fn wifi_button(screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    (
        screen_w - BUTTON_INSET - BUTTON,
        screen_h - BAR_HEIGHT + (BAR_HEIGHT - BUTTON) / 2,
        BUTTON,
        BUTTON,
    )
}

/// The Files button on that screen: the same square, one button's gap to the Wi-Fi button's
/// left, so the file view (E17) opens from the bar the way the picker does (doc 20, E20).
#[must_use]
pub fn files_button(screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let (wx, wy, _, _) = wifi_button(screen_w, screen_h);
    (wx - BUTTON_INSET - BUTTON, wy, BUTTON, BUTTON)
}

/// The largest a window may be on a screen of `screen_w` x `screen_h`: the screen less a
/// margin all round and the bar. A toolkit picks its own window size, and GTK (so Firefox)
/// picks one wider than a small screen; such a window is asked to shrink to this.
#[must_use]
pub fn max_size(screen_w: i32, screen_h: i32) -> (i32, i32) {
    (
        (screen_w - 2 * MARGIN).max(1),
        (screen_h - 2 * MARGIN - BAR_HEIGHT).max(1),
    )
}

/// The size a `w` x `h` window should be asked to take on that screen, if it is too big in
/// either direction; `None` if it fits as it is. Only the direction that overflows shrinks.
#[must_use]
pub fn fit(w: i32, h: i32, screen_w: i32, screen_h: i32) -> Option<(i32, i32)> {
    let (max_w, max_h) = max_size(screen_w, screen_h);
    (w > max_w || h > max_h).then(|| (w.min(max_w), h.min(max_h)))
}

/// A window as the manager sees it: an ID the compositor chose, its size, and which of the
/// maximized/minimized states it is in. The compositor only fills in `id`, `w` and `h`; the
/// states are the manager's own decisions, kept here so a window's place and its states
/// travel together.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub id: u32,
    pub w: i32,
    pub h: i32,
    pub maximized: bool,
    pub minimized: bool,
}

/// Where a window is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Placed {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    /// In the grab strip: within `GRAB` of the window, but not on it.
    fn on_edge(&self, x: i32, y: i32) -> bool {
        let outer = Placed {
            x: self.x - GRAB,
            y: self.y - GRAB,
            w: self.w + 2 * GRAB,
            h: self.h + 2 * GRAB,
            ..*self
        };
        outer.contains(x, y) && !self.contains(x, y)
    }

    /// In a corner's grab region: within `GRAB` of both a horizontal and a vertical edge, but
    /// not on the window. The corner is where the two strips meet.
    fn on_corner(&self, x: i32, y: i32) -> Option<Corner> {
        let left = x >= self.x - GRAB && x < self.x;
        let right = x >= self.x + self.w && x < self.x + self.w + GRAB;
        let top = y >= self.y - GRAB && y < self.y;
        let bottom = y >= self.y + self.h && y < self.y + self.h + GRAB;
        match (left, right, top, bottom) {
            (true, false, true, false) => Some(Corner::TopLeft),
            (false, true, true, false) => Some(Corner::TopRight),
            (true, false, false, true) => Some(Corner::BottomLeft),
            (false, true, false, true) => Some(Corner::BottomRight),
            _ => None,
        }
    }
}

/// A move in progress: which window, and where the pointer holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Drag {
    id: u32,
    dx: i32,
    dy: i32,
}

/// Which corner of a window a resize drags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A resize in progress: which window, which corner follows the pointer, and the opposite
/// corner (the one that stays put).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Resize {
    id: u32,
    corner: Corner,
    /// The fixed corner (opposite the dragged one), in screen coordinates, at the moment the
    /// resize began. It never moves while the drag lasts.
    fixed: (i32, i32),
}

/// The windows, in the order they opened; which one has focus; where moved ones are; which
/// is in front; and which, if any, is fullscreen.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Manager {
    windows: Vec<Window>,
    focused: Option<u32>,
    /// Windows a person moved, and where to.
    moved: Vec<(u32, i32, i32)>,
    /// Raised windows, back to front. A window never raised is behind every raised one.
    raised: Vec<u32>,
    fullscreen: Option<u32>,
    /// The size the fullscreen window had before, to ask it back to afterwards.
    before_fullscreen: Option<(u32, i32, i32)>,
    /// The geometry (x, y, w, h) a window had before it was last maximized, to put it back
    /// to afterwards. Kept until the next maximize, so the compositor can ask it back to the
    /// old size even after the maximize flag has been cleared.
    before_maximized: Option<(u32, i32, i32, i32, i32)>,
    /// The window most recently minimized, so `cycle` can bring one back when every window
    /// on the screen is minimized.
    last_minimized: Option<u32>,
    drag: Option<Drag>,
    resize: Option<Resize>,
    screen: (i32, i32),
}

impl Manager {
    /// Bring the window list in line with what the compositor has: new windows are added at
    /// the end, gone ones are removed, sizes are updated. If the focused window went away,
    /// focus moves to the last window that is left (the most recent), or to none.
    pub fn sync(&mut self, current: &[Window]) {
        self.windows
            .retain(|w| current.iter().any(|c| c.id == w.id));
        let alive = |id: &u32| current.iter().any(|c| c.id == *id);
        self.moved.retain(|(id, _, _)| alive(id));
        self.raised.retain(alive);
        if !self.fullscreen.as_ref().is_some_and(alive) {
            self.fullscreen = None;
        }
        if !self
            .before_maximized
            .as_ref()
            .is_some_and(|(id, ..)| alive(id))
        {
            self.before_maximized = None;
        }
        if !self.last_minimized.as_ref().is_some_and(alive) {
            self.last_minimized = None;
        }
        if !self.drag.as_ref().is_some_and(|d| alive(&d.id)) {
            self.drag = None;
        }
        if !self.resize.as_ref().is_some_and(|r| alive(&r.id)) {
            self.resize = None;
        }
        for c in current {
            match self.windows.iter_mut().find(|w| w.id == c.id) {
                // The compositor only reports id and size; the maximized/minimized states
                // are the manager's, so keep them across the size update.
                Some(w) => {
                    let (maximized, minimized) = (w.maximized, w.minimized);
                    *w = *c;
                    w.maximized = maximized;
                    w.minimized = minimized;
                }
                // A window that opens comes to the front: the Wi-Fi picker opening over
                // Firefox is drawn over it. (Focus still waits for a click, ADR 0013.)
                None => {
                    self.windows.push(*c);
                    self.raise(c.id);
                }
            }
        }
        if self
            .focused
            .is_some_and(|f| !self.windows.iter().any(|w| w.id == f))
        {
            self.focused = self.windows.last().map(|w| w.id);
        }
    }

    /// The screen's size: what fullscreen covers and what a moved window is kept on.
    pub fn set_screen(&mut self, w: i32, h: i32) {
        self.screen = (w, h);
    }

    /// Where every window goes, back to front: the last one is in front. A minimized window
    /// is not drawn, so it is not placed; a maximized one fills the screen less the bar.
    #[must_use]
    pub fn layout(&self) -> Vec<Placed> {
        let (mut x, mut y) = (MARGIN, MARGIN);
        let mut placed: Vec<Placed> = self
            .windows
            .iter()
            .filter(|w| !w.minimized)
            .map(|w| {
                // A window that would reach past the bottom of the screen starts a new stack
                // at the top, CASCADE right and down from the last one: under a window that
                // fills the screen (Firefox) it would otherwise open where nobody can see it.
                // Not the first of a stack: a window taller than the screen is asked to fit.
                let bottom = self.screen.1 - BAR_HEIGHT;
                if bottom > 0 && y > x && y + w.h > bottom {
                    x += CASCADE;
                    y = x;
                }
                let mut p = Placed {
                    id: w.id,
                    x,
                    y,
                    w: w.w,
                    h: w.h,
                };
                // Unmoved windows stack down the screen; a moved one leaves no gap.
                match self.moved.iter().find(|(id, _, _)| *id == w.id) {
                    Some(&(_, mx, my)) => (p.x, p.y) = (mx, my),
                    None => y += w.h + MARGIN,
                }
                if self.fullscreen == Some(w.id) {
                    (p.x, p.y, p.w, p.h) = (0, 0, self.screen.0, self.screen.1);
                } else if w.maximized {
                    // The screen less the bar, at the top: the bar stays visible.
                    (p.x, p.y, p.w, p.h) =
                        (0, 0, self.screen.0, (self.screen.1 - BAR_HEIGHT).max(1));
                }
                p
            })
            .collect();
        // Stable: windows never raised keep their opening order, behind the raised ones.
        placed.sort_by_key(|p| {
            self.raised
                .iter()
                .position(|r| *r == p.id)
                .map_or(0, |i| i + 1)
        });
        placed
    }

    /// Whether a point is on the bar's Wi-Fi button. The bar is not there while a window is
    /// fullscreen, so neither is the button.
    #[must_use]
    pub fn on_wifi_button(&self, x: i32, y: i32) -> bool {
        let (bx, by, bw, bh) = wifi_button(self.screen.0, self.screen.1);
        self.fullscreen.is_none()
            && self.screen.1 > 0
            && (bx..bx + bw).contains(&x)
            && (by..by + bh).contains(&y)
    }

    /// Whether a point is on the bar's Files button. The bar is not there while a window is
    /// fullscreen, so neither is the button.
    #[must_use]
    pub fn on_files_button(&self, x: i32, y: i32) -> bool {
        let (bx, by, bw, bh) = files_button(self.screen.0, self.screen.1);
        self.fullscreen.is_none()
            && self.screen.1 > 0
            && (bx..bx + bw).contains(&x)
            && (by..by + bh).contains(&y)
    }

    /// Bring a window to the front and give it focus, as when the shell opens it for a person.
    pub fn activate(&mut self, id: u32) {
        if self.windows.iter().any(|w| w.id == id) {
            self.raise(id);
            self.focused = Some(id);
        }
    }

    /// Bring a window to the front.
    fn raise(&mut self, id: u32) {
        self.raised.retain(|r| *r != id);
        self.raised.push(id);
    }

    /// The window under a point, if any.
    #[must_use]
    pub fn hit(&self, x: i32, y: i32) -> Option<u32> {
        self.layout()
            .into_iter()
            .rev()
            .find(|p| p.contains(x, y))
            .map(|p| p.id)
    }

    /// A click: focus whatever is under it. A click on empty space keeps focus where it is,
    /// as every desktop people know does. Returns the window now focused.
    pub fn click(&mut self, x: i32, y: i32) -> Option<u32> {
        if let Some(id) = self.hit(x, y) {
            self.focused = Some(id);
            self.raise(id);
        }
        self.focused
    }

    /// Focus the next window in opening order, wrapping round (the switch shortcut), and
    /// bring it to the front. A minimized window is not on the screen, so it is skipped; when
    /// every window is minimized, the most-recently-minimized one comes back instead, so a
    /// window is never lost (there is no taskbar to find it again).
    pub fn cycle(&mut self) -> Option<u32> {
        let visible: Vec<u32> = self
            .windows
            .iter()
            .filter(|w| !w.minimized)
            .map(|w| w.id)
            .collect();
        if visible.is_empty() {
            // Bring back the most-recently-minimized window that is still there.
            let restore = self
                .last_minimized
                .filter(|id| self.windows.iter().any(|w| w.id == *id && w.minimized))
                .or_else(|| {
                    self.windows
                        .iter()
                        .rev()
                        .find(|w| w.minimized)
                        .map(|w| w.id)
                });
            match restore {
                Some(id) => {
                    self.set_minimized(id, false);
                    self.focused = Some(id);
                    self.raise(id);
                }
                None => self.focused = None,
            }
            return self.focused;
        }
        let next = match self.focused {
            None => visible.first().copied(),
            Some(f) => {
                let i = visible.iter().position(|id| *id == f).unwrap_or(0);
                visible.get((i + 1) % visible.len()).copied()
            }
        };
        self.focused = next;
        if let Some(id) = self.focused {
            self.raise(id);
        }
        self.focused
    }

    /// A press that may start a move: on a window's edge, or anywhere on it when `anywhere`
    /// (Super is held). Returns the window it picked up, focused and in front; `None` leaves
    /// the press to be a click. A fullscreen or maximized window does not move.
    pub fn begin_drag(&mut self, x: i32, y: i32, anywhere: bool) -> Option<u32> {
        let layout = self.layout();
        // Front to back, so the press belongs to what is drawn on top. The window itself
        // stops the search: its edge strip is under it, not beside it.
        let id = layout.iter().rev().find_map(|p| {
            if p.contains(x, y) {
                Some(anywhere.then_some(p.id))
            } else if p.on_edge(x, y) {
                Some(Some(p.id))
            } else {
                None
            }
        })??;
        self.begin_move(id, x, y)
    }

    /// Start moving a window from the pointer at `(x, y)`: a drag that began on its edge, or
    /// a client asking to be moved (a press on its own title bar).
    pub fn begin_move(&mut self, id: u32, x: i32, y: i32) -> Option<u32> {
        if self.fullscreen == Some(id) || self.is_maximized(id) {
            return None;
        }
        let p = self.layout().into_iter().find(|p| p.id == id)?;
        self.drag = Some(Drag {
            id,
            dx: x - p.x,
            dy: y - p.y,
        });
        self.focused = Some(id);
        self.raise(id);
        Some(id)
    }

    /// The pointer moved during a move: the window follows, kept `KEEP` pixels on screen
    /// (its top never above the screen, so it can always be grabbed again).
    pub fn drag_to(&mut self, x: i32, y: i32) {
        let Some(d) = self.drag else { return };
        let Some(w) = self.windows.iter().find(|w| w.id == d.id) else {
            return;
        };
        let (sw, sh) = self.screen;
        let nx = (x - d.dx).clamp(KEEP - w.w, (sw - KEEP).max(KEEP - w.w));
        let ny = (y - d.dy).clamp(0, (sh - KEEP).max(0));
        self.moved.retain(|(id, _, _)| *id != d.id);
        self.moved.push((d.id, nx, ny));
    }

    /// The button came up: the move is over.
    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The window and corner under a point, if it is in a corner's grab region. Front to back,
    /// so the corner belongs to what is drawn on top.
    #[must_use]
    pub fn corner_at(&self, x: i32, y: i32) -> Option<(u32, Corner)> {
        self.layout()
            .iter()
            .rev()
            .find_map(|p| p.on_corner(x, y).map(|c| (p.id, c)))
    }

    /// Start resizing a window from its `corner`: the opposite corner stays put. A fullscreen
    /// or maximized window does not resize. Returns whether a resize began.
    pub fn begin_resize(&mut self, id: u32, corner: Corner) -> bool {
        if self.fullscreen == Some(id) || self.is_maximized(id) {
            return false;
        }
        let Some(p) = self.layout().into_iter().find(|p| p.id == id) else {
            return false;
        };
        let fixed = match corner {
            Corner::BottomRight => (p.x, p.y),
            Corner::BottomLeft => (p.x + p.w, p.y),
            Corner::TopRight => (p.x, p.y + p.h),
            Corner::TopLeft => (p.x + p.w, p.y + p.h),
        };
        self.resize = Some(Resize { id, corner, fixed });
        self.focused = Some(id);
        self.raise(id);
        true
    }

    /// The pointer moved during a resize: the dragged corner follows it, the opposite corner
    /// stays, and the size is clamped to at least `MIN_SIZE` and at most the screen's
    /// `max_size`. Returns the new `(x, y, w, h)` to tell the client, or `None` if no resize
    /// is in progress.
    #[must_use]
    pub fn resize_to(&mut self, x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
        let r = self.resize?;
        let (fx, fy) = r.fixed;
        let (max_w, max_h) = max_size(self.screen.0, self.screen.1);
        let clamp = |s: i32, max: i32| s.clamp(MIN_SIZE, max.max(MIN_SIZE));
        // The dragged corner follows the pointer; the fixed edge is the opposite corner's.
        let (nx, nw) = match r.corner {
            Corner::TopRight | Corner::BottomRight => {
                let nw = clamp(x - fx, max_w);
                (fx, nw)
            }
            Corner::TopLeft | Corner::BottomLeft => {
                let nw = clamp(fx - x, max_w);
                (fx - nw, nw)
            }
        };
        let (ny, nh) = match r.corner {
            Corner::BottomLeft | Corner::BottomRight => {
                let nh = clamp(y - fy, max_h);
                (fy, nh)
            }
            Corner::TopLeft | Corner::TopRight => {
                let nh = clamp(fy - y, max_h);
                (fy - nh, nh)
            }
        };
        self.moved.retain(|(id, _, _)| *id != r.id);
        self.moved.push((r.id, nx, ny));
        Some((nx, ny, nw, nh))
    }

    /// The button came up: the resize is over.
    pub fn end_resize(&mut self) {
        self.resize = None;
    }

    #[must_use]
    pub fn resizing(&self) -> bool {
        self.resize.is_some()
    }

    /// The window being resized, if any.
    #[must_use]
    pub fn resizing_id(&self) -> Option<u32> {
        self.resize.map(|r| r.id)
    }

    /// Make a window fullscreen, or not. Returns whether anything changed (so the client
    /// needs telling). Only one window is fullscreen at a time, and fullscreening leaves the
    /// window with no other state: a minimized or maximized window is brought back to normal
    /// and then fullscreened.
    pub fn set_fullscreen(&mut self, id: u32, on: bool) -> bool {
        if !self.windows.iter().any(|w| w.id == id) {
            return false;
        }
        let was = self.fullscreen;
        if on {
            if was != Some(id) {
                let w = self.windows.iter().find(|w| w.id == id).copied();
                self.before_fullscreen = w.map(|w| (w.id, w.w, w.h));
                self.fullscreen = Some(id);
            }
            self.unminimize(id);
            self.unmaximize(id);
            self.focused = Some(id);
            self.raise(id);
        } else if was == Some(id) {
            self.fullscreen = None;
        }
        self.fullscreen != was
    }

    /// Make a window maximized, or not: maximized fills the screen less the bar, at the top.
    /// Returns whether anything changed. The geometry the window had is remembered, and
    /// un-maximizing puts it back. Maximizing un-minimizes and un-fullscreens the window.
    pub fn set_maximized(&mut self, id: u32, on: bool) -> bool {
        let Some(w) = self.windows.iter().find(|w| w.id == id).copied() else {
            return false;
        };
        if w.maximized == on {
            return false;
        }
        if on {
            // Remember where it was and its size, to put it back. The window is no longer
            // fullscreen, so `layout` gives its real place; if it was fullscreen, remember
            // the size it had before fullscreen (its committed size is still the fullscreen
            // one).
            let was_fullscreen = self.fullscreen == Some(id);
            if was_fullscreen {
                self.fullscreen = None;
            }
            let placed = self.layout().into_iter().find(|p| p.id == id);
            let (sx, sy) = placed.map(|p| (p.x, p.y)).unwrap_or((MARGIN, MARGIN));
            let size = was_fullscreen
                .then(|| self.size_before_fullscreen(id))
                .flatten();
            let (pw, ph) = size
                .or_else(|| placed.map(|p| (p.w, p.h)))
                .unwrap_or((w.w, w.h));
            self.before_maximized = Some((id, sx, sy, pw, ph));
            self.unminimize(id);
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.maximized = true;
            }
            self.focused = Some(id);
            self.raise(id);
        } else {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.maximized = false;
            }
            self.restore_maximized_geometry(id);
        }
        true
    }

    /// Make a window minimized, or not. Returns whether anything changed. A minimized window
    /// is not drawn, not hit and not switched to. Minimizing un-fullscreens and un-maximizes
    /// the window, and drops its focus.
    pub fn set_minimized(&mut self, id: u32, on: bool) -> bool {
        let Some(w) = self.windows.iter().find(|w| w.id == id).copied() else {
            return false;
        };
        if w.minimized == on {
            return false;
        }
        if on {
            if self.fullscreen == Some(id) {
                self.fullscreen = None;
            }
            self.unmaximize(id);
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.minimized = true;
            }
            self.last_minimized = Some(id);
            // A minimized window does not hold focus: give it to the next window on screen.
            if self.focused == Some(id) {
                self.focused = self
                    .windows
                    .iter()
                    .rfind(|w| w.id != id && !w.minimized)
                    .map(|w| w.id);
            }
        } else if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = false;
            if self.last_minimized == Some(id) {
                self.last_minimized = None;
            }
        }
        true
    }

    /// Bring a window back from minimized (used by [`Self::set_maximized`] and
    /// [`Self::set_fullscreen`], which must not leave a window minimized behind them).
    fn unminimize(&mut self, id: u32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = false;
        }
        if self.last_minimized == Some(id) {
            self.last_minimized = None;
        }
    }

    /// Clear a window's maximized flag and put it back where it was (used by
    /// [`Self::set_maximized`], [`Self::set_fullscreen`] and [`Self::set_minimized`], which
    /// must not leave a window maximized behind them).
    fn unmaximize(&mut self, id: u32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.maximized = false;
        }
        self.restore_maximized_geometry(id);
    }

    /// Put a window back at the geometry it had before it was maximized (the `moved` entry
    /// the layout uses for a placed window). Keeps `before_maximized` so the compositor can
    /// still ask it back to its old size afterwards.
    fn restore_maximized_geometry(&mut self, id: u32) {
        if let Some(&(_, x, y, _, _)) = self.before_maximized.as_ref().filter(|(i, ..)| *i == id) {
            self.moved.retain(|(i, _, _)| *i != id);
            self.moved.push((id, x, y));
        }
    }

    /// Whether a window is maximized.
    #[must_use]
    pub fn is_maximized(&self, id: u32) -> bool {
        self.windows.iter().any(|w| w.id == id && w.maximized)
    }

    /// Whether a window is minimized.
    #[must_use]
    pub fn is_minimized(&self, id: u32) -> bool {
        self.windows.iter().any(|w| w.id == id && w.minimized)
    }

    #[must_use]
    pub fn fullscreen(&self) -> Option<u32> {
        self.fullscreen
    }

    /// The size a window had before it was last made fullscreen, to ask it back to.
    #[must_use]
    pub fn size_before_fullscreen(&self, id: u32) -> Option<(i32, i32)> {
        self.before_fullscreen
            .filter(|(i, _, _)| *i == id)
            .map(|(_, w, h)| (w, h))
    }

    /// The size a window had before it was last maximized, to ask it back to.
    #[must_use]
    pub fn size_before_maximized(&self, id: u32) -> Option<(i32, i32)> {
        self.before_maximized
            .filter(|(i, ..)| *i == id)
            .map(|(_, _, _, w, h)| (w, h))
    }

    #[must_use]
    pub fn focused(&self) -> Option<u32> {
        self.focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: u32, w: i32, h: i32) -> Window {
        Window {
            id,
            w,
            h,
            ..Window::default()
        }
    }

    #[test]
    fn a_window_too_big_for_the_screen_is_asked_to_fit_and_one_that_fits_is_left_alone() {
        // GTK's widget factory asked for 1393 x 740 on a 1024 x 768 screen: both overflow.
        // The height leaves the bar clear as well as the margins.
        let tall = 768 - 32 - BAR_HEIGHT;
        assert_eq!(fit(1393, 740, 1024, 768), Some((1024 - 32, tall)));
        assert_eq!(fit(1393, 600, 1024, 768), Some((1024 - 32, 600)));
        assert_eq!(fit(500, 2000, 1024, 768), Some((500, tall)));
        assert_eq!(fit(992, tall, 1024, 768), None, "exactly the limit fits");
        assert_eq!(fit(320, 200, 1024, 768), None);
    }

    #[test]
    fn windows_stack_down_the_screen_in_opening_order() {
        let mut m = Manager::default();
        m.sync(&[win(1, 100, 50), win(2, 80, 40)]);
        let l = m.layout();
        assert_eq!((l[0].x, l[0].y), (MARGIN, MARGIN));
        assert_eq!((l[1].x, l[1].y), (MARGIN, MARGIN + 50 + MARGIN));
    }

    #[test]
    fn a_window_that_would_open_below_the_screen_starts_a_new_stack_at_the_top() {
        // `plain` with no network: Firefox fills the screen, then the Wi-Fi picker opens.
        // Stacked under Firefox it would start below the bottom edge, where nobody sees it.
        let mut m = Manager::default();
        m.set_screen(1280, 800);
        m.sync(&[win(1, 1248, 768), win(2, 480, 360)]);
        let p = m.layout().into_iter().find(|p| p.id == 2).unwrap();
        assert!(p.y + p.h <= 800, "the picker is on the screen: {p:?}");
        assert_eq!((p.x, p.y), (MARGIN + CASCADE, MARGIN + CASCADE));
        // A third that fits under the second stacks there; one that would reach the bar
        // cascades again rather than landing on the second.
        m.sync(&[win(1, 1248, 768), win(2, 480, 300), win(3, 480, 300)]);
        assert_eq!(
            at(&m, 3),
            (MARGIN + CASCADE, MARGIN + CASCADE + 300 + MARGIN)
        );
        m.sync(&[win(1, 1248, 768), win(2, 480, 360), win(3, 480, 360)]);
        assert_eq!(at(&m, 3), (MARGIN + 2 * CASCADE, MARGIN + 2 * CASCADE));
    }

    #[test]
    fn the_wifi_button_sits_at_the_right_of_the_bar_and_goes_with_fullscreen() {
        let mut m = Manager::default();
        m.set_screen(640, 480);
        assert_eq!(bar(640, 480), (0, 448, 640, 32));
        assert_eq!(wifi_button(640, 480), (608, 452, 24, 24));
        assert!(m.on_wifi_button(608, 452) && m.on_wifi_button(631, 475));
        assert!(!m.on_wifi_button(607, 460) && !m.on_wifi_button(632, 460));
        assert!(!m.on_wifi_button(620, 451) && !m.on_wifi_button(620, 476));
        m.sync(&[win(1, 100, 50)]);
        m.set_fullscreen(1, true);
        assert!(
            !m.on_wifi_button(620, 460),
            "no bar over a fullscreen window"
        );
        assert!(
            !Manager::default().on_wifi_button(0, 0),
            "no screen, no button"
        );
    }

    #[test]
    fn the_files_button_sits_left_of_the_wifi_button_and_goes_with_fullscreen() {
        let mut m = Manager::default();
        m.set_screen(640, 480);
        // Wi-Fi at (608, 452); Files one button (24) plus a gap (8) to its left.
        assert_eq!(files_button(640, 480), (576, 452, 24, 24));
        assert!(m.on_files_button(576, 452) && m.on_files_button(599, 475));
        assert!(!m.on_files_button(575, 460) && !m.on_files_button(600, 460));
        assert!(!m.on_wifi_button(576, 460), "Files is not the Wi-Fi button");
        m.sync(&[win(1, 100, 50)]);
        m.set_fullscreen(1, true);
        assert!(
            !m.on_files_button(588, 460),
            "no bar over a fullscreen window"
        );
        assert!(
            !Manager::default().on_files_button(0, 0),
            "no screen, no button"
        );
    }

    #[test]
    fn a_window_stacked_onto_the_bar_starts_a_new_stack_instead() {
        let mut m = Manager::default();
        m.set_screen(640, 480);
        // Second window: y = 16 + 300 + 16 = 332; 332 + 120 = 452 reaches the bar at 448.
        m.sync(&[win(1, 200, 300), win(2, 200, 120)]);
        assert_eq!(at(&m, 2), (MARGIN + CASCADE, MARGIN + CASCADE));
    }

    #[test]
    fn activating_a_window_brings_it_to_the_front_with_focus() {
        let mut m = two();
        m.sync(&[win(1, 100, 50), win(2, 80, 40), win(3, 60, 30)]);
        m.activate(1);
        assert_eq!(m.focused(), Some(1));
        assert_eq!(m.layout().last().map(|p| p.id), Some(1));
        m.activate(9);
        assert_eq!(
            m.focused(),
            Some(1),
            "a window that is not there changes nothing"
        );
    }

    #[test]
    fn a_window_that_opens_comes_to_the_front() {
        // The picker opening over Firefox must be drawn over it, not behind it.
        let mut m = Manager::default();
        m.set_screen(1280, 800);
        m.sync(&[win(1, 1248, 768)]);
        m.sync(&[win(1, 1248, 768), win(2, 480, 360)]);
        let order: Vec<u32> = m.layout().iter().map(|p| p.id).collect();
        assert_eq!(order, [1, 2], "back to front: the new window is in front");
        assert_eq!(m.hit(MARGIN + CASCADE + 10, MARGIN + CASCADE + 10), Some(2));
    }

    #[test]
    fn a_click_focuses_the_window_under_it_and_empty_space_changes_nothing() {
        let mut m = Manager::default();
        m.sync(&[win(1, 100, 50), win(2, 80, 40)]);
        assert_eq!(m.focused(), None);
        assert_eq!(m.click(MARGIN + 5, MARGIN + 50 + MARGIN + 5), Some(2));
        assert_eq!(
            m.click(600, 600),
            Some(2),
            "empty space must not drop focus"
        );
        assert_eq!(
            m.click(MARGIN, MARGIN),
            Some(1),
            "the top-left pixel is inside"
        );
        assert_eq!(
            m.hit(MARGIN + 100, MARGIN),
            None,
            "the right edge is outside"
        );
    }

    #[test]
    fn closing_the_focused_window_moves_focus_to_the_most_recent_one_left() {
        let mut m = Manager::default();
        m.sync(&[win(1, 10, 10), win(2, 10, 10), win(3, 10, 10)]);
        m.click(MARGIN, MARGIN + 2 * (10 + MARGIN)); // focus 3
        assert_eq!(m.focused(), Some(3));
        m.sync(&[win(1, 10, 10), win(2, 10, 10)]);
        assert_eq!(m.focused(), Some(2));
        m.sync(&[]);
        assert_eq!(m.focused(), None);
    }

    #[test]
    fn closing_another_window_leaves_focus_alone_and_restacks() {
        let mut m = Manager::default();
        m.sync(&[win(1, 10, 10), win(2, 10, 20)]);
        m.click(MARGIN, MARGIN); // focus 1
        m.sync(&[win(1, 10, 10)]);
        assert_eq!(m.focused(), Some(1));
        m.sync(&[win(1, 10, 10), win(5, 10, 10)]);
        assert_eq!(at(&m, 5).1, MARGIN + 10 + MARGIN, "the gap closed up");
    }

    /// Two windows on a 640 x 480 screen: 1 at (16, 16) 100 x 50, 2 at (16, 82) 80 x 40.
    fn two() -> Manager {
        let mut m = Manager::default();
        m.set_screen(640, 480);
        m.sync(&[win(1, 100, 50), win(2, 80, 40)]);
        m
    }

    fn at(m: &Manager, id: u32) -> (i32, i32) {
        let p = m.layout().into_iter().find(|p| p.id == id).unwrap();
        (p.x, p.y)
    }

    #[test]
    fn a_window_is_dragged_by_its_edge_and_stays_where_it_is_put() {
        let mut m = two();
        // The strip just above window 1's top edge.
        assert_eq!(m.begin_drag(60, MARGIN - 2, false), Some(1));
        assert!(m.dragging());
        assert_eq!(m.focused(), Some(1), "picking it up focuses it");
        m.drag_to(260, 200 - 2);
        m.end_drag();
        assert!(!m.dragging());
        // Picked up 2 px above its top edge, so its top is 2 px below the pointer.
        assert_eq!(at(&m, 1), (16 + 200, 200));
        // Window 2 now stacks first, leaving no gap where 1 was.
        assert_eq!(at(&m, 2), (MARGIN, MARGIN));
        // A later move keeps it where it was put, and new windows do not disturb it.
        m.sync(&[win(1, 100, 50), win(2, 80, 40), win(3, 10, 10)]);
        assert_eq!(at(&m, 1), (216, 200));
    }

    #[test]
    fn a_press_inside_a_window_is_a_click_unless_super_is_held() {
        let mut m = two();
        assert_eq!(
            m.begin_drag(60, 40, false),
            None,
            "inside: a click for the window"
        );
        assert!(!m.dragging());
        assert_eq!(m.begin_drag(60, 40, true), Some(1), "with Super: a move");
        m.drag_to(70, 50);
        assert_eq!(at(&m, 1), (26, 26));
        // Far from every window: nothing to pick up, even with Super.
        m.end_drag();
        assert_eq!(m.begin_drag(600, 400, true), None);
    }

    #[test]
    fn a_moved_window_stays_reachable() {
        let mut m = two();
        m.begin_drag(60, MARGIN - 2, false);
        m.drag_to(-5000, -5000);
        assert_eq!(
            at(&m, 1),
            (KEEP - 100, 0),
            "its right part and its top stay on screen"
        );
        m.drag_to(5000, 5000);
        assert_eq!(at(&m, 1), (640 - KEEP, 480 - KEEP));
    }

    #[test]
    fn the_window_clicked_comes_to_the_front_and_owns_the_overlap() {
        let mut m = two();
        // Move 1 down to y = 30. Window 2 stacks up into the space it left, at (16, 16)
        // 80 x 40, so the two overlap between y = 30 and y = 56.
        m.begin_drag(60, MARGIN - 2, false);
        m.drag_to(60, 28);
        m.end_drag();
        assert_eq!(at(&m, 1), (MARGIN, 30));
        assert_eq!(at(&m, 2), (MARGIN, MARGIN));
        assert_eq!(
            m.layout().last().unwrap().id,
            1,
            "the moved window is in front"
        );
        assert_eq!(m.hit(30, 40), Some(1));
        // Alt+Tab to 2 brings it in front: the overlap is now 2's.
        assert_eq!(m.cycle(), Some(2));
        assert_eq!(m.layout().last().unwrap().id, 2);
        assert_eq!(m.hit(30, 40), Some(2));
        // And a click on 1 (below the overlap) brings 1 back.
        assert_eq!(m.click(30, 70), Some(1));
        assert_eq!(m.hit(30, 40), Some(1));
    }

    #[test]
    fn fullscreen_covers_the_screen_and_comes_back_to_where_the_window_was() {
        let mut m = two();
        m.begin_drag(60, MARGIN - 2, false);
        m.drag_to(260, 198);
        m.end_drag();
        assert!(m.set_fullscreen(2, true));
        assert!(
            !m.set_fullscreen(2, true),
            "already fullscreen: nothing to tell"
        );
        // The client takes the screen's size; its old size is kept, to ask it back to.
        m.sync(&[win(1, 100, 50), win(2, 640, 480)]);
        assert_eq!(m.size_before_fullscreen(2), Some((80, 40)));
        let p = *m.layout().last().unwrap();
        assert_eq!((p.id, p.x, p.y, p.w, p.h), (2, 0, 0, 640, 480));
        assert_eq!(m.focused(), Some(2));
        assert_eq!(m.hit(600, 400), Some(2), "it covers everything");
        // A fullscreen window does not move.
        assert_eq!(m.begin_drag(10, 10, true), None);
        assert!(m.set_fullscreen(2, false));
        m.sync(&[win(1, 100, 50), win(2, 80, 40)]);
        assert_eq!(at(&m, 2), (MARGIN, MARGIN));
        assert_eq!(at(&m, 1), (216, 200), "the others did not move");
        // Only one at a time, and a window that goes away takes it with it.
        m.set_fullscreen(1, true);
        m.set_fullscreen(2, true);
        assert_eq!(m.fullscreen(), Some(2));
        m.sync(&[win(1, 100, 50)]);
        assert_eq!(m.fullscreen(), None);
    }

    #[test]
    fn cycle_visits_every_window_and_wraps() {
        let mut m = Manager::default();
        assert_eq!(m.cycle(), None, "nothing to focus");
        m.sync(&[win(1, 1, 1), win(2, 1, 1), win(3, 1, 1)]);
        assert_eq!(m.cycle(), Some(1));
        assert_eq!(m.cycle(), Some(2));
        assert_eq!(m.cycle(), Some(3));
        assert_eq!(m.cycle(), Some(1));
    }

    #[test]
    fn a_window_resizes_by_its_corner_and_keeps_the_opposite_corner_fixed() {
        let mut m = two();
        // Window 2 is at (16, 82), 80 x 40; its bottom-right is (96, 122).
        assert!(m.begin_resize(2, Corner::BottomRight));
        assert!(m.resizing());
        assert_eq!(m.focused(), Some(2));
        assert_eq!(
            m.resize_to(200, 150),
            Some((16, 82, 184, 68)),
            "top-left fixed, size follows the pointer"
        );
        m.end_resize();
        assert!(!m.resizing());
    }

    #[test]
    fn a_window_resized_by_a_top_corner_moves_its_top_edge_not_its_bottom() {
        let mut m = two();
        // Window 1 is at (16, 16), 100 x 50; its bottom-right is (116, 66). Dragging the
        // top-left corner to (0, 0) grows the window, and the bottom-right stays put.
        assert!(m.begin_resize(1, Corner::TopLeft));
        let r = m.resize_to(0, 0).unwrap();
        assert_eq!(r, (0, 0, 116, 66), "the bottom-right (116, 66) stays put");
        assert_eq!(r.0 + r.2, 116);
        assert_eq!(r.1 + r.3, 66);
    }

    #[test]
    fn a_resize_is_clamped_to_a_floor_and_to_the_screen() {
        let mut m = Manager::default();
        m.set_screen(640, 480);
        m.sync(&[win(1, 200, 200)]);
        m.begin_resize(1, Corner::BottomRight);
        // Drag up-left past the fixed corner: clamped to the floor, never flipped.
        assert_eq!(
            m.resize_to(-100, -100),
            Some((MARGIN, MARGIN, MIN_SIZE, MIN_SIZE))
        );
        // Drag far past the screen: clamped to the screen's max_size.
        let (max_w, max_h) = max_size(640, 480);
        assert_eq!(
            m.resize_to(10000, 10000),
            Some((MARGIN, MARGIN, max_w, max_h))
        );
    }

    #[test]
    fn a_corner_resizes_an_edge_moves_and_a_fullscreen_window_does_neither() {
        let mut m = two();
        // The top-left corner of window 1 (at 16,16) is at (14, 14).
        assert_eq!(
            m.corner_at(MARGIN - 2, MARGIN - 2),
            Some((1, Corner::TopLeft))
        );
        assert_eq!(
            m.corner_at(MARGIN + 50, MARGIN - 2),
            None,
            "mid-edge is a move, not a corner"
        );
        assert_eq!(
            m.corner_at(MARGIN + 5, MARGIN + 5),
            None,
            "inside the window is neither"
        );
        // A fullscreen window has no grab strip, so no corner, and refuses to resize.
        m.set_fullscreen(1, true);
        assert_eq!(m.corner_at(0, 0), None);
        assert!(!m.begin_resize(1, Corner::BottomRight));
    }

    #[test]
    fn a_window_maximizes_to_the_screen_less_the_bar_and_comes_back() {
        let mut m = two(); // 1 at (16, 16) 100 x 50, 2 below it; screen 640 x 480.
        assert!(m.set_maximized(1, true));
        assert!(
            !m.set_maximized(1, true),
            "already maximized: nothing to tell"
        );
        // The screen less the bar, at the top; it covers everything above the bar.
        let p = m.layout().into_iter().find(|p| p.id == 1).unwrap();
        assert_eq!((p.x, p.y, p.w, p.h), (0, 0, 640, 480 - BAR_HEIGHT));
        assert_eq!(m.hit(600, 400), Some(1), "it covers the usable screen");
        // A maximized window does not move or resize.
        assert_eq!(m.begin_drag(10, 10, true), None);
        assert!(!m.begin_resize(1, Corner::BottomRight));
        // The client takes the maximized size; its old size is kept, to ask it back to.
        m.sync(&[win(1, 640, 448), win(2, 80, 40)]);
        assert_eq!(m.size_before_maximized(1), Some((100, 50)));
        // Un-maximize: back where it was, at its own size again.
        assert!(m.set_maximized(1, false));
        m.sync(&[win(1, 100, 50), win(2, 80, 40)]);
        let p = m.layout().into_iter().find(|p| p.id == 1).unwrap();
        assert_eq!((p.x, p.y, p.w, p.h), (16, 16, 100, 50));
        assert!(!m.is_maximized(1));
    }

    #[test]
    fn a_minimized_window_is_hidden_from_layout_and_cycle() {
        let mut m = two();
        assert!(m.set_minimized(1, true));
        assert!(
            !m.set_minimized(1, true),
            "already minimized: nothing to tell"
        );
        assert!(m.is_minimized(1));
        // Not placed, so not hit; window 2 stacks up into the gap and takes the clicks.
        assert!(!m.layout().iter().any(|p| p.id == 1));
        assert_eq!(m.hit(MARGIN + 5, MARGIN + 5), Some(2));
        // cycle skips it: only window 2 is on screen.
        assert_eq!(m.cycle(), Some(2));
        assert_eq!(m.cycle(), Some(2));
        assert_eq!(m.cycle(), Some(2));
        assert_eq!(m.focused(), Some(2));
    }

    #[test]
    fn cycle_brings_back_the_most_recently_minimized_window_when_none_are_left() {
        let mut m = two();
        m.set_minimized(1, true);
        m.set_minimized(2, true);
        assert!(m.layout().is_empty(), "both are hidden");
        assert_eq!(m.focused(), None, "a minimized window loses focus");
        // Nothing to switch to: the most recently minimized (2) comes back.
        assert_eq!(m.cycle(), Some(2));
        assert!(!m.is_minimized(2));
        assert!(m.is_minimized(1));
        assert_eq!(m.layout().len(), 1);
        // Minimize the last window again and cycle: it comes back too.
        m.set_minimized(2, true);
        assert_eq!(m.cycle(), Some(2));
        assert_eq!(
            m.cycle(),
            Some(2),
            "1 is still hidden; 2 is the only window left"
        );
    }

    #[test]
    fn minimize_maximize_and_fullscreen_stay_mutually_exclusive() {
        let mut m = two();
        // Minimizing a fullscreen window un-fullscreens it.
        m.set_fullscreen(1, true);
        assert_eq!(m.fullscreen(), Some(1));
        m.set_minimized(1, true);
        assert_eq!(m.fullscreen(), None, "minimizing un-fullscreens");
        assert!(m.is_minimized(1));
        // Maximizing a minimized window un-minimizes it.
        m.set_maximized(1, true);
        assert!(!m.is_minimized(1), "maximizing un-minimizes");
        assert!(m.is_maximized(1));
        assert_eq!(m.fullscreen(), None);
        // Fullscreening a maximized window un-maximizes it.
        m.set_fullscreen(1, true);
        assert!(!m.is_maximized(1), "fullscreening un-maximizes");
        assert_eq!(m.fullscreen(), Some(1));
        // Minimizing again leaves no other state on.
        m.set_minimized(1, true);
        assert_eq!(m.fullscreen(), None);
        assert!(!m.is_maximized(1));
        assert!(m.is_minimized(1));
    }

    #[test]
    fn minimizing_the_focused_window_moves_focus_to_the_next_one_on_screen() {
        let mut m = two();
        m.click(MARGIN, MARGIN); // focus 1
        assert_eq!(m.focused(), Some(1));
        m.set_minimized(1, true);
        assert_eq!(m.focused(), Some(2), "focus moved off the hidden window");
        // Un-minimize: focus stays where it was moved, not snapped back.
        m.set_minimized(1, false);
        assert_eq!(m.focused(), Some(2));
    }
}

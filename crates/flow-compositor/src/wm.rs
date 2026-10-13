//! Window management for the Classic feel (ADR 0013, item 3), as pure logic.
//!
//! WHY THIS IS SEPARATE FROM THE COMPOSITOR. Where windows go, which one is under the
//! pointer, which one has focus, and what happens when one closes are **decisions**. They
//! belong where they can be tested on any machine in microseconds, with no Wayland, no
//! renderer and no Smithay. The compositor (`headless.rs`, later the DRM backend) only
//! carries them out. Extending window management (move, fullscreen, tiling) means extending
//! this module and its tests first.
//!
//! Layout today: windows are stacked down the screen in the order they opened, `MARGIN`
//! apart, at their own size. The focused window is drawn with a border of `BORDER` pixels.

/// Gap between windows and the screen edges, in pixels.
pub const MARGIN: i32 = 16;
/// Width of the focus border drawn around the focused window, in pixels.
pub const BORDER: i32 = 3;

/// A window as the manager sees it: an ID the compositor chose, and its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub id: u32,
    pub w: i32,
    pub h: i32,
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
}

/// The windows, in the order they opened, and which one has focus.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Manager {
    windows: Vec<Window>,
    focused: Option<u32>,
}

impl Manager {
    /// Bring the window list in line with what the compositor has: new windows are added at
    /// the end, gone ones are removed, sizes are updated. If the focused window went away,
    /// focus moves to the last window that is left (the most recent), or to none.
    pub fn sync(&mut self, current: &[Window]) {
        self.windows
            .retain(|w| current.iter().any(|c| c.id == w.id));
        for c in current {
            match self.windows.iter_mut().find(|w| w.id == c.id) {
                Some(w) => *w = *c,
                None => self.windows.push(*c),
            }
        }
        if self
            .focused
            .is_some_and(|f| !self.windows.iter().any(|w| w.id == f))
        {
            self.focused = self.windows.last().map(|w| w.id);
        }
    }

    /// Where every window goes.
    #[must_use]
    pub fn layout(&self) -> Vec<Placed> {
        let mut y = MARGIN;
        self.windows
            .iter()
            .map(|w| {
                let p = Placed {
                    id: w.id,
                    x: MARGIN,
                    y,
                    w: w.w,
                    h: w.h,
                };
                y += w.h + MARGIN;
                p
            })
            .collect()
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
        }
        self.focused
    }

    /// Focus the next window in opening order, wrapping round (the switch shortcut).
    pub fn cycle(&mut self) -> Option<u32> {
        let next = match self.focused {
            None => self.windows.first(),
            Some(f) => {
                let i = self.windows.iter().position(|w| w.id == f).unwrap_or(0);
                self.windows.get((i + 1) % self.windows.len().max(1))
            }
        };
        self.focused = next.map(|w| w.id);
        self.focused
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
        Window { id, w, h }
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
        assert_eq!(m.layout()[1].y, MARGIN + 10 + MARGIN, "the gap closed up");
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
}

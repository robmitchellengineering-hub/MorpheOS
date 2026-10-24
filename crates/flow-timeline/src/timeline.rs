//! What the rewind timeline shows and what a key or a click does to it.
//!
//! No Wayland and no Strata here: the timeline is a list of rewind points, a selection, a
//! two-step confirm (choose, then confirm the plan the dry run reports), and a result line.
//! Every input turns into at most one [`Action`] for the backend, and a rewind runs in two
//! trips — a dry run first, whose counts the person confirms, then the real rewind — so all
//! of it is unit-tested anywhere Rust runs, and the window and the Strata code only carry
//! events in and actions out.

use flow_shell::{Colour, Frame, GLYPH_ADVANCE, GLYPH_H, Rect};

/// One moment a person can go back to: the folder Strata saw encrypted, how many files the
/// burst hit, and the moment to rewind that folder to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewindPoint {
    pub folder: String,
    pub files: usize,
    /// The rewind moment, in nanoseconds since the Unix epoch (Strata's timestamps).
    pub at: i64,
}

/// What a rewind did, or would do: the counts the result line reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewind {
    /// Files put back the way they were at the rewind point.
    pub come_back: usize,
    /// Files that did not exist then, moved to Strata's quarantine, not deleted.
    pub quarantine: usize,
    /// Files that could not be touched (a directory where a file is expected, and so on).
    pub failed: usize,
}

impl Rewind {
    /// The sentence the person sees, from the dry run and again once it is done. The two
    /// counts are Strata's `restore` and `move-away` steps, so "come back" and "go to
    /// quarantine" name the same things Strata actually does.
    #[must_use]
    pub fn result_line(&self) -> String {
        format!(
            "{} files will come back, {} will go to quarantine",
            self.come_back, self.quarantine
        )
    }
}

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

/// What the backend is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Run a dry run of this rewind and report its counts, so the person can confirm.
    Preview {
        point: RewindPoint,
    },
    /// Run the real rewind of this point, after the dry run was confirmed.
    Rewind {
        point: RewindPoint,
    },
    Quit,
}

/// What the timeline is doing, shown on its status line and by the body of the window.
///
/// The path through it is the two-step confirm: `List` → (Enter) → `Preparing` (dry run in
/// flight) → `Confirm` (counts shown, Enter to confirm) → `Rewinding` (real rewind, off the
/// draw loop) → `Done`. `Escape` backs out of `Confirm`; `Failed` and `Unavailable` are the
/// two ways a backend can refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    List,
    Preparing {
        point: RewindPoint,
    },
    Confirm {
        point: RewindPoint,
        counts: Rewind,
    },
    Rewinding {
        point: RewindPoint,
    },
    Done {
        point: RewindPoint,
        result: Rewind,
    },
    Failed {
        point: RewindPoint,
        why: String,
    },
    /// The alerts could not be read (Strata's store is unreadable, say).
    Unavailable {
        why: String,
    },
}

/// The timeline's colours, distinct from everything the shell and the Wi-Fi picker draw, so a
/// test reading a screenshot finds the timeline's parts by colour (AGENTS.md H20). The warm
/// amber "recovery" theme marks each step with its own band: the list rests on [`IDLE`], the
/// confirm on [`CONFIRM`], the rewind in flight on [`BUSY`], and the result on [`OK`].
pub mod colours {
    use flow_shell::Colour;
    /// The panel, a dark warm brown so the window reads as a different surface from the
    /// shell and the picker.
    pub const BACKGROUND: Colour = Colour::rgb(0x2a, 0x1a, 0x0c);
    /// The title band, slightly raised above [`BACKGROUND`].
    pub const TITLE: Colour = Colour::rgb(0x3a, 0x26, 0x12);
    /// Text: the brand's white, warmed so it sits on the brown.
    pub const TEXT: Colour = Colour::rgb(0xf7, 0xf0, 0xe4);
    /// Secondary text: the file counts and the hints.
    pub const DIM: Colour = Colour::rgb(0x9a, 0x8a, 0x72);
    /// The 1px dividers between the title, the rows and the status line.
    pub const SEPARATOR: Colour = Colour::rgb(0x4a, 0x36, 0x1e);
    /// The selected row: a dark amber, darkened enough to hold the light text.
    pub const SELECTED: Colour = Colour::rgb(0x7a, 0x4a, 0x00);
    /// The rewind in flight (checking, rewinding): a live amber.
    pub const BUSY: Colour = Colour::rgb(0xff, 0x9d, 0x00);
    /// The result: the rewind landed.
    pub const OK: Colour = Colour::rgb(0x00, 0xc0, 0x7a);
    /// Something went wrong.
    pub const FAIL: Colour = Colour::rgb(0xff, 0x4d, 0x4d);
    /// The confirm step: the plan is shown and waiting for the second Enter.
    pub const CONFIRM: Colour = Colour::rgb(0xff, 0xc0, 0x00);
    /// The list's resting status.
    pub const IDLE: Colour = Colour::rgb(0x4a, 0x3c, 0x28);
}

/// Layout, in window pixels. The font is 5 x 7, drawn at [`SCALE`].
pub const SCALE: u32 = 2;
pub const TITLE_H: u32 = 40;
pub const ROW_H: u32 = 36;
pub const STATUS_H: u32 = 36;
pub const PAD: u32 = 12;

#[derive(Debug, Clone)]
pub struct Timeline {
    points: Vec<RewindPoint>,
    selected: usize,
    phase: Phase,
    /// The rewind's progress, 0 to 100, painted while it runs off the draw loop (H17).
    progress: u32,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            selected: 0,
            phase: Phase::List,
            progress: 0,
        }
    }

    #[must_use]
    pub fn points(&self) -> &[RewindPoint] {
        &self.points
    }

    #[must_use]
    pub fn selected(&self) -> Option<&RewindPoint> {
        self.points.get(self.selected)
    }

    #[must_use]
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// The moments to show, oldest first. The selection stays on the folder it was on, so a
    /// re-issued alert does not move it.
    pub fn set_alerts(&mut self, seen: Vec<RewindPoint>) {
        let keep = self.selected().map(|p| p.folder.clone());
        self.points = seen;
        self.selected = keep
            .and_then(|f| self.points.iter().position(|p| p.folder == f))
            .unwrap_or(0);
    }

    /// The alerts could not be read at all.
    pub fn set_unavailable(&mut self, why: String) {
        self.phase = Phase::Unavailable { why };
    }

    /// The dry run came back: its counts are shown as the confirm prompt.
    pub fn previewed(&mut self, counts: Rewind) {
        let point = match &self.phase {
            Phase::Preparing { point } => Some(point.clone()),
            _ => None,
        };
        if let Some(point) = point {
            self.phase = Phase::Confirm { point, counts };
        }
    }

    /// The real rewind came back: show its result.
    pub fn finished(&mut self, result: Rewind) {
        let point = match &self.phase {
            Phase::Rewinding { point } => Some(point.clone()),
            _ => None,
        };
        if let Some(point) = point {
            self.phase = Phase::Done { point, result };
        }
    }

    /// The dry run or the rewind failed.
    pub fn failed(&mut self, why: String) {
        let point = match &self.phase {
            Phase::Preparing { point } | Phase::Rewinding { point } => Some(point.clone()),
            _ => None,
        };
        if let Some(point) = point {
            self.phase = Phase::Failed { point, why };
        }
    }

    /// Advance the progress bar the window paints while the rewind runs.
    pub fn set_progress(&mut self, progress: u32) {
        self.progress = progress.min(100);
    }

    /// One input. Returns what the backend must do, if anything.
    pub fn handle(&mut self, input: Input, height: u32) -> Action {
        match self.phase.clone() {
            Phase::Confirm { point, .. } => match input {
                Input::Enter => {
                    self.phase = Phase::Rewinding {
                        point: point.clone(),
                    };
                    Action::Rewind { point }
                }
                Input::Escape => {
                    self.phase = Phase::List;
                    Action::None
                }
                _ => Action::None,
            },
            Phase::Done { .. } => match input {
                Input::Enter => {
                    self.phase = Phase::List;
                    Action::None
                }
                Input::Escape => Action::Quit,
                _ => Action::None,
            },
            Phase::Failed { .. } => match input {
                Input::Enter | Input::Escape => {
                    self.phase = Phase::List;
                    Action::None
                }
                _ => Action::None,
            },
            Phase::Unavailable { .. } => match input {
                Input::Escape => Action::Quit,
                _ => Action::None,
            },
            Phase::Preparing { .. } | Phase::Rewinding { .. } => Action::None,
            Phase::List => self.list_input(input, height),
        }
    }

    fn list_input(&mut self, input: Input, height: u32) -> Action {
        match input {
            Input::Up => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            Input::Down => {
                if self.selected + 1 < self.points.len() {
                    self.selected += 1;
                }
                Action::None
            }
            Input::Escape => Action::Quit,
            Input::Enter => self.preview(),
            Input::Click { x: _, y } => {
                if let Some(row) = row_at(y, self.points.len(), height) {
                    if row == self.selected {
                        return self.preview();
                    }
                    self.selected = row;
                }
                Action::None
            }
        }
    }

    /// Enter on the selected moment: ask for a dry run of it, so its counts can be confirmed
    /// before anything changes.
    fn preview(&mut self) -> Action {
        let Some(point) = self.selected().cloned() else {
            return Action::None;
        };
        self.phase = Phase::Preparing {
            point: point.clone(),
        };
        Action::Preview { point }
    }

    /// Draw the timeline into a frame the size of its window.
    pub fn paint(&self, frame: &mut Frame) {
        let (w, h) = (frame.width(), frame.height());
        frame.clear(colours::BACKGROUND);
        // The title band, its label padded off the edges, and a hairline under it.
        frame.fill_rect(Rect::new(0, 0, w, TITLE_H), colours::TITLE);
        text(frame, "Rewind", PAD, centre(0, TITLE_H), colours::TEXT);
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

        match &self.phase {
            Phase::Confirm { point, counts } => {
                let y = TITLE_H + PAD;
                text(
                    frame,
                    &format!("Rewind {}?", point.folder),
                    PAD,
                    y,
                    colours::TEXT,
                );
                text(
                    frame,
                    &format!("{} files", point.files),
                    PAD,
                    y + ROW_H,
                    colours::DIM,
                );
                // The plan, in its own band, so the confirm step reads by colour (H20).
                let band = Rect::new(0, y + 2 * ROW_H, w, ROW_H);
                frame.fill_rect(band, colours::CONFIRM);
                text(
                    frame,
                    &counts.result_line(),
                    PAD,
                    centre(band.y, ROW_H),
                    colours::TEXT,
                );
                text(
                    frame,
                    "Enter to rewind, Esc to cancel",
                    PAD,
                    y + 3 * ROW_H + PAD,
                    colours::DIM,
                );
            }
            Phase::Preparing { point } => {
                text(
                    frame,
                    &format!("Checking {}...", point.folder),
                    PAD,
                    TITLE_H + PAD,
                    colours::DIM,
                );
            }
            Phase::Rewinding { point } => {
                let y = TITLE_H + PAD;
                text(
                    frame,
                    &format!("Rewinding {}...", point.folder),
                    PAD,
                    y,
                    colours::TEXT,
                );
                // A progress bar: the rewind runs on its own thread, and the window keeps
                // drawing (and answering the compositor's frame callbacks) the whole time
                // (AGENTS.md H17). The fill advances with [`Self::set_progress`].
                let track = Rect::new(PAD, y + ROW_H, w.saturating_sub(2 * PAD), ROW_H);
                frame.fill_rect(track, colours::SEPARATOR);
                let fill = track.w.saturating_mul(self.progress) / 100;
                frame.fill_rect(Rect::new(track.x, track.y, fill, track.h), colours::BUSY);
            }
            Phase::Done { point, result } => {
                let y = TITLE_H + PAD;
                text(
                    frame,
                    &format!("{} is back.", point.folder),
                    PAD,
                    y,
                    colours::TEXT,
                );
                text(frame, &result.result_line(), PAD, y + ROW_H, colours::TEXT);
            }
            Phase::Failed { point, why } => {
                text(
                    frame,
                    &format!("{}: {why}", point.folder),
                    PAD,
                    TITLE_H + PAD,
                    colours::FAIL,
                );
            }
            Phase::Unavailable { why } => {
                text(frame, why, PAD, TITLE_H + PAD, colours::FAIL);
            }
            Phase::List => {}
        }

        if !matches!(self.phase, Phase::List) {
            return;
        }
        if self.points.is_empty() {
            text(
                frame,
                "No alerts to rewind",
                PAD,
                TITLE_H + PAD,
                colours::DIM,
            );
            return;
        }
        for (i, p) in self.points.iter().enumerate().take(visible_rows(h)) {
            let y = TITLE_H + i as u32 * ROW_H;
            if i == self.selected {
                frame.fill_rect(Rect::new(0, y, w, ROW_H), colours::SELECTED);
            }
            // A hairline between rows; the status line draws its own above it.
            if i + 1 < visible_rows(h) {
                frame.fill_rect(Rect::new(0, y + ROW_H - 1, w, 1), colours::SEPARATOR);
            }
            text(frame, &p.folder, PAD, centre(y, ROW_H), colours::TEXT);
            let right = format!("{} files", p.files);
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
        match &self.phase {
            Phase::List => ("Choose a moment, then Enter".into(), colours::IDLE),
            Phase::Preparing { point } => (format!("Checking {}", point.folder), colours::BUSY),
            Phase::Confirm { counts, .. } => (counts.result_line(), colours::CONFIRM),
            Phase::Rewinding { point } => (format!("Rewinding {}", point.folder), colours::BUSY),
            Phase::Done { point, result } => {
                if result.failed == 0 {
                    (format!("{} is back", point.folder), colours::OK)
                } else {
                    (
                        format!("{} is back; {} not changed", point.folder, result.failed),
                        colours::FAIL,
                    )
                }
            }
            Phase::Failed { point, why } => (format!("{}: {why}", point.folder), colours::FAIL),
            Phase::Unavailable { why } => (why.clone(), colours::FAIL),
        }
    }
}

fn visible_rows(height: u32) -> usize {
    (height.saturating_sub(TITLE_H + STATUS_H) / ROW_H) as usize
}

/// Which moment row a point at `y` is on.
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

    const NOW: i64 = 1_760_000_000_000_000_000;
    const H: u32 = 400;

    fn point(folder: &str, files: usize) -> RewindPoint {
        RewindPoint {
            folder: folder.into(),
            files,
            at: NOW,
        }
    }

    #[test]
    fn the_list_holds_one_rewind_point_per_folder() {
        let mut t = Timeline::new();
        t.set_alerts(vec![
            point("/home/morph/Documents/clients", 8),
            point("/home/morph/Documents", 24),
        ]);
        let folders: Vec<_> = t.points().iter().map(|p| p.folder.as_str()).collect();
        assert_eq!(
            folders,
            ["/home/morph/Documents/clients", "/home/morph/Documents"]
        );
        assert_eq!(t.selected().unwrap().files, 8);
        assert_eq!(t.phase(), &Phase::List);
    }

    #[test]
    fn up_and_down_move_the_selection_and_escape_quits() {
        let mut t = Timeline::new();
        t.set_alerts(vec![point("a", 1), point("b", 2)]);
        assert_eq!(t.handle(Input::Down, H), Action::None);
        assert_eq!(t.selected().unwrap().folder, "b");
        assert_eq!(t.handle(Input::Up, H), Action::None);
        assert_eq!(t.selected().unwrap().folder, "a");
        assert_eq!(t.handle(Input::Escape, H), Action::Quit);
    }

    #[test]
    fn the_two_step_confirm_runs_a_dry_run_before_the_real_rewind() {
        let mut t = Timeline::new();
        let p = point("/home/morph/Documents", 24);
        t.set_alerts(vec![p.clone()]);
        // Step one: Enter asks for a dry run, not a rewind.
        assert_eq!(
            t.handle(Input::Enter, H),
            Action::Preview { point: p.clone() }
        );
        assert!(matches!(t.phase(), Phase::Preparing { .. }));
        // The dry run returns counts, shown as the confirm prompt, before anything changes.
        t.previewed(Rewind {
            come_back: 24,
            quarantine: 25,
            failed: 0,
        });
        assert!(matches!(
            t.phase(),
            Phase::Confirm {
                counts,
                ..
            } if counts.come_back == 24 && counts.quarantine == 25
        ));
        // Step two: Enter confirms the plan and runs the real rewind.
        assert_eq!(
            t.handle(Input::Enter, H),
            Action::Rewind { point: p.clone() }
        );
        assert!(matches!(t.phase(), Phase::Rewinding { .. }));
        t.finished(Rewind {
            come_back: 24,
            quarantine: 25,
            failed: 0,
        });
        assert!(matches!(
            t.phase(),
            Phase::Done { result, .. } if result.come_back == 24 && result.quarantine == 25
        ));
    }

    #[test]
    fn escape_cancels_the_confirm_and_returns_to_the_list() {
        let mut t = Timeline::new();
        let p = point("/home/morph/Documents", 24);
        t.set_alerts(vec![p.clone()]);
        t.handle(Input::Enter, H);
        t.previewed(Rewind {
            come_back: 24,
            quarantine: 25,
            failed: 0,
        });
        assert!(matches!(t.phase(), Phase::Confirm { .. }));
        assert_eq!(t.handle(Input::Escape, H), Action::None);
        assert_eq!(t.phase(), &Phase::List);
        assert_eq!(t.selected().unwrap().folder, "/home/morph/Documents");
    }

    #[test]
    fn the_result_line_names_what_comes_back_and_what_is_quarantined() {
        let r = Rewind {
            come_back: 24,
            quarantine: 25,
            failed: 0,
        };
        assert_eq!(
            r.result_line(),
            "24 files will come back, 25 will go to quarantine"
        );
    }

    #[test]
    fn a_click_selects_a_row_and_a_second_click_confirms_it() {
        let mut t = Timeline::new();
        t.set_alerts(vec![point("a", 1), point("b", 2)]);
        let row_b = TITLE_H + ROW_H + ROW_H / 2;
        assert_eq!(t.handle(Input::Click { x: 30, y: row_b }, H), Action::None);
        assert_eq!(t.selected().unwrap().folder, "b");
        assert_eq!(
            t.handle(Input::Click { x: 30, y: row_b }, H),
            Action::Preview {
                point: point("b", 2)
            }
        );
    }

    fn count(frame: &Frame, c: Colour) -> usize {
        frame.pixels().iter().filter(|p| **p == c.to_argb()).count()
    }

    #[test]
    fn the_picture_shows_the_selection_and_the_step_bands() {
        let mut t = Timeline::new();
        t.set_alerts(vec![point("a", 1), point("b", 2)]);
        let mut f = Frame::new(480, H);
        t.paint(&mut f);
        // The selected row is a band across the window.
        assert!(count(&f, colours::SELECTED) > 480 * ROW_H as usize * 8 / 10);
        assert_eq!(f.pixel(1, TITLE_H + 1), Some(colours::SELECTED));
        assert_eq!(f.pixel(1, TITLE_H + ROW_H + 1), Some(colours::BACKGROUND));
        // The list rests on the idle status line.
        assert_eq!(f.pixel(1, H - 2), Some(colours::IDLE));

        // Confirm shows the plan in the confirm band; done shows the OK band.
        t.handle(Input::Enter, H);
        t.previewed(Rewind {
            come_back: 24,
            quarantine: 25,
            failed: 0,
        });
        t.paint(&mut f);
        assert_eq!(f.pixel(1, H - 2), Some(colours::CONFIRM));
        assert!(count(&f, colours::CONFIRM) > 0);

        t.handle(Input::Enter, H);
        t.finished(Rewind {
            come_back: 24,
            quarantine: 25,
            failed: 0,
        });
        t.paint(&mut f);
        assert_eq!(f.pixel(1, H - 2), Some(colours::OK));
    }
}

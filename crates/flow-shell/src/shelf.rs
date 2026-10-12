//! The Session + shelf feel's chrome: the shelf of the session's openables, and the
//! intent bar that is a search field.
//!
//! **What this is.** ADR 0013's "Session + shelf" feel shows *the current task up front,
//! its tools prewarmed around it* — which this slice reduces to two regions the shell
//! paints around the focused session's surface:
//!
//! - the **shelf**: a thin strip listing the focused session's [`Openable`]s, left to
//!   right. It is a list of the session's capsules, **not an app grid** (ADR 0013 names
//!   that distinction explicitly), and it lists only what the session model already
//!   carries — the shell invents no apps.
//! - the **intent bar**: a search field that shows the query the person has typed, a caret
//!   when it is the input target, and "no match" when a committed query matched nothing.
//!
//! Both are painted as rectangle fills and glyphs, like the rest of the frame: no image
//! library, no text engine. What a capsule *does* when pressed, and what a matched
//! session *opens*, are later slices — this crate draws the feel, it does not run it.

use crate::{Frame, GLYPH_ADVANCE, GLYPH_H, GLYPH_W, Rect, TEXT_PAD, palette, terminal};
use flow_session::Openable;

/// How many glyph cells a line of text advances, so a caret or a neighbour can be placed
/// after it without counting bytes (a character is a cell, not a byte).
fn text_cells(text: &str) -> u32 {
    text.chars().count() as u32
}

/// Paint the intent bar: its background, the query, a caret when it is the input target,
/// and "no match" when the last committed query matched nothing.
pub(crate) fn paint_intent_bar(
    frame: &mut Frame,
    rect: Rect,
    query: &str,
    active: bool,
    no_match: bool,
) {
    frame.fill_rect(rect, palette::INTENT_BAR);
    let text_x = rect.x.saturating_add(TEXT_PAD);
    let text_y = rect.y.saturating_add(TEXT_PAD);
    terminal::paint_text(frame, query, text_x, text_y, palette::TEXT);

    // The caret sits one cell past the query, exactly where the next character lands. It
    // is drawn only when the intent bar is the input target, mirroring the terminal's
    // cursor: one marker for where input goes.
    if active {
        let caret_x = text_x.saturating_add(text_cells(query) * GLYPH_ADVANCE);
        frame.fill_rect(
            Rect::new(caret_x, text_y, GLYPH_W, GLYPH_H),
            palette::ACCENT,
        );
    }

    // "no match" is shown one cell past the caret, so a query that matched nothing reads
    // as the query followed by its verdict, never as a silent blank field.
    if no_match {
        let start = text_x
            .saturating_add(text_cells(query) * GLYPH_ADVANCE)
            .saturating_add(GLYPH_ADVANCE);
        terminal::paint_text(frame, "no match", start, text_y, palette::NO_MATCH);
    }
}

/// Paint the shelf: its background and the focused session's openables, left to right.
///
/// `openables` is the focused session's list (`None` when nothing is focused, which the
/// Session + shelf chrome cannot reach, but the type stays total).
pub(crate) fn paint_shelf(frame: &mut Frame, rect: Rect, openables: Option<&[Openable]>) {
    frame.fill_rect(rect, palette::SHELF);
    let Some(openables) = openables else {
        return;
    };
    let text_y = rect.y.saturating_add(TEXT_PAD);
    let mut x = rect.x.saturating_add(TEXT_PAD);
    for openable in openables {
        terminal::paint_text(frame, &openable.label, x, text_y, palette::TEXT);
        // Two cells of space between capsules, so the shelf reads as a list, not a run-on.
        x = x
            .saturating_add(text_cells(&openable.label) * GLYPH_ADVANCE)
            .saturating_add(GLYPH_ADVANCE * 2);
    }
}

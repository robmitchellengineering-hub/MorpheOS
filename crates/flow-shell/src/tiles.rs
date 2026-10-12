//! The Flow feel's chrome: sessions as large tiles, labelled by what the person calls
//! them, and the focus border that shows which one has been opened.
//!
//! **What this is.** ADR 0013's Flow feel shows *no app grid; surfaces are tasks, people
//! and time* — Margaret never sees an app, she sees "Photos of the grandkids", "Call
//! Jenny", "that letter". This slice reduces that to two things the shell paints when the
//! focused session's feel is [`Feel::Flow`]:
//!
//! - **tiles**: each session is laid out as a large tile rather than the stacked surfaces
//!   the Session + shelf and Classic feels share. The tile *is* the surface — the session
//!   is the thing (a task, a person, a moment), not a window onto an app. The focused
//!   session is the **hero**: it takes a larger share of the frame (it has been opened),
//!   the rest stay smaller tiles beside and below it.
//! - **large type**: the tile is labelled with the session's name at
//!   [`LARGE_TYPE_SCALE`] times the terminal font, and its openables (the surfaces the
//!   session is opening) beneath it at the normal size. No shelf, no intent bar, no app
//!   grid — the name is the label, the openables are the contents, and both are drawn
//!   with the same glyphs the rest of the frame uses.
//!
//! What *opens* when a tile is pressed is focus, made visible: the pressed tile becomes
//! the hero (it gains the focus border and the space), and the previous hero steps back to
//! a tile. That is the whole of "a tile opens a session" in this slice, and it is asserted
//! on the painted frame rather than read back from a return value.

use crate::{
    Colour, FOCUS_BORDER, Frame, GLYPH_ADVANCE, GLYPH_H, LARGE_TYPE_SCALE, MARGIN, Rect, TEXT_PAD,
    font, palette, terminal,
};
use flow_session::Openable;

/// The number of columns the non-focused tiles are arranged in.
const TILE_COLUMNS: u32 = 2;
/// The share of the content height the focused (opened) tile takes. The hero is always
/// larger than any single tile, so "opened" is visible as more space, not only as chrome.
const HERO_NUMERATOR: u32 = 3;
const HERO_DENOMINATOR: u32 = 5;

/// The advance of one large-type character: the glyph plus its spacing, scaled like the
/// glyph itself.
const fn large_advance() -> u32 {
    GLYPH_ADVANCE * LARGE_TYPE_SCALE
}

/// Lay the sessions out as Flow tiles, the focused one as the hero.
///
/// `focused` is the index of the focused session, or `None` when nothing is focused (the
/// shell cannot reach the Flow feel without a focused session, but the layout stays total).
/// The rectangles are returned in session order, so `sessions()[i].surface` receives
/// `out[i]`.
pub(crate) fn tile_layout(area: Rect, count: usize, focused: Option<usize>) -> Vec<Rect> {
    let Some(f) = focused.filter(|&f| f < count) else {
        return grid_tiles(area, count);
    };
    if count == 1 {
        // One session: its tile is the whole content area. There is nothing to be bigger
        // than, so "opened" is simply "present".
        return vec![area];
    }

    let hero_h = area
        .h
        .saturating_mul(HERO_NUMERATOR)
        .saturating_div(HERO_DENOMINATOR);
    let mut rects = vec![Rect::new(0, 0, 0, 0); count];
    rects[f] = Rect::new(area.x, area.y, area.w, hero_h);

    let rest_top = area.y.saturating_add(hero_h).saturating_add(MARGIN);
    let rest_h = area.h.saturating_sub(hero_h).saturating_sub(MARGIN);
    let rest_area = Rect::new(area.x, rest_top, area.w, rest_h);
    let rest_indices: Vec<usize> = (0..count).filter(|&i| i != f).collect();
    let grid = grid_tiles(rest_area, rest_indices.len());
    for (index, rect) in rest_indices.into_iter().zip(grid) {
        rects[index] = rect;
    }
    rects
}

/// A uniform grid of `count` equal tiles across `area`, [`TILE_COLUMNS`] per row.
fn grid_tiles(area: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let cols = TILE_COLUMNS.min(count as u32);
    let rows = count.div_ceil(cols as usize) as u32;
    let tile_w = area
        .w
        .saturating_sub(MARGIN.saturating_mul(cols.saturating_sub(1)))
        .saturating_div(cols);
    let tile_h = area
        .h
        .saturating_sub(MARGIN.saturating_mul(rows.saturating_sub(1)))
        .saturating_div(rows);
    (0..count)
        .map(|i| {
            let c = i as u32 % cols;
            let r = i as u32 / cols;
            Rect::new(
                area.x
                    .saturating_add(c.saturating_mul(tile_w.saturating_add(MARGIN))),
                area.y
                    .saturating_add(r.saturating_mul(tile_h.saturating_add(MARGIN))),
                tile_w,
                tile_h,
            )
        })
        .collect()
}

/// Paint one Flow tile: its surface, the focus border on the opened one, the session's
/// name in large type, and its openables beneath at the normal size.
pub(crate) fn paint_tile(
    frame: &mut Frame,
    label: &str,
    openables: &[Openable],
    surface: Rect,
    focused: bool,
) {
    let fill = if focused {
        palette::SURFACE_FOCUSED
    } else {
        palette::SURFACE
    };
    frame.fill_rect(surface, fill);

    if focused {
        paint_border(frame, surface, FOCUS_BORDER, palette::ACCENT);
    }

    let text_x = surface.x.saturating_add(TEXT_PAD);
    let text_y = surface.y.saturating_add(TEXT_PAD);
    paint_text_large(frame, label, text_x, text_y, palette::TEXT);

    let sub_y = text_y
        .saturating_add(GLYPH_H.saturating_mul(LARGE_TYPE_SCALE))
        .saturating_add(TEXT_PAD);
    let sub = openables
        .iter()
        .map(|o| o.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if !sub.is_empty() {
        terminal::paint_text(frame, &sub, text_x, sub_y, palette::TEXT);
    }
}

/// Draw text with each font pixel scaled to [`LARGE_TYPE_SCALE`] — the tile's large type.
fn paint_text_large(frame: &mut Frame, text: &str, x: u32, y: u32, colour: Colour) {
    for (col, ch) in text.chars().enumerate() {
        let cx = x.saturating_add(col as u32 * large_advance());
        paint_glyph_large(frame, ch, cx, y, colour);
    }
}

/// Draw one glyph at [`LARGE_TYPE_SCALE`] pixels per font pixel.
fn paint_glyph_large(frame: &mut Frame, ch: char, x: u32, y: u32, colour: Colour) {
    for (col, byte) in font::glyph(ch).iter().enumerate() {
        for row in 0..GLYPH_H {
            if byte & (1 << row) != 0 {
                frame.fill_rect(
                    Rect::new(
                        x + col as u32 * LARGE_TYPE_SCALE,
                        y + row * LARGE_TYPE_SCALE,
                        LARGE_TYPE_SCALE,
                        LARGE_TYPE_SCALE,
                    ),
                    colour,
                );
            }
        }
    }
}

/// The focus chrome: an accent border around the opened tile.
///
/// The fills are clipped by [`Frame`], so a tile too small for a real ring simply takes
/// as much border as fits rather than failing.
fn paint_border(frame: &mut Frame, rect: Rect, thickness: u32, colour: Colour) {
    frame.fill_rect(Rect::new(rect.x, rect.y, rect.w, thickness), colour); // top
    frame.fill_rect(
        Rect::new(
            rect.x,
            rect.bottom().saturating_sub(thickness),
            rect.w,
            thickness,
        ),
        colour, // bottom
    );
    frame.fill_rect(Rect::new(rect.x, rect.y, thickness, rect.h), colour); // left
    frame.fill_rect(
        Rect::new(
            rect.right().saturating_sub(thickness),
            rect.y,
            thickness,
            rect.h,
        ),
        colour, // right
    );
}

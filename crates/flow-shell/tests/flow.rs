//! The Flow feel: sessions as large tiles, large-type labels, no app grid — and a tile
//! press that opens a session, with the negative controls that prove the open is real.
//!
//! ADR 0013's Flow slice is "session surfaces as tiles (people/time/task), large-type
//! rendering, no app grid. Proved by a frame where a tile opens a session." These tests
//! hold the frame to exactly that: they assert what was *drawn*, not what a method
//! returned — a `handle` that focused the same session regardless of which tile was
//! pressed, or that returned `true` while painting nothing, would leave them red (H17:
//! a check that cannot fail is not a pass).

use flow_shell::{
    Colour, Event, Feel, Frame, GLYPH_H, LARGE_TYPE_SCALE, Openable, Pointer, Rect, SessionId,
    Shell, TEXT_PAD, palette,
};

/// How many pixels of one colour sit inside a rectangle.
fn count_in_rect(frame: &Frame, rect: Rect, colour: Colour) -> usize {
    let mut count = 0;
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            if frame.pixel(x, y) == Some(colour) {
                count += 1;
            }
        }
    }
    count
}

/// The area of a rectangle, in pixels, wide enough not to overflow on a real frame.
fn area_of(rect: Rect) -> u64 {
    u64::from(rect.w) * u64::from(rect.h)
}

/// A shell with two Flow sessions: "Photos of the grandkids" (a surface) and "Call Jenny"
/// (nothing app-shaped). The most recent — Call Jenny — has focus.
fn two_flow_sessions() -> (Shell, SessionId, SessionId) {
    let mut shell = Shell::new(800, 600);
    let photos = shell.add_session_opening(
        "Photos of the grandkids",
        vec![Openable::surface("grandkids")],
    );
    let jenny = shell.add_session_opening("Call Jenny", vec![]);
    (shell, photos, jenny)
}

#[test]
fn a_tile_opens_a_session() {
    let (mut shell, photos, jenny) = two_flow_sessions();
    assert_eq!(
        shell.focused(),
        Some(jenny),
        "the most recent session is focused"
    );
    assert_eq!(shell.sessions()[0].model.feel(), Feel::Flow);
    assert_eq!(shell.sessions()[1].model.feel(), Feel::Flow);

    let photos_surface = shell.sessions()[0].surface;
    let jenny_surface = shell.sessions()[1].surface;

    // Before: the focused tile is the hero — it has the focus border and more space than
    // the unfocused tile.
    let before = shell.paint();
    assert!(
        count_in_rect(&before, jenny_surface, palette::ACCENT) > 0,
        "the focused tile has focus chrome (the accent border)"
    );
    assert_eq!(
        count_in_rect(&before, photos_surface, palette::ACCENT),
        0,
        "the unfocused tile has no focus chrome"
    );
    assert!(
        area_of(jenny_surface) > area_of(photos_surface),
        "the focused tile is the hero: it has more space"
    );

    // Press the unfocused tile: it opens — focus moves to it, and the frame changes.
    assert!(
        shell.handle(Event::Pointer(Pointer::Press {
            x: photos_surface.x + 4,
            y: photos_surface.y + 4,
        })),
        "a press on a tile is consumed"
    );
    assert_eq!(
        shell.focused(),
        Some(photos),
        "the tile press moved focus to that session"
    );

    let after = shell.paint();
    let new_photos = shell.sessions()[0].surface;
    let new_jenny = shell.sessions()[1].surface;

    // The frame changed: the pressed tile gained focus chrome and space; the old hero lost
    // both.
    assert!(
        count_in_rect(&after, new_photos, palette::ACCENT) > 0,
        "the pressed tile gained focus chrome"
    );
    assert_eq!(
        count_in_rect(&after, new_jenny, palette::ACCENT),
        0,
        "the previous hero lost focus chrome"
    );
    assert!(
        area_of(new_photos) > area_of(new_jenny),
        "the pressed tile expanded; the previous hero shrank back to a tile"
    );
    assert_ne!(before, after, "the frame changed when the tile opened");
}

#[test]
fn the_flow_feel_lays_sessions_out_as_tiles() {
    let mut shell = Shell::new(900, 700);
    shell.add_session_opening("Photos", vec![Openable::surface("grandkids")]);
    shell.add_session_opening("Call Jenny", vec![]);
    shell.add_session_opening("That letter", vec![Openable::surface("council")]);
    assert_eq!(shell.focused(), Some(shell.sessions()[2].model.id()));
    assert_eq!(shell.sessions()[0].model.feel(), Feel::Flow);

    let area = shell.content_area();
    let surfaces: Vec<Rect> = shell.sessions().iter().map(|s| s.surface).collect();

    // Tiles, not stacked surfaces: every tile sits inside the content area with real size,
    // and none overlaps another.
    for s in &surfaces {
        assert!(
            s.x >= area.x && s.right() <= area.right(),
            "tile {s:?} stays inside the content area"
        );
        assert!(
            s.y >= area.y && s.bottom() <= area.bottom(),
            "tile {s:?} stays inside the content area"
        );
        assert!(s.w > 0 && s.h > 0, "every tile has a real surface: {s:?}");
    }
    for (i, a) in surfaces.iter().enumerate() {
        for b in surfaces.iter().skip(i + 1) {
            assert!(
                a.intersect(b).is_none(),
                "two tiles share pixels: {a:?} and {b:?}"
            );
        }
    }

    // The focused session is the hero: full width, above the rest, and larger than a tile.
    let hero = surfaces[2];
    let rest = [surfaces[0], surfaces[1]];
    assert_eq!(hero.x, area.x, "the hero spans the content width");
    assert_eq!(hero.w, area.w, "the hero spans the content width");
    assert!(hero.y < rest[0].y, "the hero sits above the other tiles");
    assert!(
        area_of(hero) > area_of(rest[0]),
        "the hero has more space than a tile"
    );

    // The unfocused sessions are tiles in a row: same top edge, side by side, not stacked.
    assert_eq!(rest[0].y, rest[1].y, "the tiles share a row");
    assert_eq!(rest[0].h, rest[1].h, "the tiles are the same height");
    assert_ne!(
        rest[0].x, rest[1].x,
        "the tiles sit side by side, not stacked"
    );
}

#[test]
fn large_type_renders_the_label_bigger_than_the_font() {
    // The tile label is large type: the same 5×7 glyph scaled by LARGE_TYPE_SCALE. A Flow
    // shell labelled "I" pins it — 'I' has a solid middle column — because a pixel one row
    // BELOW the normal font's seven rows is painted, which a normal-size glyph cannot
    // reach.
    let mut shell = Shell::new(400, 300);
    shell.add_session_opening("I", vec![]);
    assert_eq!(shell.sessions()[0].model.feel(), Feel::Flow);

    let frame = shell.paint();
    let surface = shell.sessions()[0].surface;
    let lx = surface.x + TEXT_PAD;
    let ly = surface.y + TEXT_PAD;

    // 'I' is columns [0x00, 0x41, 0x7F, 0x41, 0x00]: the middle column (2) is solid across
    // all seven rows. Scaled by LARGE_TYPE_SCALE it starts at x-offset 2*scale and spans
    // 7*scale rows. A normal-size glyph would stop at row offset GLYPH_H - 1 = 6.
    let middle_col = 2 * LARGE_TYPE_SCALE + 1;
    assert_eq!(
        frame.pixel(lx + middle_col, ly + GLYPH_H),
        Some(palette::TEXT),
        "the label glyph extends below the normal 5x7 font: it is large type"
    );
    assert_eq!(
        frame.pixel(lx + middle_col, ly + 2 * GLYPH_H),
        Some(palette::TEXT),
        "the scaled glyph spans well past the normal font's height"
    );
}

#[test]
fn a_press_on_nothing_does_not_fake_an_open() {
    // THE NEGATIVE CONTROL for the pointer path. A press that lands on no tile must report
    // `false`, move no focus, and change no pixel — otherwise a `handle` that returned
    // `true` unconditionally would pass the positive test and claim an open that never
    // happened.
    let (mut shell, _, jenny) = two_flow_sessions();
    assert_eq!(shell.focused(), Some(jenny));
    let before = shell.paint();

    // The top bar is not a tile.
    assert!(
        !shell.handle(Event::Pointer(Pointer::Press { x: 5, y: 5 })),
        "a press on the bar is not a tile"
    );
    assert_eq!(shell.focused(), Some(jenny), "focus did not move");

    // The gap between the hero and the tile below is empty space, not a tile.
    let hero = shell.sessions()[1].surface;
    let gap_y = hero.bottom() + 1;
    assert!(
        !shell.handle(Event::Pointer(Pointer::Press {
            x: hero.x + 2,
            y: gap_y,
        })),
        "a press in empty space is not a tile"
    );
    assert_eq!(shell.focused(), Some(jenny), "focus did not move");

    let after = shell.paint();
    assert_eq!(before, after, "a press on nothing must change no pixel");
}

#[test]
fn a_tile_open_that_returned_a_fixed_result_would_fail() {
    // THE NEGATIVE CONTROL for the open itself (H17). A `handle` that always focused the
    // same session — or returned success regardless of which tile was pressed — would pass
    // a single "press and assert focus" test. This fails it: distinct tiles must focus
    // distinct sessions, and a press that lands nowhere must focus no one.
    let (mut shell, photos, jenny) = two_flow_sessions();
    assert_eq!(shell.focused(), Some(jenny));
    assert_ne!(
        photos, jenny,
        "the two sessions are distinct, or the contrast is vacuous"
    );

    // Press the first tile → focus moves to it.
    let photos_surface = shell.sessions()[0].surface;
    assert!(shell.handle(Event::Pointer(Pointer::Press {
        x: photos_surface.x + 4,
        y: photos_surface.y + 4,
    })));
    assert_eq!(
        shell.focused(),
        Some(photos),
        "pressing the first tile focuses the first session"
    );

    // Re-read the surfaces after the relayout, then press the second tile → focus moves back.
    let jenny_surface = shell.sessions()[1].surface;
    assert!(shell.handle(Event::Pointer(Pointer::Press {
        x: jenny_surface.x + 4,
        y: jenny_surface.y + 4,
    })));
    assert_eq!(
        shell.focused(),
        Some(jenny),
        "pressing the second tile focuses the second session"
    );

    // A press that lands on no tile focuses no one.
    let before = shell.focused();
    assert!(!shell.handle(Event::Pointer(Pointer::Press { x: 5, y: 5 })));
    assert_eq!(
        shell.focused(),
        before,
        "a press on nothing changes no focus"
    );
}

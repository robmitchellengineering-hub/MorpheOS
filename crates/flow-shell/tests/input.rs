//! Input changes the frame — and the negative control that proves it is not a fake pass.
//!
//! The existing `frame_evidence` tests prove a frame has content. These prove *more*: that
//! feeding an event changes the frame in the specific way input is supposed to — focus
//! moves, text appears, the cursor follows — rather than that a `handle` method returned
//! `true`. A `handle` that returned `true` while drawing nothing would leave every
//! assertion here red, which is the whole point.

use flow_shell::{
    Colour, Event, Frame, GLYPH_ADVANCE, GLYPH_H, GLYPH_W, Key, LINE_ADVANCE, Pointer, Rect, Shell,
    TEXT_PAD, palette,
};

/// The top-left of a session's terminal text, matching how `terminal::paint` lays it out.
fn text_origin(surface: Rect) -> (u32, u32) {
    (surface.x + TEXT_PAD, surface.y + TEXT_PAD)
}

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

/// The rectangle a glyph cell occupies at column `col`, row `row` of a terminal.
fn cell(origin: (u32, u32), col: u32, row: u32) -> Rect {
    Rect::new(
        origin.0 + col * GLYPH_ADVANCE,
        origin.1 + row * LINE_ADVANCE,
        GLYPH_W,
        GLYPH_H,
    )
}

#[test]
fn typing_paints_text_and_moves_the_cursor() {
    let mut shell = Shell::new(320, 200);
    shell.add_session("a terminal");
    let origin = text_origin(shell.sessions()[0].surface);

    // The focused terminal starts with a cursor in its first cell, and no text.
    let before = shell.paint();
    assert_eq!(
        before.pixel(origin.0, origin.1),
        Some(palette::ACCENT),
        "the cursor block sits at the text origin"
    );

    assert!(shell.handle(Event::Key(Key::Char('A'))));
    assert!(shell.handle(Event::Key(Key::Char('B'))));

    // The buffer accumulated the text, and the cursor followed it.
    let terminal = shell.sessions()[0].terminal();
    assert_eq!(terminal.text(), "AB");
    assert_eq!(terminal.cursor(), (2, 0));

    // The painted frame reflects that: a glyph in each of the first two cells, and the
    // cursor block now in the third. This is the specific change input must cause.
    let after = shell.paint();
    assert!(
        count_in_rect(&after, cell(origin, 0, 0), palette::TEXT) > 0,
        "'A' drew some text pixels in the first cell"
    );
    assert!(
        count_in_rect(&after, cell(origin, 1, 0), palette::TEXT) > 0,
        "'B' drew some text pixels in the second cell"
    );
    assert_eq!(
        after.pixel(origin.0 + 2 * GLYPH_ADVANCE, origin.1),
        Some(palette::ACCENT),
        "the cursor moved to the cell after the text"
    );
    assert_ne!(
        after.pixel(origin.0, origin.1),
        Some(palette::ACCENT),
        "the cursor block left the first cell"
    );
}

#[test]
fn pointer_and_tab_move_focus_and_the_frame_follows() {
    let mut shell = Shell::new(400, 300);
    let first = shell.add_session("first");
    let second = shell.add_session("second");
    assert_eq!(shell.focused(), Some(second));

    let first_surface = shell.sessions()[0].surface;
    let second_surface = shell.sessions()[1].surface;

    // A press inside the first session's surface gives it focus.
    assert!(shell.handle(Event::Pointer(Pointer::Press {
        x: first_surface.x + 4,
        y: first_surface.y + 4,
    })));
    assert_eq!(shell.focused(), Some(first));
    let frame = shell.paint();
    assert_eq!(
        frame.pixel(first_surface.x + 1, first_surface.y + 1),
        Some(palette::SURFACE_FOCUSED)
    );
    assert_eq!(
        frame.pixel(second_surface.x + 1, second_surface.y + 1),
        Some(palette::SURFACE)
    );

    // Tab moves focus on to the next session, and the frame follows.
    assert!(shell.handle(Event::Key(Key::Tab)));
    assert_eq!(shell.focused(), Some(second));
    let frame = shell.paint();
    assert_eq!(
        frame.pixel(first_surface.x + 1, first_surface.y + 1),
        Some(palette::SURFACE)
    );
    assert_eq!(
        frame.pixel(second_surface.x + 1, second_surface.y + 1),
        Some(palette::SURFACE_FOCUSED)
    );

    // A press on the bar hits no session surface, so it changes nothing.
    assert!(
        !shell.handle(Event::Pointer(Pointer::Press { x: 5, y: 5 })),
        "a press on the bar is not a session"
    );
    assert_eq!(shell.focused(), Some(second));
}

#[test]
fn backspace_removes_and_enter_starts_a_line() {
    let mut shell = Shell::new(320, 200);
    shell.add_session("a terminal");
    let origin = text_origin(shell.sessions()[0].surface);

    shell.handle(Event::Key(Key::Char('A')));
    shell.handle(Event::Key(Key::Char('B')));
    shell.handle(Event::Key(Key::Backspace));
    assert_eq!(shell.sessions()[0].terminal().text(), "A");

    shell.handle(Event::Key(Key::Enter));
    assert_eq!(shell.sessions()[0].terminal().cursor(), (0, 1));
    assert_eq!(shell.sessions()[0].terminal().text(), "A\n");

    // The cursor block is now on the second line.
    let frame = shell.paint();
    assert_eq!(
        frame.pixel(origin.0, origin.1 + LINE_ADVANCE),
        Some(palette::ACCENT),
        "the cursor sits at the start of the new line"
    );
}

#[test]
fn input_with_no_session_is_a_no_op_not_a_pass() {
    // THE NEGATIVE CONTROL for the input path. `handle` has a success path (`true`), so
    // this proves the mirror: with no session to receive an event, it must report `false`,
    // change no pixel, and not panic. Without it, `handle` could return `true`
    // unconditionally and every other input test would stay green — hazard H9 in this
    // repository, a stub that makes a test vacuous.
    let mut shell = Shell::new(200, 120);
    let before = shell.paint();

    assert!(!shell.handle(Event::Key(Key::Char('x'))));
    assert!(!shell.handle(Event::Key(Key::Tab)));
    assert!(!shell.handle(Event::Key(Key::Backspace)));
    assert!(!shell.handle(Event::Key(Key::Enter)));
    assert!(!shell.handle(Event::Pointer(Pointer::Press { x: 10, y: 40 })));

    let after = shell.paint();
    assert_eq!(before, after, "input with no session must change no pixel");
    assert_eq!(
        after.distinct_colours(),
        2,
        "an empty shell is a bar and a background, and no cursor or text appeared"
    );
}

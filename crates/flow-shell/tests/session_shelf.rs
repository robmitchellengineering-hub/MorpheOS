//! The Session + shelf feel: a shelf of the session's openables, an intent bar that is a
//! search field, and a query that returns a result — with the negative controls that prove
//! the query is not a fake.
//!
//! ADR 0013's first buildable slice is "the session's surfaces plus a shelf component, one
//! focus indicator, and the intent bar as a search field. Proved by a frame where focus
//! moves and a query returns a result." These tests hold the frame to exactly that: they
//! assert what was *drawn*, not what a method returned — a `match_query` that returned a
//! fixed result regardless of input, or an intent bar that painted nothing, would leave
//! them red (H17: a check that cannot fail is not a pass).

use flow_shell::{
    Colour, Event, Feel, Frame, Key, Openable, Pointer, Rect, Shell, match_query, palette,
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

/// How many pixels of one colour sit anywhere in the frame.
fn count_in_frame(frame: &Frame, colour: Colour) -> usize {
    count_in_rect(frame, frame.bounds(), colour)
}

/// The pixels inside a rectangle, so two frames can be compared region by region.
fn region(frame: &Frame, rect: Rect) -> Vec<u32> {
    let mut out = Vec::new();
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            if let Some(p) = frame.pixel(x, y) {
                out.push(p.to_argb());
            }
        }
    }
    out
}

/// A shell with two Session + shelf sessions: "Photos" (gallery, editor) and "Dev"
/// (terminal, build). The most recent — Dev — has focus.
fn two_session_shell() -> (Shell, flow_shell::SessionId, flow_shell::SessionId) {
    let mut shell = Shell::new(800, 400);
    let photos = shell.add_session_opening(
        "Photos",
        vec![Openable::tool("gallery"), Openable::tool("editor")],
    );
    let dev = shell.add_session_opening(
        "Dev",
        vec![Openable::tool("terminal"), Openable::tool("build")],
    );
    (shell, photos, dev)
}

/// Focus the intent bar through the input model, exactly as a press would.
fn open_intent_bar(shell: &mut Shell) {
    let bar = shell.intent_bar_rect();
    assert!(
        !bar.is_empty(),
        "the Session + shelf feel has an intent bar"
    );
    assert!(
        shell.handle(Event::Pointer(Pointer::Press {
            x: bar.x + 4,
            y: bar.y + 4,
        })),
        "a press in the intent bar is consumed"
    );
    assert!(
        shell.is_intent_active(),
        "the press made it the input target"
    );
}

/// Type a query, one key at a time, through the input model.
fn type_query(shell: &mut Shell, query: &str) {
    for c in query.chars() {
        assert!(shell.handle(Event::Key(Key::Char(c))));
    }
    assert_eq!(
        shell.intent_query(),
        query,
        "the query accumulated as typed"
    );
}

#[test]
fn the_session_shelf_feel_paints_the_shelf_and_the_intent_bar() {
    let (shell, _, _) = two_session_shell();
    let frame = shell.paint();

    let bar = shell.intent_bar_rect();
    let shelf = shell.shelf_rect();
    assert!(!bar.is_empty() && !shelf.is_empty());

    // Each piece of chrome has its own colour, and it is actually on the frame.
    assert_eq!(frame.pixel(bar.x, bar.y), Some(palette::INTENT_BAR));
    assert_eq!(frame.pixel(shelf.x, shelf.y), Some(palette::SHELF));
    // The focused session's openables are drawn on the shelf as text.
    assert!(
        count_in_rect(&frame, shelf, palette::TEXT) > 0,
        "the shelf drew the focused session's openable labels"
    );
}

#[test]
fn a_query_moves_focus_and_returns_a_result() {
    let (mut shell, photos, dev) = two_session_shell();
    assert_eq!(
        shell.focused(),
        Some(dev),
        "the most recent session is focused"
    );

    let photos_surface = shell.sessions()[0].surface;
    let dev_surface = shell.sessions()[1].surface;

    open_intent_bar(&mut shell);
    type_query(&mut shell, "Photos");

    // The query is committed; focus must move to the match, and the frame must show it.
    let before = shell.paint();
    assert_eq!(
        before.pixel(photos_surface.x + 1, photos_surface.y + 1),
        Some(palette::SURFACE),
        "before the commit, Photos does not have focus"
    );

    assert!(shell.handle(Event::Key(Key::Enter)));
    assert_eq!(
        shell.focused(),
        Some(photos),
        "the query moved focus to the match"
    );
    assert!(
        !shell.is_intent_active(),
        "a match hands the keys back to the session"
    );

    let after = shell.paint();

    // The frame actually changed: the matched session gained focus, the previous lost it.
    assert_eq!(
        after.pixel(photos_surface.x + 1, photos_surface.y + 1),
        Some(palette::SURFACE_FOCUSED),
        "the matched session gained the focused surface"
    );
    assert_eq!(
        after.pixel(dev_surface.x + 1, dev_surface.y + 1),
        Some(palette::SURFACE),
        "the previous session lost the focused surface"
    );

    // The shelf now lists the matched session's openables, not the old focus's.
    let shelf = shell.shelf_rect();
    assert!(
        region(&before, shelf) != region(&after, shelf),
        "the shelf changed to the matched session's openables"
    );
    assert!(
        count_in_rect(&after, shelf, palette::TEXT) > 0,
        "the shelf drew the matched session's openable labels"
    );

    // The intent bar reflects the query that returned the result.
    let bar = shell.intent_bar_rect();
    assert!(
        count_in_rect(&after, bar, palette::TEXT) > 0,
        "the intent bar drew the query text"
    );
}

#[test]
fn a_query_with_no_match_claims_nothing() {
    // THE NEGATIVE CONTROL. A query nothing matches must not move focus, must not pretend
    // to have found a result, and must say so on the frame.
    let (mut shell, _, dev) = two_session_shell();
    assert_eq!(shell.focused(), Some(dev));

    let dev_surface = shell.sessions()[1].surface;
    let bar = shell.intent_bar_rect();

    open_intent_bar(&mut shell);
    type_query(&mut shell, "zzz-nothing-named-this");

    let before = shell.paint();
    assert!(shell.handle(Event::Key(Key::Enter)));

    assert!(shell.intent_no_match(), "the shell must report no match");
    assert_eq!(shell.focused(), Some(dev), "no session gained focus");
    assert!(
        shell.is_intent_active(),
        "the search field stays open to correct the query"
    );

    let after = shell.paint();
    assert!(
        count_in_rect(&after, bar, palette::NO_MATCH) > 0,
        "the frame says 'no match' rather than faking a result"
    );
    assert_eq!(
        after.pixel(dev_surface.x + 1, dev_surface.y + 1),
        before.pixel(dev_surface.x + 1, dev_surface.y + 1),
        "focus did not move, so the surface highlight is unchanged"
    );
}

#[test]
fn a_search_that_returned_a_fixed_result_would_fail() {
    // THE NEGATIVE CONTROL for the matcher itself (H17). Every other test would be passed
    // by a `match_query` that returned one session regardless of input; this one fails it.
    // Distinct inputs must map to distinct outcomes — and a non-match must be `None`.
    let (shell, photos, dev) = two_session_shell();
    let sessions = shell.sessions();

    assert_eq!(match_query("Photos", sessions), Some(photos));
    assert_eq!(match_query("Dev", sessions), Some(dev));
    // An openable name is a match too, and the first session in layout order wins.
    assert_eq!(match_query("terminal", sessions), Some(dev));
    // Nothing, and no match, are both `None` — never a fabricated hit.
    assert_eq!(match_query("", sessions), None);
    assert_eq!(match_query("   ", sessions), None);
    assert_eq!(match_query("zzz", sessions), None);
    assert_ne!(
        photos, dev,
        "the two sessions are distinct, or the contrast is vacuous"
    );
}

#[test]
fn the_other_feels_keep_the_frame_without_the_shelf_chrome() {
    // `paint` branches on the feel: Flow and Classic must NOT draw the Session + shelf
    // chrome, so the shelf and the intent bar exist only for the default feel.
    let mut classic = Shell::new(400, 300);
    classic.add_session_opening("Photos", vec![Openable::legacy_app("Photos")]);
    assert_eq!(classic.sessions()[0].model.feel(), Feel::Classic);
    assert!(
        classic.intent_bar_rect().is_empty(),
        "no intent bar on Classic"
    );
    assert!(classic.shelf_rect().is_empty(), "no shelf on Classic");

    let mut flow = Shell::new(400, 300);
    flow.add_session_opening("a letter", vec![]);
    assert_eq!(flow.sessions()[0].model.feel(), Feel::Flow);
    assert!(flow.intent_bar_rect().is_empty(), "no intent bar on Flow");
    assert!(flow.shelf_rect().is_empty(), "no shelf on Flow");

    for shell in [classic, flow] {
        let frame = shell.paint();
        assert_eq!(
            count_in_frame(&frame, palette::SHELF),
            0,
            "a non-shelf feel must not paint the shelf"
        );
        assert_eq!(
            count_in_frame(&frame, palette::INTENT_BAR),
            0,
            "a non-shelf feel must not paint the intent bar"
        );
    }
}

#[test]
fn keys_return_to_the_terminal_when_the_intent_bar_is_not_the_target() {
    // The two input targets stay distinct: a character goes to the terminal unless the
    // intent bar was pressed, so the existing input behaviour is the default.
    let mut shell = Shell::new(400, 300);
    shell.add_session("a terminal");

    assert!(shell.handle(Event::Key(Key::Char('A'))));
    assert_eq!(
        shell.sessions()[0].terminal().text(),
        "A",
        "the terminal, not the query, got the key"
    );
    assert_eq!(shell.intent_query(), "", "the query stayed empty");
}

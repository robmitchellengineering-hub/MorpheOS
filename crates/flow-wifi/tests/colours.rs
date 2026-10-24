//! Every colour on the screen that a test finds something by is a colour nothing else draws.
//!
//! The VM tests and the compositor's tests read verdicts off screendumps by colour: the
//! pointer is `palette::TEXT`, the picker is `colours::TITLE`, Firefox's start page is its
//! purple. On 2026-10-14 the bar's Wi-Fi icon was drawn in the pointer's colour, and
//! `test-shell.sh` saw a pointer before the mouse had moved (#66). So every colour the shell,
//! the picker and the test pages draw is listed here, and no two may be the same.
//!
//! **A new colour goes in this list**, with where it is drawn. The test pages' colours are the
//! ones the image scripts key on (`images/x86-qemu/test-*.sh`).

use flow_notice::notice::colours as notice;
use flow_shell::{Colour, palette};
use flow_timeline::timeline::colours as timeline;
use flow_wifi::picker::colours;

fn rgb(c: Colour) -> (u8, u8, u8) {
    let (_, r, g, b) = c.channels();
    (r, g, b)
}

#[test]
fn no_two_colours_a_test_reads_by_are_the_same() {
    let named: Vec<(&str, (u8, u8, u8))> = vec![
        // The shell (crates/flow-shell, drawn by flow-compositor).
        ("palette::BACKGROUND", rgb(palette::BACKGROUND)),
        ("palette::BAR", rgb(palette::BAR)),
        ("palette::BAR_BUTTON", rgb(palette::BAR_BUTTON)),
        ("palette::BAR_DIVIDER", rgb(palette::BAR_DIVIDER)),
        ("palette::BAR_ICON", rgb(palette::BAR_ICON)),
        ("palette::SURFACE", rgb(palette::SURFACE)),
        ("palette::SURFACE_FOCUSED", rgb(palette::SURFACE_FOCUSED)),
        ("palette::ACCENT", rgb(palette::ACCENT)),
        ("palette::TEXT", rgb(palette::TEXT)),
        ("palette::INTENT_BAR", rgb(palette::INTENT_BAR)),
        ("palette::SHELF", rgb(palette::SHELF)),
        ("palette::NO_MATCH", rgb(palette::NO_MATCH)),
        // The Wi-Fi picker (crates/flow-wifi/src/picker.rs).
        ("colours::BACKGROUND", rgb(colours::BACKGROUND)),
        ("colours::TITLE", rgb(colours::TITLE)),
        ("colours::TEXT", rgb(colours::TEXT)),
        ("colours::DIM", rgb(colours::DIM)),
        ("colours::SEPARATOR", rgb(colours::SEPARATOR)),
        ("colours::SELECTED", rgb(colours::SELECTED)),
        ("colours::FIELD", rgb(colours::FIELD)),
        ("colours::BUSY", rgb(colours::BUSY)),
        ("colours::OK", rgb(colours::OK)),
        ("colours::FAIL", rgb(colours::FAIL)),
        ("colours::IDLE", rgb(colours::IDLE)),
        // The rewind timeline (crates/flow-timeline/src/timeline.rs).
        ("timeline::BACKGROUND", rgb(timeline::BACKGROUND)),
        ("timeline::TITLE", rgb(timeline::TITLE)),
        ("timeline::TEXT", rgb(timeline::TEXT)),
        ("timeline::DIM", rgb(timeline::DIM)),
        ("timeline::SEPARATOR", rgb(timeline::SEPARATOR)),
        ("timeline::SELECTED", rgb(timeline::SELECTED)),
        ("timeline::BUSY", rgb(timeline::BUSY)),
        ("timeline::OK", rgb(timeline::OK)),
        ("timeline::FAIL", rgb(timeline::FAIL)),
        ("timeline::CONFIRM", rgb(timeline::CONFIRM)),
        ("timeline::IDLE", rgb(timeline::IDLE)),
        // The notice window (crates/flow-notice/src/notice.rs).
        ("notice::WARN", rgb(notice::WARN)),
        ("notice::OK", rgb(notice::OK)),
        ("notice::FAIL", rgb(notice::FAIL)),
        // What the image tests draw and look for.
        (
            "start page (overlay-browser start.html)",
            (0x5a, 0x3c, 0xb4),
        ),
        (
            "test page, top (test-browser.sh PAGE_A)",
            (0xc8, 0x1e, 0x78),
        ),
        ("test page, bottom (PAGE_B)", (0x1e, 0xc8, 0x78)),
        ("network page, top (NET_A)", (0x32, 0x64, 0xc8)),
        ("network page, bottom (NET_B)", (0xc8, 0xa0, 0x1e)),
        ("demo window (test-shell.sh IDLE)", (0x00, 0xa0, 0xff)),
        ("demo window clicked (CLICKED)", (0xc0, 0x00, 0xc0)),
        ("demo window keyed (KEYED)", (0x00, 0xc0, 0x60)),
    ];
    // The tests match a colour within 3 per channel (test-browser.sh `near`), so two colours
    // closer than that are the same colour to them.
    let close = |a: (u8, u8, u8), b: (u8, u8, u8)| {
        a.0.abs_diff(b.0) <= 3 && a.1.abs_diff(b.1) <= 3 && a.2.abs_diff(b.2) <= 3
    };
    let mut clashes = Vec::new();
    for (i, (na, a)) in named.iter().enumerate() {
        for (nb, b) in &named[i + 1..] {
            if close(*a, *b) {
                clashes.push(format!(
                    "{na} and {nb} are both #{:02x}{:02x}{:02x}",
                    a.0, a.1, a.2
                ));
            }
        }
    }
    assert!(
        clashes.is_empty(),
        "a test that finds one of these by colour would find the other: {clashes:#?}"
    );
}

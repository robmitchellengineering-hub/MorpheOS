//! Whether a frame is evidence, and the negative control that proves the check can fail.

use flow_shell::{Colour, Frame, Rect, Shell, frame_evidence, palette};

#[test]
fn a_painted_shell_is_evidence() {
    let mut shell = Shell::new(320, 200);
    shell.add_session("a terminal");
    let frame = shell.paint();
    let evidence = frame_evidence(&frame);
    assert!(evidence.passed(), "{}", evidence.verdict());
    assert!(
        evidence.distinct_colours >= 3,
        "a background, a bar and a surface at least: {}",
        evidence.distinct_colours
    );
}

#[test]
fn a_single_colour_frame_is_not_evidence() {
    // THE NEGATIVE CONTROL, and the reason it is here rather than in a comment: without
    // it, `frame_evidence` could return `passed: true` unconditionally and every other test
    // would stay green. That is hazard H17 - a check that cannot fail reads exactly like
    // a check that passed - and it is how this repository has been burned before.
    let mut frame = Frame::new(64, 64);
    frame.clear(Colour::rgb(0, 0, 0));
    let evidence = frame_evidence(&frame);
    assert_eq!(evidence.distinct_colours, 1);
    assert!(
        !evidence.passed(),
        "a blank frame must fail: {}",
        evidence.verdict()
    );
    assert!(
        evidence.verdict().contains("FAIL"),
        "{}",
        evidence.verdict()
    );
}

#[test]
fn focus_moves_the_bright_thing() {
    // Behaviour, not shape. A frame that ignored focus would still evidence the Rob evidence and
    // fail here, which is the point of asking a second question.
    let mut shell = Shell::new(400, 300);
    let first = shell.add_session("first");
    let second = shell.add_session("second");
    assert_eq!(shell.focused(), Some(second), "adding a session focuses it");

    let first_surface = shell.sessions()[0].surface;
    let second_surface = shell.sessions()[1].surface;

    let frame = shell.paint();
    assert_eq!(
        frame.pixel(second_surface.x + 1, second_surface.y + 1),
        Some(palette::SURFACE_FOCUSED)
    );
    assert_eq!(
        frame.pixel(first_surface.x + 1, first_surface.y + 1),
        Some(palette::SURFACE)
    );

    assert!(shell.focus(first));
    let frame = shell.paint();
    assert_eq!(
        frame.pixel(first_surface.x + 1, first_surface.y + 1),
        Some(palette::SURFACE_FOCUSED)
    );
    assert_eq!(
        frame.pixel(second_surface.x + 1, second_surface.y + 1),
        Some(palette::SURFACE)
    );
}

#[test]
fn closing_the_focused_session_moves_focus_somewhere_real() {
    let mut shell = Shell::new(400, 300);
    let first = shell.add_session("first");
    let second = shell.add_session("second");

    assert!(shell.close_session(second));
    assert_eq!(shell.focused(), Some(first));
    assert!(
        !shell.close_session(second),
        "closing it twice reports that it was not there"
    );

    let area = shell.content_area();
    let surface = shell.sessions()[0].surface;
    assert!(
        surface.h > 0,
        "the survivor still has a surface: {surface:?}"
    );
    assert!(surface.x >= area.x && surface.right() <= area.right());
    assert!(surface.y >= area.y && surface.bottom() <= area.bottom());
}

#[test]
fn sessions_do_not_overlap() {
    let mut shell = Shell::new(1280, 800);
    for n in 0..3 {
        shell.add_session(&format!("session {n}"));
    }
    let surfaces: Vec<Rect> = shell.sessions().iter().map(|s| s.surface).collect();
    for (i, a) in surfaces.iter().enumerate() {
        for b in surfaces.iter().skip(i + 1) {
            assert!(
                a.intersect(b).is_none(),
                "two sessions share pixels: {a:?} and {b:?}"
            );
        }
    }
}

#[test]
fn an_out_of_bounds_fill_is_a_no_op_and_not_a_panic() {
    let mut frame = Frame::new(32, 32);
    frame.clear(Colour::rgb(1, 2, 3));
    frame.fill_rect(Rect::new(1000, 1000, 10, 10), Colour::rgb(9, 9, 9));
    frame.fill_rect(Rect::new(16, 16, 1000, 1000), Colour::rgb(9, 9, 9));

    assert_eq!(frame.pixel(0, 0), Some(Colour::rgb(1, 2, 3)));
    assert_eq!(frame.pixel(20, 20), Some(Colour::rgb(9, 9, 9)));
    assert_eq!(frame.pixel(64, 64), None, "outside the frame is None");
}

#[test]
fn a_frame_written_as_a_ppm_is_readable() {
    let mut shell = Shell::new(8, 4);
    shell.add_session("x");
    let ppm = shell.paint().to_ppm();
    let header = b"P6\n8 4\n255\n";
    assert!(
        ppm.starts_with(header),
        "header was {:?}",
        String::from_utf8_lossy(&ppm[..header.len().min(ppm.len())])
    );
    assert_eq!(ppm.len(), header.len() + 8 * 4 * 3);
}

#[test]
fn an_empty_shell_still_draws_something() {
    // A shell with no sessions is not blank - it has a bar and a background - so this is
    // the weakest frame the shell can produce, and it is still evidence.
    let shell = Shell::new(64, 48);
    let evidence = frame_evidence(&shell.paint());
    assert!(evidence.passed(), "{}", evidence.verdict());
}

#[test]
fn a_frame_written_as_a_png_is_a_real_png() {
    // The frame exists to be LOOKED AT, and `convert` is not on the CI runner - so the
    // encoding is this crate's job and it has to be correct. The CRC over IEND is fixed by
    // the format, so asserting it catches a broken chunk writer rather than only a short
    // file. A real decoder accepting the output is checked separately, on the frame the
    // binary writes.
    let mut shell = Shell::new(8, 4);
    shell.add_session("x");
    let png = shell.paint().to_png();

    assert_eq!(
        &png[..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        "PNG signature"
    );
    assert_eq!(&png[12..16], b"IHDR", "IHDR is the first chunk");
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!((width, height), (8, 4));
    assert_eq!(png[24], 8, "bit depth");
    assert_eq!(png[25], 2, "colour type: truecolour");
    assert_eq!(png[26], 0, "compression: deflate");
    assert_eq!(png[27], 0, "filter method");
    assert_eq!(png[28], 0, "interlace: none");
    assert_eq!(
        &png[png.len() - 12..],
        &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82],
        "IEND with the CRC the format fixes"
    );
    assert!(png.len() > 8 * 4 * 3, "the pixels are in there");
}

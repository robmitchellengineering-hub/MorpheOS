//! The boot splash: the MorpheOS mark on the brand background, shown for the first
//! moments after the compositor starts (doc 20, E19), as pure logic.
//!
//! WHY THIS IS SEPARATE FROM THE COMPOSITOR. When the splash is showing and how far it
//! has faded are **decisions** — pure functions of the time since the compositor
//! started, with no Wayland, no renderer and no Smithay. The logo itself is a fixed
//! raster (`logo-mark.argb`, rasterized from `design/logo-1-mark.svg` by
//! `scripts/rasterize-logo-mark.py`) and embedded here, so the compositor paints it with
//! nothing but the brand colours. Keeping the timing and the pixels here means both are
//! unit-tested on any machine, the same way [`crate::wm`] is.
//!
//! The splash frame is a plain [`Frame`], so the live display shows it through the exact
//! same `render` call the shell uses, and the headless tests read it back through the
//! exact same PPM/PNG writer.

use std::time::Duration;

use flow_shell::{Colour, Frame};

/// How long the splash stays fully opaque (doc 20, E19: "about 2.5 seconds").
pub const HOLD: Duration = Duration::from_millis(2500);
/// How long it then takes to fade out (a short fade is enough).
pub const FADE: Duration = Duration::from_millis(500);
/// The whole splash, from the first pixel to the shell taking over.
pub const TOTAL: Duration = HOLD.saturating_add(FADE);

/// The brand background the mark sits on (design/README.md: "the depth the OS lives in").
pub const BRAND_BLACK: Colour = Colour::rgb(0x05, 0x07, 0x0d); // #05070d
/// Matrix green — the living mark (design/README.md: "the voice").
pub const MATRIX_GREEN: Colour = Colour::rgb(0x00, 0xff, 0x41); // #00ff41

/// The embedded mark: a square raster, one ARGB pixel (a, r, g, b) after another.
const MARK: &[u8] = include_bytes!("logo-mark.argb");
/// The mark's side length, in pixels.
const MARK_SIZE: u32 = 256;

/// Whether the splash is still showing, and how far it has faded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Splash {
    /// True from the compositor starting until the fade has finished.
    pub showing: bool,
    /// 1.0 through the hold, falling to 0.0 by the end of the fade.
    pub alpha: f32,
}

/// The splash at `elapsed` since the compositor started.
#[must_use]
pub fn state(elapsed: Duration) -> Splash {
    let t = elapsed.as_secs_f32();
    let hold = HOLD.as_secs_f32();
    let fade = FADE.as_secs_f32();
    if t < hold {
        Splash {
            showing: true,
            alpha: 1.0,
        }
    } else if t < hold + fade {
        Splash {
            showing: true,
            alpha: 1.0 - (t - hold) / fade,
        }
    } else {
        Splash {
            showing: false,
            alpha: 0.0,
        }
    }
}

/// The splash frame: the brand background with the mark centred on it, faded by `alpha`.
///
/// The frame is opaque, so it goes straight to the display (the live path shows
/// [`Frame::pixels`]) or to the headless tests' PPM/PNG, exactly like a shell frame. The
/// mark is smaller than every screen the compositor renders, so it is never clipped; the
/// loop still skips anything that would fall outside, so a tiny frame degrades rather
/// than panics.
#[must_use]
pub fn frame(width: u32, height: u32, alpha: f32) -> Frame {
    let alpha = alpha.clamp(0.0, 1.0);
    let mut pixels = vec![BRAND_BLACK.to_argb(); width as usize * height as usize];
    if alpha > 0.0 {
        let (_, br, bg, bb) = BRAND_BLACK.channels();
        let ox = width.saturating_sub(MARK_SIZE) / 2;
        let oy = height.saturating_sub(MARK_SIZE) / 2;
        for row in 0..MARK_SIZE {
            let y = oy + row;
            if y >= height {
                continue;
            }
            for col in 0..MARK_SIZE {
                let x = ox + col;
                if x >= width {
                    continue;
                }
                let i = ((row * MARK_SIZE + col) * 4) as usize;
                let a = MARK[i];
                if a == 0 {
                    continue;
                }
                let r = MARK[i + 1];
                let g = MARK[i + 2];
                let b = MARK[i + 3];
                // Blend the mark over the background by its own coverage and the fade.
                let eff = f32::from(a) / 255.0 * alpha;
                let mix = |c: u8, base: u8| {
                    (f32::from(c) * eff + f32::from(base) * (1.0 - eff)).round() as u8
                };
                let pixel = Colour::rgb(mix(r, br), mix(g, bg), mix(b, bb));
                pixels[(y * width + x) as usize] = pixel.to_argb();
            }
        }
    }
    Frame::from_pixels(width, height, pixels).expect("the count matches the size")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_showing_through_the_hold() {
        assert_eq!(
            state(Duration::ZERO),
            Splash {
                showing: true,
                alpha: 1.0,
            }
        );
        assert_eq!(
            state(HOLD / 2),
            Splash {
                showing: true,
                alpha: 1.0,
            }
        );
        assert_eq!(
            state(HOLD),
            Splash {
                showing: true,
                alpha: 1.0,
            }
        );
    }

    #[test]
    fn fades_linearly_to_zero() {
        let halfway = state(HOLD + FADE / 2);
        assert!(halfway.showing);
        assert!(
            (halfway.alpha - 0.5).abs() < 0.001,
            "halfway is {halfway:?}"
        );
        let near_end = state(TOTAL - Duration::from_millis(1));
        assert!(near_end.showing);
        assert!(near_end.alpha < 0.01, "nearly gone is {near_end:?}");
    }

    #[test]
    fn gone_once_the_fade_is_over() {
        assert_eq!(
            state(TOTAL),
            Splash {
                showing: false,
                alpha: 0.0,
            }
        );
        assert!(!state(TOTAL + Duration::from_secs(60)).showing);
    }

    #[test]
    fn the_frame_is_the_mark_in_green_on_brand_black() {
        let f = frame(640, 480, 1.0);
        // A corner the mark cannot reach is brand black.
        assert_eq!(f.pixel(0, 0), Some(BRAND_BLACK));
        // The mark's centre is fully matrix green.
        assert_eq!(f.pixel(320, 240), Some(MATRIX_GREEN));
        // And there is a real mark there, not a couple of stray pixels.
        let green = f
            .pixels()
            .iter()
            .filter(|p| **p == MATRIX_GREEN.to_argb())
            .count();
        assert!(
            green > 5000,
            "the mark should be thousands of green pixels, found {green}"
        );
    }

    #[test]
    fn a_faded_frame_keeps_the_background_and_dims_the_mark() {
        let faded = frame(640, 480, 0.5);
        // The background is unchanged.
        assert_eq!(faded.pixel(0, 0), Some(BRAND_BLACK));
        // The centre is a blend of green and black, dimmer than the full mark.
        let (_, r, g, b) = faded.pixel(320, 240).unwrap().channels();
        assert!(
            g < 255 && g > 0,
            "a half-faded centre should dim, got {r},{g},{b}"
        );
        // No fully-green pixel survives a 0.5 fade of an opaque mark.
        assert!(!faded.pixels().iter().any(|p| *p == MATRIX_GREEN.to_argb()));
    }
}

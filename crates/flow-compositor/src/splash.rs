//! The boot splash: the animated MorpheOS geomesh (doc 20, E19), as pure logic.
//!
//! WHY THIS IS SEPARATE FROM THE COMPOSITOR. The splash is a decision — what the shell's
//! first moments look like — and it is a pure function of the time since the compositor
//! started, with no Wayland, no renderer and no Smithay. It is the `login.html` scene
//! (`design/login.html`) ported to software: the wireframe icosahedron draws itself in
//! while phasing blue -> cyan -> pink -> purple -> matrix green, three data rings orbit it,
//! a twinkling starfield sits behind, the wordmark rises below, a green settle ripple
//! fires the moment the mesh lands, and then the whole splash fades out to the shell.
//! Keeping the timing, the geometry and the pixels here means they are unit-tested on any
//! machine, the same way [`crate::wm`] is.
//!
//! The frame is a plain [`Frame`], so the live display shows it through the exact same
//! `render` call the shell uses, and the headless tests read it back through the exact
//! same PPM/PNG writer.
//!
//! Approximations vs the WebGL original, all in software (no GPU, no Three.js): the cable
//! rings are simple coloured line rings (no tube or travelling bulge), the settle ripple
//! and the tip dot are flat discs with a radial falloff instead of bloom sprites, colour
//! stops interpolate in sRGB instead of linear light, and the starfield does not slowly
//! rotate. Everything else — the golden-ratio icosahedron, the camera orbit, the edge
//! reveal order, the ease-out, the phasing stops — matches `login.html`.

use std::time::Duration;

use flow_shell::{Colour, Frame};

// ---- the clock (doc 20, E19: one 4/4 bar at 42 BPM) ----

/// The bar's tempo. One bar of 4/4 at this BPM is how long the mesh takes to draw.
pub const BPM: f32 = 42.0;
/// How long the mesh takes to draw itself into completion, in seconds.
pub const DRAW_SEC: f32 = 4.0 * 60.0 / BPM;
/// How long the completed mesh holds (the settle ripple fires as the bar ends).
pub const SETTLE_SEC: f32 = 1.0;
/// How long the whole splash then takes to fade out to the shell.
pub const FADE_SEC: f32 = 0.5;
/// The whole splash, from the first pixel to the shell taking over.
pub const TOTAL_SEC: f32 = DRAW_SEC + SETTLE_SEC + FADE_SEC;

/// Whether the compositor shows this software port at boot. It does NOT (E19 rework): the real
/// The native boot splash (E19): the compositor itself draws the geomesh as the machine
/// boots — not Firefox. `design/login.html` remains the design reference this port matches.
pub fn shown_at_boot() -> bool {
    true
}

/// The brand background the mark sits on (design/README.md: "the depth the OS lives in").
pub const BRAND_BLACK: Colour = Colour::rgb(0x05, 0x07, 0x0d); // #05070d
/// Matrix green — where the mark lands (design/README.md: "the voice").
pub const MATRIX_GREEN: Colour = Colour::rgb(0x00, 0xff, 0x41); // #00ff41

/// The geomesh's radius, in the scene's world units.
const GEOMESH_R: f32 = 2.4;
/// The camera's vertical field of view, in degrees.
const FOV: f32 = 50.0;
/// How long the settle ripple expands and fades for, once the mesh lands.
const RIPPLE_SEC: f32 = 0.7;
/// The number of twinkling stars.
const STARS: usize = 420;

/// The embedded wordmark (`design/logo-3-wordmark.svg`, rasterized by
/// `scripts/rasterize-wordmark.py`): "Morphe" in the green gradient and "OS" in white,
/// with the brand background keyed out to transparent, one ARGB pixel (a, r, g, b) each.
const WORDMARK: &[u8] = include_bytes!("logo-wordmark.argb");
const WORDMARK_W: u32 = 400;
const WORDMARK_H: u32 = 120;

/// The embedded tagline (`design/logo-tagline.svg`, rasterized by `scripts/rasterize-tagline.py`):
/// "Never breaks · Heals itself" in the muted green, background keyed out to transparent.
const TAGLINE: &[u8] = include_bytes!("logo-tagline.argb");
const TAGLINE_W: u32 = 400;
const TAGLINE_H: u32 = 30;

/// Whether the splash is still showing, and how far it has faded out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Splash {
    /// True from the compositor starting until the fade has finished.
    pub showing: bool,
    /// 1.0 through the draw and settle, falling to 0.0 by the end of the fade.
    pub alpha: f32,
}

/// The splash at `elapsed` since the compositor started.
#[must_use]
pub fn state(elapsed: Duration) -> Splash {
    let t = elapsed.as_secs_f32();
    if t < DRAW_SEC + SETTLE_SEC {
        Splash {
            showing: true,
            alpha: 1.0,
        }
    } else if t < TOTAL_SEC {
        Splash {
            showing: true,
            alpha: 1.0 - (t - DRAW_SEC - SETTLE_SEC) / FADE_SEC,
        }
    } else {
        Splash {
            showing: false,
            alpha: 0.0,
        }
    }
}

/// The whole splash as a [`Duration`], for the headless shell frame to render after it.
#[must_use]
pub fn total() -> Duration {
    Duration::from_secs_f32(TOTAL_SEC)
}

// ---- pure animation decisions ----

/// The draw progress `(p, k)`: `p` is time clamped to the bar, `k` is its ease-out.
#[must_use]
pub fn progress(t: f32) -> (f32, f32) {
    let p = (t / DRAW_SEC).clamp(0.0, 1.0);
    (p, ease_out(p))
}

/// The ease-out the whole draw uses (as `login.html`'s `easeOut`).
#[must_use]
pub fn ease_out(p: f32) -> f32 {
    1.0 - (1.0 - p).powi(3)
}

/// The colour stops the mesh phases through, by draw progress.
const STOPS: [(f32, (u8, u8, u8)); 6] = [
    (0.00, (0x4d, 0x6b, 0xfe)), // DeepSeek blue
    (0.22, (0x06, 0xb6, 0xd4)), // cyan
    (0.44, (0xff, 0x2f, 0xd6)), // neon pink
    (0.66, (0xa7, 0x8b, 0xfa)), // purple
    (0.86, (0x00, 0xff, 0x41)), // matrix green — lands here
    (1.00, (0x00, 0xff, 0x41)),
];

/// The mesh line colour at draw progress `p`.
#[must_use]
pub fn mesh_colour(p: f32) -> (u8, u8, u8) {
    let mut i = 0;
    while i < STOPS.len() - 2 && p > STOPS[i + 1].0 {
        i += 1;
    }
    let seg = STOPS[i + 1].0 - STOPS[i].0;
    let f = if seg <= 0.0 {
        0.0
    } else {
        ((p - STOPS[i].0) / seg).clamp(0.0, 1.0)
    };
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * f).round() as u8;
    (
        mix(STOPS[i].1.0, STOPS[i + 1].1.0),
        mix(STOPS[i].1.1, STOPS[i + 1].1.1),
        mix(STOPS[i].1.2, STOPS[i + 1].1.2),
    )
}

// ---- geometry ----

#[derive(Clone, Copy, Debug)]
struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

impl V3 {
    fn sub(self, o: V3) -> V3 {
        V3 {
            x: self.x - o.x,
            y: self.y - o.y,
            z: self.z - o.z,
        }
    }
    fn add(self, o: V3) -> V3 {
        V3 {
            x: self.x + o.x,
            y: self.y + o.y,
            z: self.z + o.z,
        }
    }
    fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn cross(self, o: V3) -> V3 {
        V3 {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
        }
    }
    fn scale(self, s: f32) -> V3 {
        V3 {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
    fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    fn len_sq(self) -> f32 {
        self.dot(self)
    }
    fn normalize(self) -> V3 {
        let l = self.len();
        if l > 0.0 { self.scale(1.0 / l) } else { self }
    }
}

/// The standard icosahedron's 12 vertices (the golden-ratio ones), on a sphere of radius
/// [`GEOMESH_R`].
#[must_use]
fn icosahedron_vertices() -> [V3; 12] {
    let phi = (1.0 + 5.0f32.sqrt()) / 2.0;
    [
        (0.0, 1.0, phi),
        (0.0, -1.0, phi),
        (0.0, 1.0, -phi),
        (0.0, -1.0, -phi),
        (1.0, phi, 0.0),
        (-1.0, phi, 0.0),
        (1.0, -phi, 0.0),
        (-1.0, -phi, 0.0),
        (phi, 0.0, 1.0),
        (phi, 0.0, -1.0),
        (-phi, 0.0, 1.0),
        (-phi, 0.0, -1.0),
    ]
    .map(|(x, y, z)| V3 { x, y, z }.normalize().scale(GEOMESH_R))
}

/// The icosahedron's 30 edges, as vertex-index pairs, in a deterministic order.
#[must_use]
fn icosahedron_edges() -> Vec<(usize, usize)> {
    let v = icosahedron_vertices();
    let mut min = f32::INFINITY;
    for i in 0..12 {
        for j in i + 1..12 {
            min = min.min(v[i].sub(v[j]).len_sq());
        }
    }
    let mut edges = Vec::with_capacity(30);
    for i in 0..12 {
        for j in i + 1..12 {
            if v[i].sub(v[j]).len_sq() <= min * 1.0001 {
                edges.push((i, j));
            }
        }
    }
    edges
}

/// The mesh's spin, as `login.html` sets `geomesh.rotation` (Three.js `XYZ` Euler, so the
/// Y rotation is applied first, then X).
#[must_use]
fn rotate_mesh(v: V3, rot_x: f32, rot_y: f32) -> V3 {
    let (cx, sx) = (rot_x.cos(), rot_x.sin());
    let (cy, sy) = (rot_y.cos(), rot_y.sin());
    // v1 = Ry * v
    let v1 = V3 {
        x: cy * v.x + sy * v.z,
        y: v.y,
        z: -sy * v.x + cy * v.z,
    };
    // v2 = Rx * v1
    V3 {
        x: v1.x,
        y: cx * v1.y - sx * v1.z,
        z: sx * v1.y + cx * v1.z,
    }
}

/// Rotate a point about an arbitrary axis by an angle (Rodrigues' rotation).
#[must_use]
fn rotate_axis(v: V3, axis: V3, angle: f32) -> V3 {
    let (c, s) = (angle.cos(), angle.sin());
    let k = axis.normalize();
    // v' = v cos + (k x v) sin + k (k . v) (1 - cos)
    v.scale(c)
        .add(k.cross(v).scale(s))
        .add(k.scale(k.dot(v) * (1.0 - c)))
}

// ---- the camera ----

/// Where the camera sits at time `t` (it orbits the origin).
#[must_use]
fn camera_eye(t: f32) -> V3 {
    V3 {
        x: (t * 0.30).sin() * 0.4,
        y: 0.5 + (t * 0.22).sin() * 0.22,
        z: 10.0,
    }
}

/// A perspective camera looking at the origin, built for one frame.
struct Cam {
    eye: V3,
    right: V3,
    up: V3,
    forward: V3,
    focal: f32,
    cx: f32,
    cy: f32,
}

impl Cam {
    fn new(t: f32, width: u32, height: u32) -> Self {
        let eye = camera_eye(t);
        let forward = eye.scale(-1.0).normalize();
        let world_up = V3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        };
        let right = forward.cross(world_up).normalize();
        let up = right.cross(forward);
        let focal = (height as f32 * 0.5) / (FOV.to_radians() * 0.5).tan();
        Self {
            eye,
            right,
            up,
            forward,
            focal,
            cx: width as f32 * 0.5,
            cy: height as f32 * 0.5,
        }
    }

    /// Project a world point to `(screen_x, screen_y, camera_z)`, or `None` behind the eye.
    fn project(&self, v: V3) -> Option<(f32, f32, f32)> {
        let rel = v.sub(self.eye);
        let z = rel.dot(self.forward);
        if z <= 0.001 {
            return None;
        }
        let x = rel.dot(self.right);
        let y = rel.dot(self.up);
        Some((
            self.cx + self.focal * x / z,
            self.cy - self.focal * y / z,
            z,
        ))
    }
}

// ---- a deterministic star field ----

/// A tiny xorshift, seeded once so the star field is reproducible.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 16_777_216.0
    }
}

// ---- the frame ----

/// The splash frame at `elapsed` since the compositor started: the whole scene, faded out
/// at the end. Opaque, so it goes straight to the display or the PPM/PNG writer.
#[must_use]
pub fn frame(width: u32, height: u32, elapsed: Duration) -> Frame {
    let fade = state(elapsed).alpha;
    // Clamp the scene to the end of the settle, so the completed mark holds while the
    // frame fades out underneath it.
    let t = elapsed.as_secs_f32().min(DRAW_SEC + SETTLE_SEC);
    let mut pixels = vec![BRAND_BLACK.to_argb(); width as usize * height as usize];
    let cam = Cam::new(t, width, height);
    let mut canvas = Canvas {
        px: &mut pixels,
        width,
        height,
    };

    draw_stars(&mut canvas, &cam, t);
    draw_rings(&mut canvas, &cam, t);
    draw_geomesh(&mut canvas, &cam, t);
    draw_settle(&mut canvas, &cam, t);
    draw_wordmark(&mut canvas, t);
    draw_tagline(&mut canvas, t);

    if fade < 1.0 {
        let (_, br, bg, bb) = BRAND_BLACK.channels();
        for p in &mut pixels {
            let (_, r, g, b) = channels(*p);
            let mix = |c: u8, base: u8| {
                (f32::from(c) * fade + f32::from(base) * (1.0 - fade)).round() as u8
            };
            *p = Colour::rgb(mix(r, br), mix(g, bg), mix(b, bb)).to_argb();
        }
    }

    Frame::from_pixels(width, height, pixels).expect("the count matches the size")
}

/// A pixel's channels, as `(a, r, g, b)` from its ARGB word.
#[must_use]
fn channels(pixel: u32) -> (u8, u8, u8, u8) {
    (
        ((pixel >> 24) & 0xff) as u8,
        ((pixel >> 16) & 0xff) as u8,
        ((pixel >> 8) & 0xff) as u8,
        (pixel & 0xff) as u8,
    )
}

/// The pixel buffer the scene is painted into, with the primitives the scene needs.
struct Canvas<'a> {
    px: &'a mut [u32],
    width: u32,
    height: u32,
}

impl Canvas<'_> {
    /// A Bresenham line, clipped to the frame, in one solid colour.
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Colour) {
        let (mut x0, mut y0) = (x0.round() as i32, y0.round() as i32);
        let (x1, y1) = (x1.round() as i32, y1.round() as i32);
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 >= 0 && y0 >= 0 && (x0 as u32) < self.width && (y0 as u32) < self.height {
                self.px[(y0 as u32 * self.width + x0 as u32) as usize] = c.to_argb();
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Composite one pixel `(r, g, b)` at `alpha` over what is already there.
    fn blend(&mut self, x: i32, y: i32, rgb: (u8, u8, u8), alpha: f32) {
        if alpha <= 0.0 || x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return;
        }
        let i = (y as u32 * self.width + x as u32) as usize;
        let (_, dr, dg, db) = channels(self.px[i]);
        let mix =
            |c: u8, d: u8| (f32::from(c) * alpha + f32::from(d) * (1.0 - alpha)).round() as u8;
        self.px[i] = Colour::rgb(mix(rgb.0, dr), mix(rgb.1, dg), mix(rgb.2, db)).to_argb();
    }

    /// A filled disc, composited at `alpha` (a star, the tip dot, the settle glow).
    fn disc(&mut self, cx: f32, cy: f32, r: f32, rgb: (u8, u8, u8), alpha: f32) {
        let (cix, ciy) = (cx.round() as i32, cy.round() as i32);
        let rr = r.round() as i32;
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                if dx * dx + dy * dy <= rr * rr {
                    self.blend(cix + dx, ciy + dy, rgb, alpha);
                }
            }
        }
    }

    /// A soft disc with a linear falloff toward its edge (the settle ripple's glow).
    fn glow(&mut self, cx: f32, cy: f32, r: f32, rgb: (u8, u8, u8), alpha: f32) {
        let (cix, ciy) = (cx.round() as i32, cy.round() as i32);
        let rr = r.round() as i32;
        for dy in -rr..=rr {
            for dx in -rr..=rr {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d <= r {
                    self.blend(cix + dx, ciy + dy, rgb, alpha * (1.0 - d / r));
                }
            }
        }
    }
}

/// One of the three data rings.
struct Ring {
    radius: f32,
    /// The tube's cross-section radius (the cable's thickness, in world units).
    tube_r: f32,
    colour: (u8, u8, u8),
    spin: f32,
    precess: f32,
    phase: f32,
    /// How fast the travelling bulge moves around the ring, in rings per second.
    speed: f32,
    /// Where the bulge starts, in rings (0..1).
    ball_t: f32,
}

/// The twinkling star field (fixed seed; a few stars are green).
fn draw_stars(canvas: &mut Canvas<'_>, cam: &Cam, t: f32) {
    let mut rng = Rng(0x9e37_79b9);
    for _ in 0..STARS {
        let rr = 14.0 + rng.next() * 40.0;
        let th = rng.next() * std::f32::consts::TAU;
        let ph = (2.0 * rng.next() - 1.0).acos();
        let pos = V3 {
            x: rr * ph.sin() * th.cos(),
            y: rr * ph.sin() * th.sin(),
            z: rr * ph.cos(),
        };
        let phase = rng.next();
        let size = 0.8 + rng.next() * 1.5;
        let green = rng.next() < 0.35;
        let rgb = if green {
            (166, 255, 184)
        } else {
            (255, 255, 255)
        };
        let Some((sx, sy, z)) = cam.project(pos) else {
            continue;
        };
        // `gl_PointSize = aSize * (62 / -mv.z)`, clamped so a near star is a dot, not a blob.
        let radius = (size * (62.0 / z) * 0.5).clamp(1.0, 3.0);
        let tw = 0.78 + 0.22 * (t * 2.4 + phase * std::f32::consts::TAU).sin();
        // Keep the twinkle below full so the wordmark's white stays the only pure white.
        canvas.disc(sx, sy, radius, rgb, tw * 0.9);
    }
}

/// The three data rings (magenta, cyan, blue), each a cable tube that spins and precesses,
/// with the login.html flow and travelling bulge: the tube's colour carries a moving wave
/// and a bright white pulse rides around it.
fn draw_rings(canvas: &mut Canvas<'_>, cam: &Cam, t: f32) {
    const RINGS: [Ring; 3] = [
        Ring {
            radius: 3.0,
            tube_r: 0.045,
            colour: (0xff, 0x2f, 0xd6),
            spin: 0.9,
            precess: 0.5,
            phase: 0.0,
            speed: 0.16,
            ball_t: 0.0,
        },
        Ring {
            radius: 3.6,
            tube_r: 0.038,
            colour: (0x06, 0xb6, 0xd4),
            spin: -0.7,
            precess: -0.4,
            phase: 1.7,
            speed: -0.12,
            ball_t: 0.55,
        },
        Ring {
            radius: 4.15,
            tube_r: 0.032,
            colour: (0x4d, 0x6b, 0xfe),
            spin: 0.65,
            precess: 0.55,
            phase: 3.4,
            speed: 0.11,
            ball_t: 0.2,
        },
    ];
    let scale = 0.8 + 0.2 * ease_out((t / 1.4).min(1.0));
    for ring in RINGS {
        let pa = t * ring.precess + ring.phase;
        let axis = V3 {
            x: pa.sin(),
            y: pa.cos() * 0.55,
            z: pa.cos() * 0.8,
        }
        .normalize();
        let angle = t * ring.spin;
        // The travelling bulge's position around the ring, in rings (0..1).
        let pulse_t = (ring.ball_t + t * ring.speed).rem_euclid(1.0);
        for i in 0..64 {
            let ang = i as f32 / 64.0 * std::f32::consts::TAU;
            let pt = rotate_axis(
                V3 {
                    x: ring.radius * ang.cos(),
                    y: ring.radius * ang.sin(),
                    z: 0.0,
                },
                axis,
                angle,
            )
            .scale(scale);
            let Some((sx, sy, z)) = cam.project(pt) else {
                continue;
            };
            // The shader's `flow` (a wave moving along the tube) and `pulse` (the white
            // bulge), as in login.html's fragment shader; the shortest arc wraps the pulse
            // cleanly around the seam.
            let along = ang / std::f32::consts::TAU;
            let flow = 0.5 + 0.5 * (along * 18.0 - t * 2.6).sin();
            let dp = (along - pulse_t).rem_euclid(1.0);
            let dp = dp.min(1.0 - dp);
            let pulse = (-(dp * 2.0).powi(2)).exp();
            let base = 0.24 + flow * 0.17;
            let (r, g, b) = ring.colour;
            let cr = (f32::from(r) * base + 255.0 * pulse * 0.90).min(255.0) as u8;
            let cg = (f32::from(g) * base + 255.0 * pulse * 0.90).min(255.0) as u8;
            let cb = (f32::from(b) * base + 255.0 * pulse * 0.90).min(255.0) as u8;
            // The tube's screen radius (diameter), with a floor so the thin cables stay
            // visible; the glow gives the additive-blended cable its soft falloff.
            let sr = (2.0 * ring.tube_r * 62.0 / z * scale).max(0.9);
            canvas.glow(sx, sy, sr, (cr, cg, cb), 0.8);
        }
    }
}

/// The geomesh: its edges draw themselves in, phasing toward green, with a bright tip on
/// the edge being drawn.
fn draw_geomesh(canvas: &mut Canvas<'_>, cam: &Cam, t: f32) {
    let (p, k) = progress(t);
    let verts = icosahedron_vertices();
    let edges = icosahedron_edges();
    let rot_y = -t * (0.3 + 0.55 * (1.0 - k));
    let rot_x = (t * 0.35).sin() * 0.2;
    let scale = mesh_scale(t);
    let projected: Vec<Option<(f32, f32, f32)>> = verts
        .iter()
        .map(|v| cam.project(rotate_mesh(*v, rot_x, rot_y).scale(scale)))
        .collect();

    let colour = Colour::rgb(mesh_colour(p).0, mesh_colour(p).1, mesh_colour(p).2);
    let drawn = (30.0 * k).floor() as usize;
    for (idx, &(a, b)) in edges.iter().enumerate() {
        if idx >= drawn {
            break;
        }
        if let (Some(pa), Some(pb)) = (projected[a], projected[b]) {
            canvas.line(pa.0, pa.1, pb.0, pb.1, colour);
        }
    }
    // The hot tip on the edge still being drawn, until the mesh is complete.
    if p < 1.0
        && let Some(&(a, b)) = edges.get((30.0 * k).floor() as usize)
    {
        let f = 30.0 * k - (30.0 * k).floor();
        if let (Some(pa), Some(pb)) = (projected[a], projected[b]) {
            let (tx, ty) = (pa.0 + (pb.0 - pa.0) * f, pa.1 + (pb.1 - pa.1) * f);
            canvas.disc(tx, ty, 2.5, (0xea, 0xf6, 0xff), 0.95);
            canvas.disc(tx, ty, 5.0, (0x4d, 0x9b, 0xff), 0.8);
        }
    }
}

/// The mesh's tiny settle pulse once it has landed.
#[must_use]
fn mesh_scale(t: f32) -> f32 {
    if t < DRAW_SEC {
        return 1.0;
    }
    let fp = ((t - DRAW_SEC) / RIPPLE_SEC).min(1.0);
    1.0 + 0.05 * (fp * std::f32::consts::PI).sin()
}

/// The green settle ripple that fires the moment the mesh lands.
fn draw_settle(canvas: &mut Canvas<'_>, cam: &Cam, t: f32) {
    if t < DRAW_SEC {
        return;
    }
    let fp = (t - DRAW_SEC) / RIPPLE_SEC;
    if fp >= 1.0 {
        return;
    }
    let Some((sx, sy, z)) = cam.project(V3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }) else {
        return;
    };
    let radius = cam.focal * (0.6 + 2.2 * fp) / z;
    canvas.glow(sx, sy, radius, (0x00, 0xff, 0x41), 0.55 * (1.0 - fp));
}

/// Where the wordmark's top-left corner goes: centred, its bottom at 85% of the height.
#[must_use]
fn wordmark_origin(width: u32, height: u32) -> (i32, i32) {
    let ox = (width.saturating_sub(WORDMARK_W) / 2) as i32;
    let oy = (height as f32 * 0.85) as i32 - WORDMARK_H as i32;
    (ox, oy)
}

/// The wordmark, rising in below the mark once the mesh is well under way.
fn draw_wordmark(canvas: &mut Canvas<'_>, t: f32) {
    let alpha = ease_out(((t - 1.1) / 1.8).clamp(0.0, 1.0));
    if alpha <= 0.0 {
        return;
    }
    let (ox, oy) = wordmark_origin(canvas.width, canvas.height);
    for row in 0..WORDMARK_H {
        let y = oy + row as i32;
        if y < 0 || y >= canvas.height as i32 {
            continue;
        }
        for col in 0..WORDMARK_W {
            let x = ox + col as i32;
            if x < 0 || x >= canvas.width as i32 {
                continue;
            }
            let i = ((row * WORDMARK_W + col) * 4) as usize;
            let a = WORDMARK[i];
            if a == 0 {
                continue;
            }
            let rgb = (WORDMARK[i + 1], WORDMARK[i + 2], WORDMARK[i + 3]);
            canvas.blend(x, y, rgb, f32::from(a) / 255.0 * alpha);
        }
    }
}

/// Where the tagline's top-left corner goes: centred, its bottom at 92% of the height
/// (the login.html `.tag` sits at `bottom: 8%`).
#[must_use]
fn tagline_origin(width: u32, height: u32) -> (i32, i32) {
    let ox = (width.saturating_sub(TAGLINE_W) / 2) as i32;
    let oy = (height as f32 * 0.92) as i32 - TAGLINE_H as i32;
    (ox, oy)
}

/// The tagline, rising in below the wordmark once the mesh has settled (the login.html
/// `.tag` rises at 1.5 s).
fn draw_tagline(canvas: &mut Canvas<'_>, t: f32) {
    let alpha = ease_out(((t - 1.5) / 1.8).clamp(0.0, 1.0));
    if alpha <= 0.0 {
        return;
    }
    let (ox, oy) = tagline_origin(canvas.width, canvas.height);
    for row in 0..TAGLINE_H {
        let y = oy + row as i32;
        if y < 0 || y >= canvas.height as i32 {
            continue;
        }
        for col in 0..TAGLINE_W {
            let x = ox + col as i32;
            if x < 0 || x >= canvas.width as i32 {
                continue;
            }
            let i = ((row * TAGLINE_W + col) * 4) as usize;
            let a = TAGLINE[i];
            if a == 0 {
                continue;
            }
            let rgb = (TAGLINE[i + 1], TAGLINE[i + 2], TAGLINE[i + 3]);
            canvas.blend(x, y, rgb, f32::from(a) / 255.0 * alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_through_the_draw_and_settle_then_fades() {
        assert_eq!(
            state(Duration::ZERO),
            Splash {
                showing: true,
                alpha: 1.0,
            }
        );
        assert_eq!(
            state(Duration::from_secs_f32(DRAW_SEC + SETTLE_SEC)),
            Splash {
                showing: true,
                alpha: 1.0,
            }
        );
        let mid_fade = state(Duration::from_secs_f32(
            DRAW_SEC + SETTLE_SEC + FADE_SEC / 2.0,
        ));
        assert!(mid_fade.showing);
        assert!((mid_fade.alpha - 0.5).abs() < 0.001, "{mid_fade:?}");
    }

    #[test]
    fn gone_once_the_fade_is_over() {
        assert_eq!(
            state(total()),
            Splash {
                showing: false,
                alpha: 0.0,
            }
        );
        assert!(!state(total() + Duration::from_secs(60)).showing);
    }

    #[test]
    fn progress_eases_from_zero_to_one() {
        assert_eq!(progress(0.0), (0.0, 0.0));
        assert_eq!(progress(DRAW_SEC), (1.0, 1.0));
        assert!((ease_out(0.5) - 0.875).abs() < 0.001);
    }

    #[test]
    fn the_icosahedron_has_twelve_vertices_and_thirty_equal_edges() {
        let v = icosahedron_vertices();
        assert_eq!(v.len(), 12);
        for vertex in &v {
            assert!((vertex.len() - GEOMESH_R).abs() < 0.001, "{vertex:?}");
        }
        let e = icosahedron_edges();
        assert_eq!(e.len(), 30);
        let lengths: Vec<f32> = e.iter().map(|&(a, b)| v[a].sub(v[b]).len()).collect();
        for l in &lengths {
            assert!(
                (l - lengths[0]).abs() < 0.001,
                "edges are not all one length"
            );
        }
    }

    #[test]
    fn the_colour_phases_from_blue_to_green() {
        assert_eq!(mesh_colour(0.0), (0x4d, 0x6b, 0xfe));
        assert_eq!(mesh_colour(0.44), (0xff, 0x2f, 0xd6));
        assert_eq!(mesh_colour(0.86), (0x00, 0xff, 0x41));
        assert_eq!(mesh_colour(1.0), (0x00, 0xff, 0x41));
    }

    #[test]
    fn the_first_moment_draws_blue_edges_on_brand_black() {
        let f = frame(640, 480, Duration::from_millis(300));
        assert_eq!(f.pixel(0, 0), Some(BRAND_BLACK));
        let (r, g, b) = mesh_colour(progress(0.3).0);
        let blue = f
            .pixels()
            .iter()
            .filter(|p| **p == Colour::rgb(r, g, b).to_argb())
            .count();
        assert!(
            blue > 50,
            "the first blue edges should be drawn, found {blue}"
        );
    }

    #[test]
    fn the_mark_lands_on_matrix_green() {
        let f = frame(640, 480, Duration::from_secs_f32(DRAW_SEC));
        assert_eq!(f.pixel(0, 0), Some(BRAND_BLACK));
        let green = f
            .pixels()
            .iter()
            .filter(|p| **p == MATRIX_GREEN.to_argb())
            .count();
        assert!(
            green > 500,
            "the completed mark should be green, found {green}"
        );
    }

    #[test]
    fn the_wordmark_rises_in_below_the_mark() {
        // The embedded wordmark carries the white "OS".
        let white = WORDMARK
            .chunks(4)
            .filter(|px| px[0] == 255 && px[1] == 255 && px[2] == 255 && px[3] == 255)
            .count();
        assert!(
            white > 800,
            "the white 'OS' should be in the wordmark, found {white}"
        );
        // And it is drawn (once risen) below the mark: the only pure white in the frame is
        // the wordmark's "OS", since the stars twinkle below full brightness.
        let f = frame(640, 480, Duration::from_secs_f32(3.0));
        let white = f
            .pixels()
            .iter()
            .filter(|p| **p == Colour::rgb(0xff, 0xff, 0xff).to_argb())
            .count();
        assert!(
            white > 800,
            "the wordmark should be drawn, found {white} white pixels"
        );
    }
}

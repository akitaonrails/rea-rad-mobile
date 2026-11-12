//! Pictures the launcher makes on the player's machine.
//!
//! - The menu backdrop: a snapshot of the player's own ROM (the intro's
//!   sunset skyline), dimmed and given scanlines. Nothing from the game is
//!   shipped; the launcher captures it once and keeps it in its data folder.
//! - The cabinet bezel for the 4:3 picture: our own art, drawn here.
//!
//! The palette follows the Rad Mobile title screen: a purple dusk sky,
//! crimson clouds, the gold of "MOBILE" and the chrome of "RAD".

/// An RGBA picture, row by row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&c);
    }
}

/// The title screen's colours.
pub mod palette {
    pub const NIGHT: [u8; 3] = [0x14, 0x0a, 0x2c];
    pub const PURPLE: [u8; 3] = [0x5a, 0x1a, 0x78];
    pub const CRIMSON: [u8; 3] = [0xc8, 0x3a, 0x2a];
    pub const ORANGE: [u8; 3] = [0xf0, 0x82, 0x1e];
    pub const GOLD: [u8; 3] = [0xff, 0xcc, 0x33];
    pub const GOLD_DEEP: [u8; 3] = [0xc0, 0x84, 0x10];
    pub const CHROME: [u8; 3] = [0xdc, 0xe0, 0xec];
    pub const STEEL: [u8; 3] = [0x6c, 0x70, 0x88];
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

fn scale(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (f32::from(v) * k).round().clamp(0.0, 255.0) as u8)
}

/// The game's native picture.
pub const GAME_WIDTH: u32 = 320;
pub const GAME_HEIGHT: u32 = 224;
/// The backdrop keeps the snapshot's top half: sky and skyline.
pub const BACKDROP_ROWS: u32 = GAME_HEIGHT / 2;

/// The menu backdrop from a snapshot of the intro: the middle 320 pixels
/// of its top half, dimmed (darker toward the bottom, where the menu's
/// panels sit) and with a dark line between every two (each source row
/// becomes two). None when the snapshot is too small or blank, so the menu
/// keeps its drawn sky.
pub fn backdrop(snapshot: &Image) -> Option<Image> {
    if snapshot.width < GAME_WIDTH || snapshot.height < GAME_HEIGHT {
        return None;
    }
    let x0 = (snapshot.width - GAME_WIDTH) / 2;
    let crop: Vec<[u8; 3]> = (0..BACKDROP_ROWS)
        .flat_map(|y| (0..GAME_WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| {
            let [r, g, b, _] = snapshot.pixel(x0 + x, y);
            [r, g, b]
        })
        .collect();
    if !looks_like_a_scene(&crop) {
        return None;
    }
    let mut out = Image::new(GAME_WIDTH, BACKDROP_ROWS * 2);
    for y in 0..BACKDROP_ROWS {
        let depth = y as f32 / (BACKDROP_ROWS - 1) as f32;
        for x in 0..GAME_WIDTH {
            let c = crop[(y * GAME_WIDTH + x) as usize];
            let lit = mix(scale(c, 0.62), palette::NIGHT, depth * depth * 0.55);
            let [r, g, b] = lit;
            out.set(x, y * 2, [r, g, b, 255]);
            let [r, g, b] = scale(lit, 0.72);
            out.set(x, y * 2 + 1, [r, g, b, 255]);
        }
    }
    Some(out)
}

/// Not a black or single-colour frame (the attract mode between scenes).
fn looks_like_a_scene(pixels: &[[u8; 3]]) -> bool {
    let luma: Vec<f32> = pixels
        .iter()
        .map(|[r, g, b]| 0.299 * f32::from(*r) + 0.587 * f32::from(*g) + 0.114 * f32::from(*b))
        .collect();
    let n = luma.len().max(1) as f32;
    let mean = luma.iter().sum::<f32>() / n;
    let var = luma.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / n;
    mean > 40.0 && var.sqrt() > 12.0
}

/// Where the game's picture sits inside a bezel, in bezel pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The 4:3 picture's share of the bezel's height.
const SCREEN_SHARE: f32 = 0.82;
/// Space above the picture, as a share of the height; the rest of the
/// spare height goes below, to the control panel.
const TOP_SHARE: f32 = 0.05;

/// The cabinet's own colours, after the upright Rad Mobile cabinet: flag
/// red, white and blue sides, and a control panel with sunset bars.
mod cab {
    pub const RED: [u8; 3] = [0xc4, 0x1e, 0x2a];
    pub const WHITE: [u8; 3] = [0xf2, 0xef, 0xe6];
    pub const BLUE: [u8; 3] = [0x1b, 0x2c, 0x86];
    pub const BLUE_LIGHT: [u8; 3] = [0x2f, 0x52, 0xb8];
    pub const PLASTIC: [u8; 3] = [0x0b, 0x0a, 0x10];
    pub const PANEL: [u8; 3] = [0xd6, 0xd8, 0xe0];
    pub const DEEP_RED: [u8; 3] = [0x8e, 0x12, 0x1c];
}

/// The cabinet around the 4:3 picture, `width` × `height` (the player's
/// screen shape), in the spirit of the upright Rad Mobile cabinet: stars
/// and stripes sweeping down both sides with a star roundel holding a
/// chrome steering wheel, a glossy black monitor surround with red and
/// blue pinstripes and a chrome lip, and below it the control panel's
/// sunset bars. Drawn here, shape by shape: nothing is copied from the
/// cabinet's artwork, and there are no logos.
pub fn bezel(width: u32, height: u32) -> (Image, ScreenRect) {
    let (w, h) = (width as f32, height as f32);
    // even sizes keep the hole centred on whole pixels
    let even = |v: f32| (v / 2.0).round() * 2.0;
    let sh = even(h * SCREEN_SHARE);
    let sw = even(sh * 4.0 / 3.0).min(w);
    let screen = ScreenRect {
        x: (w - sw) / 2.0,
        y: even(h * TOP_SHARE),
        width: sw,
        height: sh,
    };
    let cab = Cabinet::new(w, h, screen);
    let mut img = Image::new(width, height);
    for py in 0..height {
        for px in 0..width {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let (c, hole) = cab.paint(x, y);
            // anti-aliased edge of the hole
            let alpha = (hole + 0.5).clamp(0.0, 1.0);
            let [r, g, b] = c;
            img.set(px, py, [r, g, b, (alpha * 255.0).round() as u8]);
        }
    }
    (img, screen)
}

/// The bezel's measurements, worked out once.
struct Cabinet {
    w: f32,
    h: f32,
    screen: ScreenRect,
    centre: (f32, f32),
    frame: f32,
    corner: f32,
    /// Where the control panel starts.
    panel_top: f32,
    /// The side art's width, from the outer edge to the monitor surround.
    side: f32,
    /// The roundel's centre on the side (distance from the outer edge, y)
    /// and radius.
    roundel: (f32, f32, f32),
}

/// How much the side stripes fall per unit they run outward.
const STRIPE_SLOPE: f32 = 0.7;

impl Cabinet {
    fn new(w: f32, h: f32, screen: ScreenRect) -> Self {
        let frame = h * 0.042;
        let side = (screen.x - frame).max(1.0);
        let panel_top = screen.y + screen.height + frame + h * 0.012;
        let radius = (side * 0.40).min(h * 0.15);
        Cabinet {
            w,
            h,
            screen,
            centre: (
                screen.x + screen.width / 2.0,
                screen.y + screen.height / 2.0,
            ),
            frame,
            corner: h * 0.025,
            panel_top,
            side,
            roundel: (side * 0.5, screen.y + screen.height * 0.64, radius),
        }
    }

    /// The colour at (x, y), and the signed distance to the picture's
    /// hole (negative inside).
    fn paint(&self, x: f32, y: f32) -> ([u8; 3], f32) {
        let (sw, sh) = (self.screen.width / 2.0, self.screen.height / 2.0);
        let (dx, dy) = (x - self.centre.0, y - self.centre.1);
        let hole = round_rect(dx, dy, sw, sh, self.corner);
        let f = self.frame;
        let outer = round_rect(dx, dy, sw + f, sh + f, self.corner + f);
        let c = if outer < 0.0 {
            self.monitor_frame(hole, outer, y)
        } else if y >= self.panel_top {
            self.panel(x, y)
        } else {
            let c = self.side_art(x.min(self.w - x), y);
            // the surround casts a soft shadow
            scale(c, 1.0 - (-outer / (f * 1.2)).exp() * 0.6)
        };
        (c, hole)
    }

    /// Black gloss, lighter toward the top, with a red and a blue
    /// pinstripe and a chrome lip at the glass.
    fn monitor_frame(&self, hole: f32, outer: f32, y: f32) -> [u8; 3] {
        let f = self.frame;
        let down = ((y - self.screen.y + f) / (self.screen.height + 2.0 * f)).clamp(0.0, 1.0);
        let gloss = (1.0 - down).powi(3) * 0.45 + (1.0 + outer / f).clamp(0.0, 1.0) * 0.12;
        let mut c = mix(cab::PLASTIC, [0x34, 0x30, 0x40], gloss);
        let line = (f * 0.07).max(1.0);
        c = over(c, cab::RED, band(-outer, f * 0.22, line));
        c = over(
            c,
            cab::BLUE_LIGHT,
            band(-outer, f * 0.22 + line * 2.2, line),
        );
        if hole < 2.5 {
            c = mix(scale(palette::CHROME, 0.8), palette::STEEL, down);
        }
        c
    }

    /// Stars and stripes, `sx` from the nearer outer edge.
    fn side_art(&self, sx: f32, y: f32) -> [u8; 3] {
        let pitch = self.h * 0.05;
        let norm = (1.0 + STRIPE_SLOPE * STRIPE_SLOPE).sqrt();
        let t = (y + STRIPE_SLOPE * (self.side - sx)) / pitch;
        let canton_end = 4.5 + 3.0 * self.side / self.h;
        // red and white stripes, edges anti-aliased
        let into = (t.fract() - 0.5).abs(); // 0.5 at an edge
        let edge = (0.5 - into) * pitch / norm;
        let (this, other) = if (t.floor() as i64) % 2 == 0 {
            (cab::RED, cab::WHITE)
        } else {
            (cab::WHITE, cab::RED)
        };
        let mut c = mix(other, this, (edge + 0.5).clamp(0.0, 1.0));
        // the blue field with its stars, above the stripes
        let canton = (canton_end - t) * pitch / norm;
        if canton > -1.0 {
            let mut blue = cab::BLUE;
            let star = self.star_field(sx, y, |cx, cy| {
                (canton_end - (cy + STRIPE_SLOPE * (self.side - cx)) / pitch) * pitch / norm
            });
            blue = over(blue, cab::WHITE, star);
            c = over(c, blue, (canton + 0.5).clamp(0.0, 1.0));
        }
        // a sheen sweeping across, and darker toward the outer edge
        let sheen = (-((t - canton_end - 3.0) / 1.6).powi(2)).exp() * 0.10;
        c = mix(c, [0xff, 0xff, 0xff], sheen);
        c = scale(c, 0.72 + 0.28 * (sx / self.side).clamp(0.0, 1.0).sqrt());
        self.roundel_over(c, sx, y)
    }

    /// Coverage of the canton's staggered white stars at (sx, y); only
    /// stars that sit wholly inside the field (`room`: how far a point is
    /// inside it).
    fn star_field(&self, sx: f32, y: f32, room: impl Fn(f32, f32) -> f32) -> f32 {
        let gap = (self.side * 0.2).clamp(self.h * 0.035, self.h * 0.06);
        let r = gap * 0.3;
        let row = (y / gap).floor();
        let shift = if (row as i64) % 2 == 0 { 0.5 } else { 0.0 };
        let col = (sx / gap - shift).floor();
        let (cx, cy) = ((col + shift + 0.5) * gap, (row + 0.5) * gap);
        if room(cx, cy) < r * 1.6 || cx + r > self.side || cx - r < 0.0 {
            return 0.0;
        }
        coverage(star5(sx - cx, y - cy, r))
    }

    /// The roundel: a white rim, a blue ring of stars, a red disc, and a
    /// chrome hub holding a black steering wheel.
    fn roundel_over(&self, mut c: [u8; 3], sx: f32, y: f32) -> [u8; 3] {
        let (rx, ry, r) = self.roundel;
        let (x, y) = (sx - rx, y - ry);
        let d = x.hypot(y);
        if d > r * 1.12 {
            return c;
        }
        c = over(c, [0x08, 0x06, 0x10], coverage(d - r * 1.10) * 0.5); // shadow
        c = over(c, cab::WHITE, coverage(d - r * 1.06));
        c = over(c, cab::BLUE, coverage(d - r));
        let angle = y.atan2(x);
        let step = std::f32::consts::TAU / 12.0;
        let a = (angle / step).round() * step;
        let (scx, scy) = (a.cos() * r * 0.84, a.sin() * r * 0.84);
        c = over(c, cab::WHITE, coverage(star5(x - scx, y - scy, r * 0.10)));
        c = over(c, cab::WHITE, coverage(d - r * 0.70));
        c = over(c, cab::RED, coverage(d - r * 0.66));
        c = over(c, cab::DEEP_RED, band(d, r * 0.50, r * 0.012) * 0.8);
        // the chrome hub, lit from the top left
        let lit = ((-x - y) / (r * 0.9) * 0.5 + 0.5).clamp(0.0, 1.0);
        let chrome = mix(palette::STEEL, palette::CHROME, lit);
        c = over(c, chrome, coverage(d - r * 0.46));
        // the wheel: rim, hub and three spokes
        let rim = (d - r * 0.33).abs() - r * 0.045;
        let hub = d - r * 0.08;
        let spoke =
            |ax: f32, ay: f32| segment(x, y, 0.0, 0.0, ax * r * 0.33, ay * r * 0.33) - r * 0.03;
        let spokes = spoke(-1.0, 0.0).min(spoke(1.0, 0.0)).min(spoke(0.0, 1.0));
        let wheel = rim.min(hub).min(spokes);
        over(c, [0x14, 0x14, 0x1a], coverage(wheel))
    }

    /// The control panel: white with a chrome top lip, sunset bars on both
    /// sides and a blue plate in the middle, where the wheel sits.
    fn panel(&self, x: f32, y: f32) -> [u8; 3] {
        let height = (self.h - self.panel_top).max(1.0);
        let v = (y - self.panel_top) / height;
        let mut c = mix(cab::PANEL, scale(cab::PANEL, 0.78), v);
        const BARS: [([u8; 3], f32, f32); 4] = [
            (palette::GOLD, 0.16, 0.24),
            (palette::ORANGE, 0.31, 0.42),
            (cab::RED, 0.50, 0.65),
            (cab::DEEP_RED, 0.73, 0.92),
        ];
        let px = 1.0 / height;
        for (colour, top, bottom) in BARS {
            let inside =
                ((v - top) / px + 0.5).clamp(0.0, 1.0) * ((bottom - v) / px + 0.5).clamp(0.0, 1.0);
            c = over(c, colour, inside);
        }
        let plate = round_rect(
            x - self.centre.0,
            y - (self.panel_top + height * 0.55),
            self.screen.width * 0.22,
            height * 0.42,
            height * 0.12,
        );
        let plate_c = mix(cab::BLUE_LIGHT, cab::BLUE, v);
        c = over(c, scale(c, 0.6), coverage(plate - 2.0) * 0.6); // its shadow
        c = over(c, plate_c, coverage(plate));
        // START on the left, WIPER and LIGHT on the right, as on the cabinet
        let r = height * 0.24;
        let by = self.panel_top + height * 0.52;
        let reach = self.screen.width * 0.22 + r * 2.4;
        let buttons = [
            (self.centre.0 - reach, palette::GOLD),
            (self.centre.0 + reach, cab::BLUE_LIGHT),
            (self.centre.0 + reach + r * 2.8, [0x1c, 0x9a, 0x48]),
        ];
        for (bx, colour) in buttons {
            c = button_over(c, x - bx, y - by, r, colour);
        }
        // chrome lip along the top
        let lip = (self.h * 0.006).max(1.0);
        if y - self.panel_top < lip {
            c = mix(palette::CHROME, palette::STEEL, (y - self.panel_top) / lip);
        }
        c
    }
}

/// A round arcade button of radius `r` at the origin: a dark bezel ring, a
/// coloured cap lit from the top left, and its shadow.
fn button_over(mut c: [u8; 3], x: f32, y: f32, r: f32, colour: [u8; 3]) -> [u8; 3] {
    let d = x.hypot(y);
    if d > r * 1.5 {
        return c;
    }
    c = over(
        c,
        scale(c, 0.55),
        coverage(x.hypot(y - r * 0.18) - r * 1.12) * 0.7,
    );
    c = over(c, [0x1a, 0x1a, 0x22], coverage(d - r * 1.12));
    let lit = ((-x - y) / r * 0.5 + 0.5).clamp(0.0, 1.0);
    let cap = mix(
        scale(colour, 0.7),
        mix(colour, [0xff, 0xff, 0xff], 0.35),
        lit,
    );
    c = over(c, cap, coverage(d - r * 0.9));
    let glint = x.hypot(y + r * 0.4) / (r * 0.35);
    over(c, [0xff, 0xff, 0xff], ((1.0 - glint) * 0.6).clamp(0.0, 0.6))
}

/// Signed distance to a rounded rectangle centred on the origin.
fn round_rect(x: f32, y: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let qx = x.abs() - half_w + radius;
    let qy = y.abs() - half_h + radius;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - radius
}

/// Signed distance to a five-pointed star of outer radius `r`, point up
/// (after Inigo Quilez's `sdStar5`).
fn star5(x: f32, y: f32, r: f32) -> f32 {
    const K1: (f32, f32) = (0.809_017, -0.587_785);
    const K2: (f32, f32) = (-K1.0, K1.1);
    const INNER: f32 = 0.382;
    // the formula works with y up
    let (mut px, mut py) = (x.abs(), -y);
    let d1 = (K1.0 * px + K1.1 * py).max(0.0) * 2.0;
    px -= d1 * K1.0;
    py -= d1 * K1.1;
    let d2 = (K2.0 * px + K2.1 * py).max(0.0) * 2.0;
    px -= d2 * K2.0;
    py -= d2 * K2.1;
    px = px.abs();
    py -= r;
    let (bax, bay) = (INNER * -K1.1, INNER * K1.0 - 1.0);
    let h = ((px * bax + py * bay) / (bax * bax + bay * bay)).clamp(0.0, r);
    let len = (px - bax * h).hypot(py - bay * h);
    len * (py * bax - px * bay).signum()
}

/// Distance from (x, y) to the segment from (ax, ay) to (bx, by).
fn segment(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (pax, pay, bax, bay) = (x - ax, y - ay, bx - ax, by - ay);
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    (pax - bax * h).hypot(pay - bay * h)
}

/// How much of a pixel a shape covers, from its signed distance.
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Coverage of a line `width` wide, `at` along a distance `d`.
fn band(d: f32, at: f32, width: f32) -> f32 {
    coverage((d - at).abs() - width / 2.0)
}

fn over(base: [u8; 3], top: [u8; 3], amount: f32) -> [u8; 3] {
    mix(base, top, amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(fill: impl Fn(u32, u32) -> [u8; 3]) -> Image {
        let mut img = Image::new(426, 224);
        for y in 0..224 {
            for x in 0..426 {
                let [r, g, b] = fill(x, y);
                img.set(x, y, [r, g, b, 255]);
            }
        }
        img
    }

    #[test]
    fn the_backdrop_keeps_the_middle_of_the_sky_dimmed_with_scanlines() {
        // a busy sky: stripes of colour
        let snap = snapshot(|x, _| {
            if x % 7 < 3 {
                [200, 120, 180]
            } else {
                [80, 60, 160]
            }
        });
        let out = backdrop(&snap).expect("a scene");
        assert_eq!((out.width, out.height), (GAME_WIDTH, BACKDROP_ROWS * 2));
        let source = snap.pixel(53, 0);
        let lit = out.pixel(0, 0);
        assert!(lit[0] < source[0], "dimmed");
        let line = out.pixel(0, 1);
        assert!(line[0] < lit[0], "every second row is a darker scanline");
        // darker toward the bottom, where the panels are
        let top = out.pixel(3, 0)[0];
        let bottom = out.pixel(3, out.height - 2)[0];
        assert!(bottom < top);
    }

    #[test]
    fn a_blank_or_tiny_snapshot_makes_no_backdrop() {
        assert!(backdrop(&snapshot(|_, _| [0, 0, 0])).is_none());
        assert!(backdrop(&snapshot(|_, _| [90, 90, 90])).is_none());
        assert!(backdrop(&Image::new(100, 50)).is_none());
    }

    #[test]
    fn the_bezel_frames_a_transparent_4_3_hole() {
        let (img, screen) = bezel(320, 180);
        assert!((screen.width / screen.height - 4.0 / 3.0).abs() < 0.02);
        assert!(screen.x > 0.0 && screen.x + screen.width < 320.0);
        // the middle shows the game, the sides are solid cabinet
        assert_eq!(img.pixel(160, 90)[3], 0);
        assert_eq!(img.pixel(5, 90)[3], 255);
        assert_eq!(img.pixel(314, 90)[3], 255);
        // just outside the hole is the frame, solid too
        let edge = (screen.x - 2.0) as u32;
        assert_eq!(img.pixel(edge, 90)[3], 255);
    }

    #[test]
    fn the_bezel_is_mirrored_left_to_right() {
        let (img, _) = bezel(320, 180);
        for y in [20, 90, 160] {
            assert_eq!(img.pixel(10, y), img.pixel(309, y), "row {y}");
        }
    }

    #[test]
    fn the_sides_wear_stars_and_stripes_and_the_panel_sits_below() {
        let (img, screen) = bezel(1920, 1080);
        assert!(screen.y < 1080.0 - (screen.y + screen.height), "room below");
        let side = (screen.x - 1080.0 * 0.042) as u32;
        let mut red = 0;
        let mut white = 0;
        let mut blue = 0;
        for y in (0..900).step_by(3) {
            for x in (0..side).step_by(3) {
                let [r, g, b, _] = img.pixel(x, y);
                let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
                if r > 150 && g < 80 && b < 80 {
                    red += 1;
                } else if r > 180 && g > 180 && b > 170 {
                    white += 1;
                } else if b > 100 && r < 70 {
                    blue += 1;
                }
            }
        }
        assert!(
            red > 500 && white > 500 && blue > 500,
            "{red} {white} {blue}"
        );
        // the panel's bars: warm colours under the picture's corners
        let below = (screen.y + screen.height) as u32 + 60;
        let warm = (below..1080).any(|y| {
            let [r, g, b, _] = img.pixel(screen.x as u32 + 20, y);
            r > 180 && b < 90 && g < 200
        });
        assert!(warm);
    }

    #[test]
    fn a_star_is_inside_at_its_centre_and_points() {
        assert!(star5(0.0, 0.0, 10.0) < 0.0);
        assert!(star5(0.0, -9.0, 10.0) < 0.0, "top point (y down)");
        assert!(
            star5(0.0, 9.0, 10.0) > 0.0,
            "a notch between the lower points"
        );
        assert!(star5(20.0, 0.0, 10.0) > 0.0);
    }

    #[test]
    fn rounded_rectangle_distances() {
        assert!(round_rect(0.0, 0.0, 10.0, 5.0, 1.0) < 0.0);
        assert!((round_rect(12.0, 0.0, 10.0, 5.0, 1.0) - 2.0).abs() < 1e-4);
        // the corner is rounded: the exact corner point is outside
        assert!(round_rect(10.0, 5.0, 10.0, 5.0, 2.0) > 0.0);
    }
}

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
const SCREEN_SHARE: f32 = 0.86;

/// The cabinet around the 4:3 picture, `width` × `height` (the player's
/// screen shape): a dark cabinet with speed stripes in the title screen's
/// colours on both sides, a glossy black monitor frame with a chrome lip,
/// and a transparent, round-cornered hole where the picture shows.
pub fn bezel(width: u32, height: u32) -> (Image, ScreenRect) {
    let (w, h) = (width as f32, height as f32);
    // even sizes keep the hole centred on whole pixels
    let even = |v: f32| (v / 2.0).round() * 2.0;
    let sh = even(h * SCREEN_SHARE);
    let sw = even(sh * 4.0 / 3.0).min(w);
    let screen = ScreenRect {
        x: (w - sw) / 2.0,
        y: (h - sh) / 2.0,
        width: sw,
        height: sh,
    };
    let frame = h * 0.035;
    let corner = h * 0.025;
    let (cx, cy) = (screen.x + sw / 2.0, screen.y + sh / 2.0);
    let mut img = Image::new(width, height);
    for py in 0..height {
        for px in 0..width {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let hole = round_rect(x - cx, y - cy, sw / 2.0, sh / 2.0, corner);
            let outer = round_rect(
                x - cx,
                y - cy,
                sw / 2.0 + frame,
                sh / 2.0 + frame,
                corner + frame,
            );
            let c = if outer < 0.0 {
                monitor_frame(
                    hole,
                    outer,
                    frame,
                    (y - screen.y + frame) / (sh + 2.0 * frame),
                )
            } else {
                cabinet(x, y, w, h, outer, frame)
            };
            // anti-aliased edge of the hole
            let alpha = (hole + 0.5).clamp(0.0, 1.0);
            let [r, g, b] = c;
            img.set(px, py, [r, g, b, (alpha * 255.0).round() as u8]);
        }
    }
    (img, screen)
}

/// Signed distance to a rounded rectangle centred on the origin.
fn round_rect(x: f32, y: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let qx = x.abs() - half_w + radius;
    let qy = y.abs() - half_h + radius;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - radius
}

/// Black plastic, lighter toward the top, with a chrome lip at the glass.
fn monitor_frame(hole: f32, outer: f32, frame: f32, down: f32) -> [u8; 3] {
    let depth = (-outer / frame).clamp(0.0, 1.0); // 0 at the outer edge, 1 at the glass
    let gloss = (1.0 - down).powi(3) * 0.5;
    let base = mix(
        [0x0a, 0x08, 0x0e],
        [0x2e, 0x2a, 0x3a],
        gloss + (1.0 - depth) * 0.25,
    );
    if hole < 2.5 {
        mix(scale(palette::CHROME, 0.75), palette::STEEL, down)
    } else {
        base
    }
}

/// The cabinet beside the screen: a dusk gradient with four speed stripes
/// sweeping up on each side, and the frame's shadow.
fn cabinet(x: f32, y: f32, w: f32, h: f32, outer: f32, frame: f32) -> [u8; 3] {
    let down = y / h;
    let mut c = mix(palette::NIGHT, [0x08, 0x05, 0x12], down);
    // mirror the right side onto the left
    let side_x = x.min(w - x);
    let u = (side_x + (h - y) * 0.55) / h;
    const STRIPES: [[u8; 3]; 4] = [
        palette::GOLD,
        palette::ORANGE,
        palette::CRIMSON,
        palette::PURPLE,
    ];
    let start = 0.10;
    let pitch = 0.055;
    let band = (u - start) / pitch;
    if band >= 0.0 && (band as usize) < STRIPES.len() && band.fract() < 0.7 {
        let stripe = STRIPES[band as usize];
        // fade out as the stripes climb toward the top
        c = mix(c, stripe, 0.6 + 0.3 * down);
    }
    // the frame casts a soft shadow
    let shadow = (-outer / (frame * 1.5)).exp() * 0.65;
    scale(c, 1.0 - shadow)
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
    fn rounded_rectangle_distances() {
        assert!(round_rect(0.0, 0.0, 10.0, 5.0, 1.0) < 0.0);
        assert!((round_rect(12.0, 0.0, 10.0, 5.0, 1.0) - 2.0).abs() < 1e-4);
        // the corner is rounded: the exact corner point is outside
        assert!(round_rect(10.0, 5.0, 10.0, 5.0, 2.0) > 0.0);
    }
}

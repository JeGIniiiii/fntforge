//! Glyph layer-effect compositor.
//!
//! Synthesis order (Photoshop-inspired, bottom to top):
//! drop shadow → outer glow → fill → inner shadow → inner glow → color overlay → stroke.

use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(hex: &str) -> Self {
        let h = hex.trim().trim_start_matches('#');
        let parse = |i| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
        if h.len() >= 8 {
            Self::new(parse(0), parse(2), parse(4), parse(6))
        } else {
            Self::new(parse(0), parse(2), parse(4), 255)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Solid(Rgba8),
    Linear {
        stops: Vec<(f32, Rgba8)>,
        angle_deg: f32,
    },
}

impl Default for Fill {
    fn default() -> Self {
        Fill::Solid(Rgba8::WHITE)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokePosition {
    Outer,
    Center,
    Inner,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub enabled: bool,
    pub size: f32,
    pub position: StrokePosition,
    pub color: Rgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub enabled: bool,
    pub color: Rgba8,
    pub distance: f32,
    pub angle_deg: f32,
    pub size: f32,
    pub spread: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glow {
    pub enabled: bool,
    pub color: Rgba8,
    pub size: f32,
    pub spread: f32,
    #[allow(dead_code)]
    pub from_center: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    pub enabled: bool,
    pub color: Rgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleStack {
    pub fill: Fill,
    pub stroke: Stroke,
    pub drop_shadow: Shadow,
    pub inner_shadow: Shadow,
    pub outer_glow: Glow,
    pub inner_glow: Glow,
    pub color_overlay: Overlay,
}

impl Default for StyleStack {
    fn default() -> Self {
        Self {
            fill: Fill::default(),
            stroke: Stroke {
                enabled: false,
                size: 2.0,
                position: StrokePosition::Outer,
                color: Rgba8::BLACK,
            },
            drop_shadow: Shadow {
                enabled: false,
                color: Rgba8::new(0, 0, 0, 160),
                distance: 2.0,
                angle_deg: 120.0,
                size: 2.0,
                spread: 0.0,
            },
            inner_shadow: Shadow {
                enabled: false,
                color: Rgba8::new(0, 0, 0, 140),
                distance: 1.0,
                angle_deg: 120.0,
                size: 2.0,
                spread: 0.0,
            },
            outer_glow: Glow {
                enabled: false,
                color: Rgba8::new(255, 200, 80, 180),
                size: 4.0,
                spread: 0.0,
                from_center: false,
            },
            inner_glow: Glow {
                enabled: false,
                color: Rgba8::new(255, 255, 255, 120),
                size: 3.0,
                spread: 0.0,
                from_center: false,
            },
            color_overlay: Overlay {
                enabled: false,
                color: Rgba8::new(255, 210, 80, 255),
            },
        }
    }
}

impl StyleStack {
    /// Extra pixels needed around a tight glyph bitmap so effects are not clipped.
    pub fn padding(&self) -> i32 {
        let mut p = 1.0f32;
        if self.stroke.enabled {
            let extra = match self.stroke.position {
                StrokePosition::Inner => 0.0,
                StrokePosition::Center => self.stroke.size * 0.5,
                StrokePosition::Outer => self.stroke.size,
            };
            p = p.max(extra + 1.0);
        }
        if self.drop_shadow.enabled {
            p = p.max(self.drop_shadow.distance + self.drop_shadow.size + 2.0);
        }
        if self.outer_glow.enabled {
            p = p.max(self.outer_glow.size + 2.0);
        }
        p.ceil() as i32
    }
}

/// Render a glyph: `mask` is an 8-bit coverage buffer of size `mw x mh`.
pub fn render_glyph(mask: &[u8], mw: u32, mh: u32, style: &StyleStack) -> RgbaImage {
    let pad = style.padding().max(0) as u32;
    let w = mw + pad * 2;
    let h = mh + pad * 2;
    let mut src = vec![0u8; (w * h) as usize];
    for y in 0..mh {
        for x in 0..mw {
            src[((y + pad) * w + (x + pad)) as usize] = mask[(y * mw + x) as usize];
        }
    }

    let mut out = RgbaImage::new(w, h);

    if style.drop_shadow.enabled {
        blit_shadow(&mut out, &src, w, h, &style.drop_shadow, false);
    }
    if style.outer_glow.enabled {
        blit_glow(&mut out, &src, w, h, &style.outer_glow, false);
    }

    // Fill
    for y in 0..h {
        for x in 0..w {
            let a = src[(y * w + x) as usize] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let col = sample_fill(&style.fill, x, y, w, h);
            let px = premul_over(pixel(&out, x, y), scale_rgba(col, a));
            put(&mut out, x, y, px);
        }
    }

    if style.inner_shadow.enabled {
        blit_shadow(&mut out, &src, w, h, &style.inner_shadow, true);
    }
    if style.inner_glow.enabled {
        blit_glow(&mut out, &src, w, h, &style.inner_glow, true);
    }

    if style.color_overlay.enabled {
        let ov = style.color_overlay.color;
        for y in 0..h {
            for x in 0..w {
                let a = src[(y * w + x) as usize] as f32 / 255.0;
                if a <= 0.0 {
                    continue;
                }
                let dst = pixel(&out, x, y);
                let srcp = scale_rgba(ov, a * (ov.a as f32 / 255.0));
                put(&mut out, x, y, premul_over(dst, srcp));
            }
        }
    }

    if style.stroke.enabled && style.stroke.size > 0.0 {
        blit_stroke(&mut out, &src, w, h, &style.stroke);
    }

    out
}

fn sample_fill(fill: &Fill, x: u32, y: u32, w: u32, h: u32) -> Rgba8 {
    match fill {
        Fill::Solid(c) => *c,
        Fill::Linear { stops, angle_deg } => {
            if stops.is_empty() {
                return Rgba8::WHITE;
            }
            let rad = angle_deg.to_radians();
            let nx = rad.cos();
            let ny = rad.sin();
            let cx = w as f32 * 0.5;
            let cy = h as f32 * 0.5;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let proj = dx * nx + dy * ny;
            let span = (w.max(h) as f32) * 0.5;
            let t = ((proj / span) * 0.5 + 0.5).clamp(0.0, 1.0);
            lerp_stops(stops, t)
        }
    }
}

fn lerp_stops(stops: &[(f32, Rgba8)], t: f32) -> Rgba8 {
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if t <= s[0].0 {
        return s[0].1;
    }
    for w in s.windows(2) {
        if t <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-5);
            let u = (t - w[0].0) / span;
            return lerp_color(w[0].1, w[1].1, u);
        }
    }
    s.last().unwrap().1
}

fn lerp_color(a: Rgba8, b: Rgba8, t: f32) -> Rgba8 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgba8::new(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
}

fn blit_shadow(out: &mut RgbaImage, src: &[u8], w: u32, h: u32, sh: &Shadow, inner: bool) {
    let rad = sh.angle_deg.to_radians();
    // Photoshop 0° is right, increasing counter-clockwise; Y grows down here so sin is flipped.
    let ox = (rad.cos() * sh.distance).round() as i32;
    let oy = (-rad.sin() * sh.distance).round() as i32;
    let mut shifted = vec![0u8; src.len()];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let sx = x - ox;
            let sy = y - oy;
            let v = sample_u8(src, w, h, sx, sy);
            shifted[(y as u32 * w + x as u32) as usize] = v;
        }
    }
    let mut layer = if inner {
        shifted.iter().map(|v| 255u8.saturating_sub(*v)).collect()
    } else {
        shifted
    };
    if sh.spread > 0.0 {
        layer = dilate(&layer, w, h, sh.spread);
    }
    let blurred = blur(&layer, w, h, sh.size);
    for y in 0..h {
        for x in 0..w {
            let mut a = blurred[(y * w + x) as usize] as f32 / 255.0;
            if inner {
                let m = src[(y * w + x) as usize] as f32 / 255.0;
                a *= m;
            }
            if a <= 0.001 {
                continue;
            }
            let srcp = scale_rgba(sh.color, a);
            let dst = pixel(out, x, y);
            put(out, x, y, premul_over(dst, srcp));
        }
    }
}

fn blit_glow(out: &mut RgbaImage, src: &[u8], w: u32, h: u32, glow: &Glow, inner: bool) {
    let mut layer = if inner {
        src.iter().map(|v| 255u8.saturating_sub(*v)).collect()
    } else {
        src.to_vec()
    };
    if glow.spread > 0.0 {
        layer = dilate(&layer, w, h, glow.spread);
    }
    let blurred = blur(&layer, w, h, glow.size.max(0.5));
    for y in 0..h {
        for x in 0..w {
            let mut a = blurred[(y * w + x) as usize] as f32 / 255.0;
            if inner {
                a *= src[(y * w + x) as usize] as f32 / 255.0;
            } else {
                // Keep glow outside the solid glyph so fill stays clean.
                a *= 1.0 - src[(y * w + x) as usize] as f32 / 255.0;
            }
            if a <= 0.001 {
                continue;
            }
            let srcp = scale_rgba(glow.color, a);
            let dst = pixel(out, x, y);
            put(out, x, y, premul_over(dst, srcp));
        }
    }
}

fn blit_stroke(out: &mut RgbaImage, src: &[u8], w: u32, h: u32, stroke: &Stroke) {
    let size = stroke.size.max(0.5);
    let outer = dilate(src, w, h, size);
    let inner = erode(src, w, h, size);
    let half_o = dilate(src, w, h, size * 0.5);
    let half_i = erode(src, w, h, size * 0.5);
    for y in 0..h {
        for x in 0..w {
            let s = src[(y * w + x) as usize] as f32 / 255.0;
            let o = outer[(y * w + x) as usize] as f32 / 255.0;
            let i = inner[(y * w + x) as usize] as f32 / 255.0;
            let a = match stroke.position {
                StrokePosition::Outer => (o - s).max(0.0),
                StrokePosition::Inner => (s - i).max(0.0),
                StrokePosition::Center => {
                    let ho = half_o[(y * w + x) as usize] as f32 / 255.0;
                    let hi = half_i[(y * w + x) as usize] as f32 / 255.0;
                    (ho - hi).max(0.0)
                }
            };
            if a <= 0.001 {
                continue;
            }
            let srcp = scale_rgba(stroke.color, a);
            let dst = pixel(out, x, y);
            put(out, x, y, premul_over(dst, srcp));
        }
    }
}

fn dilate(src: &[u8], w: u32, h: u32, radius: f32) -> Vec<u8> {
    morph(src, w, h, radius, true)
}

fn erode(src: &[u8], w: u32, h: u32, radius: f32) -> Vec<u8> {
    morph(src, w, h, radius, false)
}

fn morph(src: &[u8], w: u32, h: u32, radius: f32, dilate: bool) -> Vec<u8> {
    let r = radius.ceil() as i32;
    if r <= 0 {
        return src.to_vec();
    }
    let r2 = radius * radius;
    let mut out = vec![if dilate { 0 } else { 255 }; src.len()];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = if dilate { 0u8 } else { 255u8 };
            for dy in -r..=r {
                for dx in -r..=r {
                    if (dx * dx + dy * dy) as f32 > r2 {
                        continue;
                    }
                    let v = sample_u8(src, w, h, x + dx, y + dy);
                    acc = if dilate { acc.max(v) } else { acc.min(v) };
                }
            }
            out[(y as u32 * w + x as u32) as usize] = acc;
        }
    }
    out
}

/// Separable approximate Gaussian via 3 box blurs.
fn blur(src: &[u8], w: u32, h: u32, radius: f32) -> Vec<u8> {
    if radius < 0.4 {
        return src.to_vec();
    }
    let n = (radius.clamp(0.5, 32.0)).round() as i32;
    let mut buf = src.to_vec();
    for _ in 0..3 {
        buf = box_blur(&buf, w, h, n);
    }
    buf
}

fn box_blur(src: &[u8], w: u32, h: u32, r: i32) -> Vec<u8> {
    let mut tmp = vec![0u8; src.len()];
    let mut out = vec![0u8; src.len()];
    let wr = (r * 2 + 1) as f32;
    // horizontal
    for y in 0..h as i32 {
        let mut sum = 0i32;
        for x in -r..=r {
            sum += sample_u8(src, w, h, x, y) as i32;
        }
        for x in 0..w as i32 {
            tmp[(y as u32 * w + x as u32) as usize] = (sum as f32 / wr).round() as u8;
            sum -= sample_u8(src, w, h, x - r, y) as i32;
            sum += sample_u8(src, w, h, x + r + 1, y) as i32;
        }
    }
    // vertical
    for x in 0..w as i32 {
        let mut sum = 0i32;
        for y in -r..=r {
            sum += sample_u8(&tmp, w, h, x, y) as i32;
        }
        for y in 0..h as i32 {
            out[(y as u32 * w + x as u32) as usize] = (sum as f32 / wr).round() as u8;
            sum -= sample_u8(&tmp, w, h, x, y - r) as i32;
            sum += sample_u8(&tmp, w, h, x, y + r + 1) as i32;
        }
    }
    out
}

fn sample_u8(src: &[u8], w: u32, h: u32, x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        0
    } else {
        src[(y as u32 * w + x as u32) as usize]
    }
}

fn pixel(img: &RgbaImage, x: u32, y: u32) -> Rgba<u8> {
    *img.get_pixel(x, y)
}

fn put(img: &mut RgbaImage, x: u32, y: u32, p: Rgba<u8>) {
    img.put_pixel(x, y, p);
}

fn scale_rgba(c: Rgba8, a: f32) -> Rgba<u8> {
    let aa = (c.a as f32 / 255.0) * a.clamp(0.0, 1.0);
    Rgba([
        (c.r as f32 * aa).round() as u8,
        (c.g as f32 * aa).round() as u8,
        (c.b as f32 * aa).round() as u8,
        (aa * 255.0).round() as u8,
    ])
}

/// `src` and `dst` stored as straight-ish premultiplied in RGB, A separate.
fn premul_over(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mix = |s, d| (s as f32 + d as f32 * (1.0 - sa)) / out_a;
    Rgba([
        mix(src[0], dst[0]).round().clamp(0.0, 255.0) as u8,
        mix(src[1], dst[1]).round().clamp(0.0, 255.0) as u8,
        mix(src[2], dst[2]).round().clamp(0.0, 255.0) as u8,
        (out_a * 255.0).round() as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_grows_with_shadow() {
        let mut s = StyleStack::default();
        s.drop_shadow.enabled = true;
        s.drop_shadow.distance = 4.0;
        s.drop_shadow.size = 3.0;
        assert!(s.padding() >= 7);
    }

    #[test]
    fn solid_fill_preserves_coverage() {
        let mut mask = vec![0u8; 8 * 8];
        mask[3 * 8 + 3] = 255;
        let img = render_glyph(&mask, 8, 8, &StyleStack::default());
        let mut any = false;
        for p in img.pixels() {
            if p[3] > 0 {
                any = true;
                assert_eq!(p[0], 255);
            }
        }
        assert!(any);
    }
}

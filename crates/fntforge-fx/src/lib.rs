//! Glyph layer-effect compositor.
//!
//! Order (Photoshop-inspired, bottom → top):
//! drop shadow → outer glow → fill → bevel → inner shadow → inner glow
//! → satin → color/gradient overlay → stroke.

mod sdf;
mod style;

pub use style::*;

use image::{Rgba, RgbaImage};
use sdf::{band, max_inside, signed_distance, smoothstep};

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

    let sdf = signed_distance(&src, w as usize, h as usize);
    let mut out = RgbaImage::new(w, h);

    if style.drop_shadow.enabled {
        blit_shadow(&mut out, &src, &sdf, w, h, style, false);
    }
    if style.outer_glow.enabled {
        blit_glow(&mut out, &src, &sdf, w, h, &style.outer_glow, false);
    }

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

    if style.bevel.enabled {
        blit_bevel(&mut out, &src, &sdf, w, h, style);
    }
    if style.inner_shadow.enabled {
        blit_shadow(&mut out, &src, &sdf, w, h, style, true);
    }
    if style.inner_glow.enabled {
        blit_glow(&mut out, &src, &sdf, w, h, &style.inner_glow, true);
    }
    if style.satin.enabled {
        blit_satin(&mut out, &src, w, h, &style.satin);
    }
    if style.color_overlay.enabled {
        let ov = style.color_overlay.color;
        let op = style.color_overlay.opacity;
        for y in 0..h {
            for x in 0..w {
                let a = src[(y * w + x) as usize] as f32 / 255.0 * op * (ov.a as f32 / 255.0);
                if a <= 0.0 {
                    continue;
                }
                let blended = blend(style.color_overlay.blend, pixel(&out, x, y), ov, a);
                put(&mut out, x, y, blended);
            }
        }
    }
    if style.gradient_overlay.enabled && !style.gradient_overlay.stops.is_empty() {
        let go = &style.gradient_overlay;
        for y in 0..h {
            for x in 0..w {
                let a = src[(y * w + x) as usize] as f32 / 255.0 * go.opacity;
                if a <= 0.0 {
                    continue;
                }
                let col = lerp_stops(&go.stops, gradient_t(x, y, w, h, go.angle_deg));
                let blended = blend(go.blend, pixel(&out, x, y), col, a * (col.a as f32 / 255.0));
                put(&mut out, x, y, blended);
            }
        }
    }

    if style.stroke.enabled && style.stroke.size > 0.0 {
        blit_stroke(&mut out, &src, &sdf, w, h, &style.stroke);
    }

    out
}

fn sample_fill(fill: &Fill, x: u32, y: u32, w: u32, h: u32) -> Rgba8 {
    match fill {
        Fill::Solid { color } => *color,
        Fill::Linear { stops, angle_deg } => {
            if stops.is_empty() {
                return Rgba8::WHITE;
            }
            lerp_stops(stops, gradient_t(x, y, w, h, *angle_deg))
        }
    }
}

fn gradient_t(x: u32, y: u32, w: u32, h: u32, angle_deg: f32) -> f32 {
    let rad = angle_deg.to_radians();
    let nx = rad.cos();
    let ny = -rad.sin();
    let dx = x as f32 - w as f32 * 0.5;
    let dy = y as f32 - h as f32 * 0.5;
    let proj = dx * nx + dy * ny;
    let span = (w.max(h) as f32) * 0.5;
    ((proj / span) * 0.5 + 0.5).clamp(0.0, 1.0)
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
            return lerp_color(w[0].1, w[1].1, (t - w[0].0) / span);
        }
    }
    s.last().unwrap().1
}

fn lerp_color(a: Rgba8, b: Rgba8, t: f32) -> Rgba8 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgba8::new(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
}

fn light_angle(style: &StyleStack, local: f32, use_global: bool) -> f32 {
    if use_global {
        style.global_light.angle_deg
    } else {
        local
    }
}

fn blit_shadow(
    out: &mut RgbaImage,
    src: &[u8],
    sdf: &[f32],
    w: u32,
    h: u32,
    style: &StyleStack,
    inner: bool,
) {
    let sh = if inner {
        &style.inner_shadow
    } else {
        &style.drop_shadow
    };
    let ang = light_angle(style, sh.angle_deg, sh.use_global_light).to_radians();
    let ox = (ang.cos() * sh.distance).round() as i32;
    let oy = (-ang.sin() * sh.distance).round() as i32;
    let size = sh.size.max(0.5);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let sx = x - ox;
            let sy = y - oy;
            if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
                continue;
            }
            let idx = (sy as u32 * w + sx as u32) as usize;
            let d = sdf[idx];
            let mut a = if inner {
                // inside the shifted hole
                smoothstep(-size, 0.0, d)
            } else {
                1.0 - smoothstep(0.0, size + sh.spread, d.max(0.0))
            };
            if inner {
                a *= src[(y as u32 * w + x as u32) as usize] as f32 / 255.0;
            } else {
                a *= 1.0 - src[(y as u32 * w + x as u32) as usize] as f32 / 255.0;
            }
            a *= sh.opacity;
            if a <= 0.001 {
                continue;
            }
            let blended = blend(sh.blend, pixel(out, x as u32, y as u32), sh.color, a * (sh.color.a as f32 / 255.0));
            put(out, x as u32, y as u32, blended);
        }
    }
}

fn blit_glow(
    out: &mut RgbaImage,
    src: &[u8],
    sdf: &[f32],
    w: u32,
    h: u32,
    glow: &Glow,
    inner: bool,
) {
    let size = glow.size.max(0.5);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let d = sdf[i];
            let t = if inner {
                if d >= 0.0 {
                    0.0
                } else {
                    (1.0 - (-d / size).clamp(0.0, 1.0)).clamp(0.0, 1.0)
                }
            } else if d <= 0.0 {
                0.0
            } else {
                (1.0 - (d / (size + glow.spread)).clamp(0.0, 1.0)).clamp(0.0, 1.0)
            };
            let mut a = glow.contour.apply(t);
            if inner {
                a *= src[i] as f32 / 255.0;
            } else {
                a *= 1.0 - src[i] as f32 / 255.0;
            }
            a *= glow.opacity;
            if a <= 0.001 {
                continue;
            }
            let blended = blend(glow.blend, pixel(out, x, y), glow.color, a * (glow.color.a as f32 / 255.0));
            put(out, x, y, blended);
        }
    }
}

fn blit_stroke(out: &mut RgbaImage, src: &[u8], sdf: &[f32], w: u32, h: u32, stroke: &Stroke) {
    let size = stroke.size.max(0.35);
    // Never eat a thin glyph (e.g. '+') into a hollow ring.
    let thick = max_inside(sdf).max(0.6);
    let inner_size = size.min(thick * 0.85);
    let cov = match stroke.position {
        StrokePosition::Outer => band(sdf, 0.0, size),
        StrokePosition::Inner => band(sdf, -inner_size, 0.0),
        StrokePosition::Center => {
            let half = (size * 0.5).min(thick * 0.7);
            band(sdf, -half, half)
        }
    };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let mut a = cov[i] * stroke.opacity;
            if stroke.position == StrokePosition::Inner {
                a *= src[i] as f32 / 255.0;
            }
            if a <= 0.001 {
                continue;
            }
            let blended = blend(stroke.blend, pixel(out, x, y), stroke.color, a * (stroke.color.a as f32 / 255.0));
            put(out, x, y, blended);
        }
    }
}

fn blit_bevel(out: &mut RgbaImage, src: &[u8], sdf: &[f32], w: u32, h: u32, style: &StyleStack) {
    let b = &style.bevel;
    let ang = style.global_light.angle_deg.to_radians();
    let alt = style.global_light.altitude_deg.to_radians();
    let lx = ang.cos() * alt.cos();
    let ly = -ang.sin() * alt.cos();
    let lz = alt.sin();
    let size = b.size.max(0.5);
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = (y * w + x) as usize;
            let m = src[i] as f32 / 255.0;
            if m <= 0.0 {
                continue;
            }
            let dx = sdf[i + 1] - sdf[i - 1];
            let dy = sdf[i + w as usize] - sdf[i - w as usize];
            let inv = (dx * dx + dy * dy + 1.0).sqrt();
            let nx = -dx / inv;
            let ny = -dy / inv;
            let nz = 1.0 / inv;
            let ndotl = (nx * lx + ny * ly + nz * lz) * b.depth;
            let in_band = smoothstep(-size, 0.0, sdf[i]).min(1.0 - smoothstep(-0.2, 0.8, sdf[i].abs()));
            if ndotl > 0.0 {
                let a = ndotl.clamp(0.0, 1.0) * b.opacity * m * in_band.max(0.25);
                let px = premul_over(pixel(out, x, y), scale_rgba(b.highlight, a));
                put(out, x, y, px);
            } else {
                let a = (-ndotl).clamp(0.0, 1.0) * b.opacity * m * in_band.max(0.25);
                let px = premul_over(pixel(out, x, y), scale_rgba(b.shadow, a));
                put(out, x, y, px);
            }
        }
    }
}

fn blit_satin(out: &mut RgbaImage, src: &[u8], w: u32, h: u32, satin: &Satin) {
    let rad = satin.angle_deg.to_radians();
    let ox = (rad.cos() * satin.distance).round() as i32;
    let oy = (-rad.sin() * satin.distance).round() as i32;
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let a0 = sample_u8(src, w, h, x, y) as f32 / 255.0;
            if a0 <= 0.0 {
                continue;
            }
            let a1 = sample_u8(src, w, h, x + ox, y + oy) as f32 / 255.0;
            let a2 = sample_u8(src, w, h, x - ox, y - oy) as f32 / 255.0;
            let xor = (a1 - a2).abs();
            let a = xor * a0 * satin.opacity * (satin.color.a as f32 / 255.0);
            if a <= 0.001 {
                continue;
            }
            let px = premul_over(pixel(out, x as u32, y as u32), scale_rgba(satin.color, a));
            put(out, x as u32, y as u32, px);
        }
    }
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

fn blend(mode: BlendMode, dst: Rgba<u8>, src: Rgba8, a: f32) -> Rgba<u8> {
    let da = dst[3] as f32 / 255.0;
    if da <= 0.0 {
        return scale_rgba(src, a);
    }
    let dr = dst[0] as f32 / 255.0;
    let dg = dst[1] as f32 / 255.0;
    let db = dst[2] as f32 / 255.0;
    let sr = src.r as f32 / 255.0;
    let sg = src.g as f32 / 255.0;
    let sb = src.b as f32 / 255.0;
    let ch = |d: f32, s: f32| match mode {
        BlendMode::Normal => s,
        BlendMode::Multiply => d * s,
        BlendMode::Screen => 1.0 - (1.0 - d) * (1.0 - s),
        BlendMode::Overlay => {
            if d < 0.5 {
                2.0 * d * s
            } else {
                1.0 - 2.0 * (1.0 - d) * (1.0 - s)
            }
        }
        BlendMode::LinearDodge => (d + s).min(1.0),
        BlendMode::ColorDodge => {
            if s >= 1.0 {
                1.0
            } else {
                (d / (1.0 - s).max(1e-5)).min(1.0)
            }
        }
        BlendMode::LinearBurn => (d + s - 1.0).max(0.0),
        BlendMode::ColorBurn => {
            if s <= 0.0 {
                0.0
            } else {
                (1.0 - (1.0 - d) / s.max(1e-5)).max(0.0)
            }
        }
    };
    let rr = ch(dr, sr);
    let gg = ch(dg, sg);
    let bb = ch(db, sb);
    let aa = a.clamp(0.0, 1.0);
    let out_r = rr * aa + dr * (1.0 - aa);
    let out_g = gg * aa + dg * (1.0 - aa);
    let out_b = bb * aa + db * (1.0 - aa);
    let out_a = aa + da * (1.0 - aa);
    Rgba([
        (out_r * 255.0).round().clamp(0.0, 255.0) as u8,
        (out_g * 255.0).round().clamp(0.0, 255.0) as u8,
        (out_b * 255.0).round().clamp(0.0, 255.0) as u8,
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
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

    #[test]
    fn plus_stroke_keeps_center_filled() {
        // 11x11 plus: 3px arms. Inner/center stroke must not punch a hole.
        let mut mask = vec![0u8; 11 * 11];
        for y in 0..11 {
            for x in 0..11 {
                if (x >= 4 && x <= 6) || (y >= 4 && y <= 6) {
                    mask[y * 11 + x] = 255;
                }
            }
        }
        let mut style = StyleStack::default();
        style.fill = Fill::solid(Rgba8::new(232, 196, 74, 255));
        style.stroke.enabled = true;
        style.stroke.size = 2.0;
        style.stroke.position = StrokePosition::Outer;
        style.stroke.color = Rgba8::new(42, 24, 8, 255);
        let img = render_glyph(&mask, 11, 11, &style);
        let pad = style.padding() as u32;
        let cx = pad + 5;
        let cy = pad + 5;
        let p = img.get_pixel(cx, cy);
        assert!(p[3] > 80, "center of plus must stay filled, got {:?}", p);
        assert!(p[0] > 80, "center should not be a dark hollow ring, got {:?}", p);
    }
}

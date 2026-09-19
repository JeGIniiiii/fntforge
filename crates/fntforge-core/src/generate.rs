use crate::pack::pack_glyphs;
use crate::{Error, Project, Result};
use fntforge_fx::render_glyph;
use fontdue::{Font, FontSettings};
use image::{imageops, Rgba, RgbaImage};
use rayon::prelude::*;

const FALLBACK_LATIN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/DejaVuSans.ttf"
));
const FALLBACK_CJK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/cjk_hud.ttf"
));

#[derive(Clone, Debug)]
pub struct GlyphImage {
    pub id: u32,
    pub ch: char,
    pub image: RgbaImage,
    pub xoffset: i32,
    pub yoffset: i32,
    pub xadvance: i32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub page: u32,
    pub from_fallback: bool,
}

#[derive(Clone, Debug)]
pub struct GeneratedFont {
    pub face: String,
    pub size: f32,
    pub line_height: i32,
    pub base: i32,
    pub padding: i32,
    pub spacing: i32,
    pub scale_w: u32,
    pub scale_h: u32,
    pub glyphs: Vec<GlyphImage>,
    pub pages: Vec<RgbaImage>,
    pub kernings: Vec<(u32, u32, i32)>,
    pub missing: Vec<char>,
    pub fallback_used: Vec<char>,
}

pub fn generate(project: &Project) -> Result<GeneratedFont> {
    generate_scaled(project, 1.0)
}

pub fn generate_scaled(project: &Project, scale: f32) -> Result<GeneratedFont> {
    let settings = FontSettings {
        collection_index: 0,
        scale: project.font_size * scale,
        ..FontSettings::default()
    };
    let primary = Font::from_bytes(project.font_bytes.as_slice(), settings.clone())
        .map_err(|e| Error::Font(e.to_string()))?;
    let fb_latin = Font::from_bytes(FALLBACK_LATIN, settings.clone())
        .map_err(|e| Error::Font(e.to_string()))?;
    let fb_cjk = Font::from_bytes(FALLBACK_CJK, settings).map_err(|e| Error::Font(e.to_string()))?;

    let px = project.font_size * scale;
    let line = primary
        .horizontal_line_metrics(px)
        .ok_or_else(|| Error::Font("missing line metrics".into()))?;
    let ascent = line.ascent;
    let descent = line.descent;
    let line_gap = line.line_gap;
    let mut line_height =
        (ascent - descent + line_gap).round() as i32 + (project.extra_line_height as f32 * scale).round() as i32;
    let base = ascent.round() as i32;

    let chars: Vec<char> = {
        let mut v: Vec<char> = crate::charset::extract_chars(&project.chars).chars().collect();
        v.sort_by_key(|c| *c as u32);
        v
    };

    let fonts = Fonts {
        primary: &primary,
        latin: &fb_latin,
        cjk: &fb_cjk,
    };

    let rendered: Vec<Rastered> = chars
        .par_iter()
        .map(|&ch| raster_one(&fonts, ch, px, project, scale))
        .collect();

    let mut missing = Vec::new();
    let mut fallback_used = Vec::new();
    let mut glyphs = Vec::new();
    for r in rendered {
        if r.missing {
            missing.push(r.ch);
            continue;
        }
        if r.from_fallback {
            fallback_used.push(r.ch);
        }
        glyphs.push(r.glyph);
    }

    if glyphs.is_empty() {
        return Err(Error::Empty);
    }

    if project.tabular_nums {
        let max_adv = glyphs
            .iter()
            .filter(|g| g.ch.is_ascii_digit())
            .map(|g| g.xadvance)
            .max()
            .unwrap_or(0);
        for g in &mut glyphs {
            if g.ch.is_ascii_digit() {
                let extra = max_adv - g.xadvance;
                g.xoffset += extra / 2;
                g.xadvance = max_adv;
            }
        }
    }

    for g in &glyphs {
        line_height = line_height.max(g.yoffset + g.image.height() as i32);
    }
    line_height = line_height.max(base - (descent * scale).round() as i32);

    let sizes: Vec<(u32, u32)> = glyphs
        .iter()
        .map(|g| (g.image.width().max(1), g.image.height().max(1)))
        .collect();
    let (packed, pages_meta) = pack_glyphs(&sizes, &project.pack)?;

    let mut pages: Vec<RgbaImage> = pages_meta
        .iter()
        .map(|p| RgbaImage::from_pixel(p.width.max(1), p.height.max(1), Rgba([0, 0, 0, 0])))
        .collect();

    for (i, g) in glyphs.iter_mut().enumerate() {
        let p = &packed[i];
        g.x = p.x;
        g.y = p.y;
        g.width = p.w;
        g.height = p.h;
        g.page = p.page;
        blit(&mut pages[p.page as usize], &g.image, p.x, p.y);
    }

    let scale_w = pages.first().map(|p| p.width()).unwrap_or(1);
    let scale_h = pages.first().map(|p| p.height()).unwrap_or(1);
    let kernings = read_kerning(&project.font_bytes, &glyphs, px);

    Ok(GeneratedFont {
        face: project.font_name.clone(),
        size: px,
        line_height,
        base,
        padding: project.style.padding(),
        spacing: project.pack.spacing as i32,
        scale_w,
        scale_h,
        glyphs,
        pages,
        kernings,
        missing,
        fallback_used,
    })
}

struct Fonts<'a> {
    primary: &'a Font,
    latin: &'a Font,
    cjk: &'a Font,
}

struct Rastered {
    ch: char,
    glyph: GlyphImage,
    missing: bool,
    from_fallback: bool,
}

fn has_glyph(font: &Font, ch: char) -> bool {
    font.lookup_glyph_index(ch) != 0
}

fn looks_like_hollow_box(bitmap: &[u8], w: u32, h: u32) -> bool {
    if w < 5 || h < 5 || bitmap.is_empty() {
        return false;
    }
    let cx = w / 2;
    let cy = h / 2;
    let center = bitmap[(cy * w + cx) as usize];
    let mut rim = 0u32;
    let mut n = 0u32;
    for x in 0..w {
        rim += bitmap[x as usize] as u32;
        rim += bitmap[((h - 1) * w + x) as usize] as u32;
        n += 2;
    }
    center < 32 && n > 0 && (rim / n) > 64
}

fn pick_font<'a>(fonts: &'a Fonts<'a>, ch: char, ascii_fallback: bool) -> (&'a Font, bool) {
    let primary_ok = has_glyph(fonts.primary, ch) || ch == ' ';
    if primary_ok && !(ascii_fallback && ch.is_ascii() && !ch.is_ascii_alphabetic() && ch != ' ' && !ch.is_ascii_digit()) {
        return (fonts.primary, false);
    }
    // Decorative CJK title fonts often ship a circled/hollow '+' (.notdef-like).
    if ch.is_ascii() && has_glyph(fonts.latin, ch) {
        if !primary_ok || (ascii_fallback && !ch.is_ascii_alphabetic() && !ch.is_ascii_digit() && ch != ' ') {
            return (fonts.latin, true);
        }
    }
    if !primary_ok && has_glyph(fonts.cjk, ch) {
        return (fonts.cjk, true);
    }
    if primary_ok {
        (fonts.primary, false)
    } else if has_glyph(fonts.latin, ch) {
        (fonts.latin, true)
    } else {
        (fonts.primary, false)
    }
}

fn raster_one(fonts: &Fonts, ch: char, px: f32, project: &Project, scale: f32) -> Rastered {
    let (mut font, mut from_fallback) = pick_font(fonts, ch, project.ascii_fallback);
    let (mut metrics, mut bitmap) = font.rasterize(ch, px);

    if looks_like_hollow_box(&bitmap, metrics.width as u32, metrics.height as u32) && ch != ' ' {
        if has_glyph(fonts.latin, ch) && !std::ptr::eq(font, fonts.latin) {
            font = fonts.latin;
            from_fallback = true;
            let r = font.rasterize(ch, px);
            metrics = r.0;
            bitmap = r.1;
        } else if ch != ' ' && font.lookup_glyph_index(ch) == 0 {
            return Rastered {
                ch,
                glyph: empty_glyph(ch, px, project, scale),
                missing: true,
                from_fallback: false,
            };
        }
    }

    if ch != ' ' && font.lookup_glyph_index(ch) == 0 && !from_fallback {
        return Rastered {
            ch,
            glyph: empty_glyph(ch, px, project, scale),
            missing: true,
            from_fallback: false,
        };
    }

    let mw = metrics.width as u32;
    let mh = metrics.height as u32;
    let over = project.oversample.max(1);
    let img = if mw == 0 || mh == 0 {
        RgbaImage::new(1, 1)
    } else if over > 1 {
        let (m2, b2) = font.rasterize(ch, px * over as f32);
        let hi = render_glyph(&b2, m2.width as u32, m2.height as u32, &project.style);
        downsample(&hi, over)
    } else {
        render_glyph(&bitmap, mw, mh, &project.style)
    };
    let pad = project.style.padding();
    let top_from_baseline = metrics.ymin + metrics.height as i32;
    let yoffset = font
        .horizontal_line_metrics(px)
        .map(|m| m.ascent.round() as i32)
        .unwrap_or(px.round() as i32)
        - top_from_baseline
        - pad;
    let xoffset = metrics.xmin - pad;
    let xadvance = metrics.advance_width.round() as i32
        + (project.extra_letter_spacing as f32 * scale).round() as i32;

    Rastered {
        ch,
        missing: false,
        from_fallback,
        glyph: GlyphImage {
            id: ch as u32,
            ch,
            image: img,
            xoffset,
            yoffset,
            xadvance,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            page: 0,
            from_fallback,
        },
    }
}

fn empty_glyph(ch: char, px: f32, project: &Project, scale: f32) -> GlyphImage {
    GlyphImage {
        id: ch as u32,
        ch,
        image: RgbaImage::new(1, 1),
        xoffset: 0,
        yoffset: 0,
        xadvance: (px * 0.5).round() as i32 + (project.extra_letter_spacing as f32 * scale).round() as i32,
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        page: 0,
        from_fallback: false,
    }
}

fn downsample(img: &RgbaImage, n: u32) -> RgbaImage {
    let w = (img.width() / n).max(1);
    let h = (img.height() / n).max(1);
    imageops::resize(img, w, h, imageops::FilterType::Triangle)
}

fn blit(dst: &mut RgbaImage, src: &RgbaImage, x: u32, y: u32) {
    for (px, py, p) in src.enumerate_pixels() {
        let dx = x + px;
        let dy = y + py;
        if dx < dst.width() && dy < dst.height() {
            dst.put_pixel(dx, dy, *p);
        }
    }
}

fn read_kerning(font_bytes: &[u8], glyphs: &[GlyphImage], px: f32) -> Vec<(u32, u32, i32)> {
    let Ok(face) = ttf_parser::Face::parse(font_bytes, 0) else {
        return Vec::new();
    };
    let Some(kern) = face.tables().kern else {
        return Vec::new();
    };
    let units = face.units_per_em() as f32;
    if units <= 0.0 {
        return Vec::new();
    }
    let scale = px / units;
    let ids: Vec<(u32, ttf_parser::GlyphId)> = glyphs
        .iter()
        .filter_map(|g| {
            face.glyph_index(g.ch).map(|gid| (g.id, gid))
        })
        .collect();
    let mut out = Vec::new();
    for &(a, ga) in &ids {
        for &(b, gb) in &ids {
            for st in kern.subtables {
                if let Some(v) = st.glyphs_kerning(ga, gb) {
                    let amt = (v as f32 * scale).round() as i32;
                    if amt != 0 {
                        out.push((a, b, amt));
                    }
                    break;
                }
            }
        }
    }
    out
}

pub fn lua_snippet(stem: &str, preview: &str) -> String {
    let preview = preview.lines().next().unwrap_or("TEXT").replace('"', "\\\"");
    format!(
        "-- FntForge 生成，把 {stem}.fnt 与 PNG 放在同一目录\n\
         local lb = cc.Label:createWithBMFont(\"fonts/{stem}.fnt\", \"{preview}\", cc.TEXT_ALIGNMENT_CENTER)\n\
         self:addChild(lb)\n"
    )
}

pub fn missing_report(font: &GeneratedFont) -> String {
    if font.missing.is_empty() && font.fallback_used.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    if !font.missing.is_empty() {
        s.push_str("缺字（源字体与后备字体都没有，未写入图集）：\n");
        for c in &font.missing {
            s.push_str(&format!("U+{:04X} {}\n", *c as u32, c));
        }
    }
    if !font.fallback_used.is_empty() {
        s.push_str("\n以下字符使用了后备字体（常见于标题美工字体缺「+」等符号）：\n");
        for c in &font.fallback_used {
            s.push_str(&format!("U+{:04X} {}\n", *c as u32, c));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fnt::write_fnt;
    use crate::Project;
    use fntforge_fx::{Fill, Rgba8};

    fn load() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../testdata/DejaVuSans.ttf"
        ))
        .expect("testdata font")
    }

    #[test]
    fn generates_ascii_fnt() {
        let mut project = Project::new(load(), "DejaVuSans");
        project.font_size = 32.0;
        project.chars = "ABCgj 中+".into();
        project.style.stroke.enabled = true;
        project.style.drop_shadow.enabled = true;
        let font = generate(&project).unwrap();
        assert!(!font.glyphs.is_empty());
        assert!(!font.pages.is_empty());
        let fnt = write_fnt(&font, "test");
        assert!(fnt.text.contains("info face="));
        assert!(fnt.text.contains("char id="));
        assert!(fnt.text.contains("page id=0 file=\"test.png\""));
        assert!(font.glyphs.iter().any(|g| g.id == 32));
        assert!(font.glyphs.iter().any(|g| g.ch == '+'));
    }

    #[test]
    fn plus_center_is_solid_gold() {
        let mut project = Project::new(load(), "DejaVuSans");
        project.font_size = 48.0;
        project.chars = "+".into();
        project.style = fntforge_fx::StyleStack::preset_gold();
        let font = generate(&project).unwrap();
        let plus = font.glyphs.iter().find(|g| g.ch == '+').unwrap();
        let img = &plus.image;
        let opaque: Vec<(u32, u32)> = img
            .enumerate_pixels()
            .filter(|(_, _, p)| p[3] > 80)
            .map(|(x, y, _)| (x, y))
            .collect();
        assert!(opaque.len() > 20, "plus should have body pixels");
        let minx = opaque.iter().map(|p| p.0).min().unwrap();
        let maxx = opaque.iter().map(|p| p.0).max().unwrap();
        let miny = opaque.iter().map(|p| p.1).min().unwrap();
        let maxy = opaque.iter().map(|p| p.1).max().unwrap();
        let cx = (minx + maxx) / 2;
        let cy = (miny + maxy) / 2;
        let p = img.get_pixel(cx, cy);
        assert!(p[3] > 80, "plus center must not be a hole, got {:?}", p);
        assert!(
            p[0] as u16 + p[1] as u16 > 120,
            "plus center should stay gold, not a dark ring, got {:?}",
            p
        );
    }

    #[test]
    fn fill_solid_constructor() {
        let _ = Fill::solid(Rgba8::WHITE);
    }
}

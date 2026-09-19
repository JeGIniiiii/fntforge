use crate::pack::pack_glyphs;
use crate::{Error, Project, Result};
use fntforge_fx::render_glyph;
use fontdue::{Font, FontSettings};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;

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
}

pub fn generate(project: &Project) -> Result<GeneratedFont> {
    let font = Font::from_bytes(
        project.font_bytes.as_slice(),
        FontSettings {
            collection_index: 0,
            scale: project.font_size,
            ..FontSettings::default()
        },
    )
    .map_err(|e| Error::Font(e.to_string()))?;

    let px = project.font_size;
    let line = font
        .horizontal_line_metrics(px)
        .ok_or_else(|| Error::Font("missing line metrics".into()))?;
    // fontdue: ascent is positive, descent is negative.
    let ascent = line.ascent;
    let descent = line.descent;
    let line_gap = line.line_gap;
    let mut line_height = (ascent - descent + line_gap).round() as i32 + project.extra_line_height;
    let base = ascent.round() as i32;

    let chars: Vec<char> = {
        let mut v: Vec<char> = crate::charset::extract_chars(&project.chars).chars().collect();
        v.sort_by_key(|c| *c as u32);
        v
    };

    let rendered: Vec<GlyphImage> = chars
        .par_iter()
        .map(|&ch| raster_one(&font, ch, px, project))
        .collect();

    let mut glyphs = rendered;
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
    line_height = line_height.max(base - descent.round() as i32);

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
        blit(
            &mut pages[p.page as usize],
            &g.image,
            p.x,
            p.y,
        );
    }

    let scale_w = pages.first().map(|p| p.width()).unwrap_or(1);
    let scale_h = pages.first().map(|p| p.height()).unwrap_or(1);

    Ok(GeneratedFont {
        face: project.font_name.clone(),
        size: project.font_size,
        line_height,
        base,
        padding: project.style.padding(),
        spacing: project.pack.spacing as i32,
        scale_w,
        scale_h,
        glyphs,
        pages,
        kernings: Vec::new(),
    })
}

fn raster_one(font: &Font, ch: char, px: f32, project: &Project) -> GlyphImage {
    let (metrics, bitmap) = font.rasterize(ch, px);
    let mw = metrics.width as u32;
    let mh = metrics.height as u32;
    let img = if mw == 0 || mh == 0 {
        RgbaImage::new(1, 1)
    } else {
        render_glyph(&bitmap, mw, mh, &project.style)
    };
    let pad = project.style.padding();
    // fontdue ymin: bottom of bbox, y-up from baseline.
    // Bitmap top relative to baseline = ymin + height.
    let top_from_baseline = metrics.ymin + metrics.height as i32;
    let yoffset = project.base_adjust(font, px) - top_from_baseline - pad;
    let xoffset = metrics.xmin - pad;
    let xadvance = metrics.advance_width.round() as i32 + project.extra_letter_spacing;

    GlyphImage {
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
    }
}

impl Project {
    fn base_adjust(&self, font: &Font, px: f32) -> i32 {
        font.horizontal_line_metrics(px)
            .map(|m| m.ascent.round() as i32)
            .unwrap_or(px.round() as i32)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fnt::write_fnt;
    use crate::Project;

    #[test]
    fn generates_ascii_fnt() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../testdata/DejaVuSans.ttf"
        ))
        .expect("testdata font");
        let mut project = Project::new(bytes, "DejaVuSans");
        project.font_size = 32.0;
        project.chars = "ABCgj 中".into();
        project.style.stroke.enabled = true;
        project.style.drop_shadow.enabled = true;
        let font = generate(&project).unwrap();
        assert!(!font.glyphs.is_empty());
        assert!(!font.pages.is_empty());
        let fnt = write_fnt(&font, "test");
        assert!(fnt.text.contains("info face="));
        assert!(fnt.text.contains("char id="));
        assert!(fnt.text.contains("page id=0 file=\"test.png\""));
        let space = font.glyphs.iter().find(|g| g.id == 32);
        assert!(space.is_some());
    }
}

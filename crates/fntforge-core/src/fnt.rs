use crate::generate::GeneratedFont;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct BmFont {
    pub text: String,
}

pub fn write_fnt(font: &GeneratedFont, png_stem: &str) -> BmFont {
    let mut s = String::new();
    let face = sanitize_face(&font.face);
    s.push_str(&format!(
        "info face=\"{face}\" size={size} bold=0 italic=0 charset=\"\" unicode=1 stretchH=100 smooth=1 aa=1 padding={p},{p},{p},{p} spacing={sp},{sp} outline=0\n",
        size = font.size.round() as i32,
        p = font.padding,
        sp = font.spacing,
    ));
    s.push_str(&format!(
        "common lineHeight={lh} base={base} scaleW={w} scaleH={h} pages={pages} packed=0 alphaChnl=0 redChnl=0 greenChnl=0 blueChnl=0\n",
        lh = font.line_height,
        base = font.base,
        w = font.scale_w,
        h = font.scale_h,
        pages = font.pages.len(),
    ));
    for (i, page) in font.pages.iter().enumerate() {
        let file = if font.pages.len() == 1 {
            format!("{png_stem}.png")
        } else {
            format!("{png_stem}_{i}.png")
        };
        let _ = page;
        s.push_str(&format!("page id={i} file=\"{file}\"\n"));
    }
    s.push_str(&format!("chars count={}\n", font.glyphs.len()));
    let mut glyphs = font.glyphs.clone();
    glyphs.sort_by_key(|g| g.id);
    for g in &glyphs {
        let letter = display_letter(g.id);
        s.push_str(&format!(
            "char id={id:<5} x={x:<5} y={y:<5} width={w:<5} height={h:<5} xoffset={xo:<5} yoffset={yo:<5} xadvance={xa:<5} page={page}  chnl=15 letter=\"{letter}\"\n",
            id = g.id,
            x = g.x,
            y = g.y,
            w = g.width,
            h = g.height,
            xo = g.xoffset,
            yo = g.yoffset,
            xa = g.xadvance,
            page = g.page,
        ));
    }
    if !font.kernings.is_empty() {
        s.push_str(&format!("kernings count={}\n", font.kernings.len()));
        for k in &font.kernings {
            s.push_str(&format!(
                "kerning first={:<5} second={:<5} amount={}\n",
                k.0, k.1, k.2
            ));
        }
    }
    BmFont { text: s }
}

pub fn write_files(font: &GeneratedFont, dir: &Path, stem: &str) -> io::Result<Vec<std::path::PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let desc = write_fnt(font, stem);
    let fnt_path = dir.join(format!("{stem}.fnt"));
    let mut f = std::fs::File::create(&fnt_path)?;
    f.write_all(desc.text.as_bytes())?;
    let mut paths = vec![fnt_path];
    for (i, page) in font.pages.iter().enumerate() {
        let name = if font.pages.len() == 1 {
            format!("{stem}.png")
        } else {
            format!("{stem}_{i}.png")
        };
        let p = dir.join(&name);
        page.save(&p).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        paths.push(p);
    }
    Ok(paths)
}

fn sanitize_face(name: &str) -> String {
    name.replace('"', "'")
}

fn display_letter(id: u32) -> String {
    char::from_u32(id)
        .map(|c| match c {
            '"' => "\\\"".into(),
            '\n' => "\\n".into(),
            _ => c.to_string(),
        })
        .unwrap_or_default()
}

use crate::generate::GeneratedFont;
use crate::{Error, Result};
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct BmFont {
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct ImportedFnt {
    pub face: String,
    pub size: i32,
    pub padding: i32,
    pub spacing: i32,
    pub line_height: i32,
    pub base: i32,
    pub scale_w: u32,
    pub scale_h: u32,
    pub pages: Vec<String>,
    pub chars: String,
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
    let miss = crate::generate::missing_report(font);
    if !miss.is_empty() {
        let p = dir.join(format!("{stem}_missing.txt"));
        std::fs::write(&p, miss)?;
        paths.push(p);
    }
    let lua = crate::generate::lua_snippet(stem, "TEXT");
    let p = dir.join(format!("{stem}_sample.lua"));
    std::fs::write(&p, lua)?;
    paths.push(p);
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

/// Read an existing AngelCode text `.fnt` so we can reuse its charset / size and append glyphs.
pub fn parse_fnt(text: &str) -> Result<ImportedFnt> {
    if !text.contains("char id=") && !text.contains("chars count=") {
        return Err(Error::Fnt("不是 AngelCode 文本 .fnt".into()));
    }
    let mut face = String::new();
    let mut size = 32i32;
    let mut padding = 1i32;
    let mut spacing = 1i32;
    let mut line_height = 0i32;
    let mut base = 0i32;
    let mut scale_w = 0u32;
    let mut scale_h = 0u32;
    let mut pages = Vec::new();
    let mut chars = String::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("info ") {
            if let Some(v) = kv_str(line, "face") {
                face = v;
            }
            if let Some(v) = kv_i(line, "size") {
                size = v;
            }
            if let Some(v) = kv_str(line, "padding") {
                if let Some(first) = v.split(',').next() {
                    padding = first.trim().parse().unwrap_or(padding);
                }
            }
            if let Some(v) = kv_str(line, "spacing") {
                if let Some(first) = v.split(',').next() {
                    spacing = first.trim().parse().unwrap_or(spacing);
                }
            }
        } else if line.starts_with("common ") {
            line_height = kv_i(line, "lineHeight").unwrap_or(line_height);
            base = kv_i(line, "base").unwrap_or(base);
            scale_w = kv_i(line, "scaleW").unwrap_or(0).max(0) as u32;
            scale_h = kv_i(line, "scaleH").unwrap_or(0).max(0) as u32;
        } else if line.starts_with("page ") {
            if let Some(f) = kv_str(line, "file") {
                pages.push(f);
            }
        } else if line.starts_with("char ") {
            if let Some(id) = kv_i(line, "id") {
                if let Some(c) = char::from_u32(id as u32) {
                    if !c.is_control() {
                        chars.push(c);
                    }
                }
            }
        }
    }
    let chars = crate::charset::extract_chars(&chars);
    if chars.chars().all(|c| c == ' ') {
        return Err(Error::Fnt(".fnt 里没有可用字符".into()));
    }
    Ok(ImportedFnt {
        face,
        size: size.max(1),
        padding,
        spacing,
        line_height,
        base,
        scale_w,
        scale_h,
        pages,
        chars,
    })
}

fn kv_str(line: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=");
    let rest = line.split(&pat).nth(1)?;
    if rest.starts_with('"') {
        let inner = rest.get(1..)?;
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        Some(rest.split_whitespace().next()?.to_string())
    }
}

fn kv_i(line: &str, key: &str) -> Option<i32> {
    kv_str(line, key)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_parse() {
        let sample = r#"info face="Gold" size=48 bold=0 italic=0 charset="" unicode=1 stretchH=100 smooth=1 aa=1 padding=2,2,2,2 spacing=1,1 outline=0
common lineHeight=56 base=40 scaleW=256 scaleH=128 pages=1 packed=0
page id=0 file="gold.png"
chars count=3
char id=32    x=0     y=0     width=1     height=1     xoffset=0     yoffset=0     xadvance=12    page=0  chnl=15 letter=" "
char id=43    x=10    y=0     width=20    height=20    xoffset=0     yoffset=8     xadvance=22    page=0  chnl=15 letter="+"
char id=37329 x=40    y=0     width=40    height=40    xoffset=0     yoffset=2     xadvance=42    page=0  chnl=15 letter="金"
"#;
        let i = parse_fnt(sample).unwrap();
        assert_eq!(i.face, "Gold");
        assert_eq!(i.size, 48);
        assert_eq!(i.padding, 2);
        assert!(i.chars.contains('+'));
        assert!(i.chars.contains('金'));
        assert_eq!(i.pages[0], "gold.png");
    }
}


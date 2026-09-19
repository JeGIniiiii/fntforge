//! Read-only Photoshop ASL / PSD layer-style import.
//! Never launches Photoshop. Unknown keys are ignored.

use fntforge_fx::{
    Fill, GradientOverlay, Overlay, Rgba8, StrokePosition, StyleStack,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, ImportError>;

#[derive(Clone, Debug)]
pub struct PsdLayer {
    pub name: String,
    pub kind: String,
}

/// Import a `.asl` style sheet. Uses the first style in the file.
pub fn import_asl(bytes: &[u8]) -> Result<StyleStack> {
    parse_asl(bytes)
}

/// List layers in a PSD so the user can pick a text layer.
pub fn list_psd_layers(bytes: &[u8]) -> Result<Vec<PsdLayer>> {
    if bytes.len() < 26 || &bytes[0..4] != b"8BPS" {
        return Err(ImportError::Parse("不是 PSD 文件".into()));
    }
    // Very small layer-name scanner: Pascal/Unicode names after '8BIM' blocks.
    let mut layers = Vec::new();
    let mut i = 0;
    while i + 8 < bytes.len() {
        if &bytes[i..i + 4] == b"8BIM" {
            i += 4;
            continue;
        }
        i += 1;
    }
    // Fallback: extract printable UTF-16LE runs that look like layer names.
    let mut run = Vec::new();
    let mut k = 0;
    while k + 1 < bytes.len() {
        let u = u16::from_le_bytes([bytes[k], bytes[k + 1]]);
        if (0x20..=0x7e).contains(&u) || (0x4e00..=0x9fff).contains(&u) {
            if let Some(c) = char::from_u32(u as u32) {
                run.push(c);
            }
            k += 2;
        } else {
            if run.len() >= 2 {
                let name: String = run.iter().collect();
                if !name.chars().all(|c| c.is_ascii_digit()) {
                    layers.push(PsdLayer {
                        name,
                        kind: "layer".into(),
                    });
                }
            }
            run.clear();
            k += 1;
        }
    }
    layers.truncate(64);
    if layers.is_empty() {
        layers.push(PsdLayer {
            name: "背景".into(),
            kind: "layer".into(),
        });
    }
    Ok(layers)
}

/// Pull a StyleStack out of lfx2-ish bytes (ASL or a PSD extra).
pub fn style_from_descriptor_bytes(bytes: &[u8]) -> StyleStack {
    let mut style = StyleStack::default();
    apply_keys(bytes, &mut style);
    style
}

fn parse_asl(bytes: &[u8]) -> Result<StyleStack> {
    if bytes.len() < 8 {
        return Err(ImportError::Parse("ASL 文件太短".into()));
    }
    let mut style = StyleStack::default();
    apply_keys(bytes, &mut style);
    Ok(style)
}

fn apply_keys(bytes: &[u8], style: &mut StyleStack) {
    let colors = find_rgbs(bytes);
    if let Some(c) = colors.first() {
        style.fill = Fill::solid(*c);
    }
    if bytes_contains(bytes, b"DrSh") || bytes_contains(bytes, b"dropShadow") {
        style.drop_shadow.enabled = true;
        if let Some(c) = colors.get(1) {
            style.drop_shadow.color = c.with_alpha(160);
        }
        if let Some(v) = find_unit(bytes, b"Dstn") {
            style.drop_shadow.distance = v;
        }
        if let Some(v) = find_unit(bytes, b"blur") {
            style.drop_shadow.size = v;
        }
    }
    if bytes_contains(bytes, b"OrGl") || bytes_contains(bytes, b"outerGlow") {
        style.outer_glow.enabled = true;
        if let Some(c) = colors.get(2).or(colors.first()) {
            style.outer_glow.color = c.with_alpha(180);
        }
        if let Some(v) = find_unit(bytes, b"blur") {
            style.outer_glow.size = v.max(1.0);
        }
    }
    if bytes_contains(bytes, b"IrSh") {
        style.inner_shadow.enabled = true;
    }
    if bytes_contains(bytes, b"IrGl") {
        style.inner_glow.enabled = true;
    }
    if bytes_contains(bytes, b"FrFX") || bytes_contains(bytes, b"frameFX") {
        style.stroke.enabled = true;
        if let Some(v) = find_unit(bytes, b"Sz  ").or_else(|| find_unit(bytes, b"Sz")) {
            style.stroke.size = v.max(0.5);
        }
        if bytes_contains(bytes, b"OutF") {
            style.stroke.position = StrokePosition::Outer;
        } else if bytes_contains(bytes, b"InsF") {
            style.stroke.position = StrokePosition::Inner;
        } else if bytes_contains(bytes, b"CtrF") {
            style.stroke.position = StrokePosition::Center;
        }
        if let Some(c) = colors.last() {
            style.stroke.color = *c;
        }
    }
    if bytes_contains(bytes, b"SoFi") {
        style.color_overlay.enabled = true;
        if let Some(c) = colors.first() {
            style.color_overlay.color = *c;
            style.color_overlay = Overlay {
                enabled: true,
                color: *c,
                opacity: 1.0,
                blend: Default::default(),
            };
        }
    }
    if bytes_contains(bytes, b"GrFl") || bytes_contains(bytes, b"GdFl") {
        style.gradient_overlay.enabled = true;
        if colors.len() >= 2 {
            style.gradient_overlay = GradientOverlay {
                enabled: true,
                angle_deg: find_unit(bytes, b"Angl").unwrap_or(90.0),
                opacity: 1.0,
                blend: Default::default(),
                stops: colors
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i as f32 / (colors.len() - 1) as f32, *c))
                    .collect(),
            };
        }
    }
    if bytes_contains(bytes, b"ebbl") {
        style.bevel.enabled = true;
    }
}

fn bytes_contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn find_unit(bytes: &[u8], key: &[u8]) -> Option<f32> {
    let mut i = 0;
    while i + key.len() + 12 < bytes.len() {
        if &bytes[i..i + key.len()] == key {
            let t = &bytes[i + key.len()..i + key.len() + 4];
            if t == b"UntF" || t == b"doub" {
                let off = if t == b"UntF" { 8 } else { 4 };
                let start = i + key.len() + off;
                if start + 8 <= bytes.len() {
                    let bits = u64::from_be_bytes(bytes[start..start + 8].try_into().ok()?);
                    let v = f64::from_bits(bits) as f32;
                    if v.is_finite() && v.abs() < 10_000.0 {
                        return Some(v.abs());
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn find_rgbs(bytes: &[u8]) -> Vec<Rgba8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < bytes.len() {
        if &bytes[i..i + 4] == b"Rd  " || &bytes[i..i + 4] == b"Rd\0\0" {
            if let (Some(r), Some(g), Some(b)) = (
                read_f64_after(bytes, i + 4),
                find_next_f64_key(bytes, i, b"Grn "),
                find_next_f64_key(bytes, i, b"Bl  "),
            ) {
                out.push(Rgba8::new(clamp_ch(r), clamp_ch(g), clamp_ch(b), 255));
            }
        }
        i += 1;
        if out.len() > 12 {
            break;
        }
    }
    out
}

fn find_next_f64_key(bytes: &[u8], from: usize, key: &[u8]) -> Option<f64> {
    let end = (from + 80).min(bytes.len().saturating_sub(key.len() + 12));
    let mut i = from;
    while i < end {
        if &bytes[i..i + key.len()] == key {
            return read_f64_after(bytes, i + key.len());
        }
        i += 1;
    }
    None
}

fn read_f64_after(bytes: &[u8], mut i: usize) -> Option<f64> {
    // skip type tag if present
    if i + 4 <= bytes.len() {
        let t = &bytes[i..i + 4];
        if t == b"doub" || t == b"UntF" {
            i += 4;
            if t == b"UntF" {
                i += 4;
            }
        }
    }
    if i + 8 > bytes.len() {
        return None;
    }
    let bits = u64::from_be_bytes(bytes[i..i + 8].try_into().ok()?);
    let v = f64::from_bits(bits);
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

fn clamp_ch(v: f64) -> u8 {
    // PS stores 0..255 or 0..1
    let x = if v <= 1.0 && v >= 0.0 { v * 255.0 } else { v };
    x.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asl_too_short() {
        assert!(import_asl(&[0, 1, 2]).is_err());
    }

    #[test]
    fn psd_header_reject() {
        assert!(list_psd_layers(b"not a psd").is_err());
    }
}

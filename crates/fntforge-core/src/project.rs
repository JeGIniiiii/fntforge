use crate::pack::PackOptions;
use fntforge_fx::StyleStack;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignH {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignV {
    Top,
    Middle,
    Baseline,
    Bottom,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub font_bytes: Vec<u8>,
    pub font_name: String,
    pub font_path: Option<PathBuf>,
    pub font_size: f32,
    pub chars: String,
    pub style: StyleStack,
    pub extra_letter_spacing: i32,
    pub extra_line_height: i32,
    pub tabular_nums: bool,
    pub preview_text: String,
    pub align_h: AlignH,
    pub align_v: AlignV,
    pub pack: PackOptions,
    /// Use bundled Latin/CJK fallback when the face is missing a glyph (fixes decorative '+' rings).
    pub ascii_fallback: bool,
    /// Super-sample raster (1 or 2).
    pub oversample: u32,
    /// Extra export scales besides 1x, e.g. [2.0] writes @2x.
    pub extra_scales: Vec<f32>,
}

impl Project {
    pub fn new(font_bytes: Vec<u8>, font_name: impl Into<String>) -> Self {
        Self {
            font_bytes,
            font_name: font_name.into(),
            font_path: None,
            font_size: 48.0,
            chars: crate::charset::preset_chars(crate::charset::CharsetPreset::GameHud),
            style: StyleStack::preset_gold(),
            extra_letter_spacing: 0,
            extra_line_height: 0,
            tabular_nums: true,
            preview_text: "金币 1111HP+".into(),
            align_h: AlignH::Center,
            align_v: AlignV::Baseline,
            pack: PackOptions::default(),
            ascii_fallback: true,
            oversample: 1,
            extra_scales: vec![2.0],
        }
    }

    pub fn font_hash(&self) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in &self.font_bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{:016x}-{}", h, self.font_bytes.len())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    pub font_path: Option<String>,
    pub font_hash: String,
    pub font_name: String,
    pub font_size: f32,
    pub chars: String,
    pub preview_text: String,
    pub tabular_nums: bool,
    pub extra_letter_spacing: i32,
    pub extra_line_height: i32,
    pub ascii_fallback: bool,
    pub oversample: u32,
    pub extra_scales: Vec<f32>,
    pub align_h: AlignH,
    pub align_v: AlignV,
    pub pack: PackOptions,
    pub style: StyleStack,
}

impl ProjectFile {
    pub fn from_project(p: &Project) -> Self {
        Self {
            version: 1,
            font_path: p.font_path.as_ref().map(|x| x.to_string_lossy().into_owned()),
            font_hash: p.font_hash(),
            font_name: p.font_name.clone(),
            font_size: p.font_size,
            chars: p.chars.clone(),
            preview_text: p.preview_text.clone(),
            tabular_nums: p.tabular_nums,
            extra_letter_spacing: p.extra_letter_spacing,
            extra_line_height: p.extra_line_height,
            ascii_fallback: p.ascii_fallback,
            oversample: p.oversample,
            extra_scales: p.extra_scales.clone(),
            align_h: p.align_h,
            align_v: p.align_v,
            pack: p.pack.clone(),
            style: p.style.clone(),
        }
    }

    pub fn apply_to(&self, p: &mut Project) {
        p.font_name = self.font_name.clone();
        p.font_size = self.font_size;
        p.chars = self.chars.clone();
        p.preview_text = self.preview_text.clone();
        p.tabular_nums = self.tabular_nums;
        p.extra_letter_spacing = self.extra_letter_spacing;
        p.extra_line_height = self.extra_line_height;
        p.ascii_fallback = self.ascii_fallback;
        p.oversample = self.oversample.max(1);
        p.extra_scales = self.extra_scales.clone();
        p.align_h = self.align_h;
        p.align_v = self.align_v;
        p.pack = self.pack.clone();
        p.style = self.style.clone();
        if let Some(s) = &self.font_path {
            p.font_path = Some(PathBuf::from(s));
        }
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let t = std::fs::read_to_string(path)?;
        serde_json::from_str(&t).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let t = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, t)
    }
}

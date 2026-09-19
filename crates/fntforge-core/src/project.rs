use fntforge_fx::StyleStack;
use serde::{Deserialize, Serialize};

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
    pub font_size: f32,
    pub chars: String,
    pub style: StyleStack,
    pub extra_letter_spacing: i32,
    pub extra_line_height: i32,
    pub tabular_nums: bool,
    pub preview_text: String,
    pub align_h: AlignH,
    pub align_v: AlignV,
    pub pack: crate::pack::PackOptions,
}

impl Project {
    pub fn new(font_bytes: Vec<u8>, font_name: impl Into<String>) -> Self {
        Self {
            font_bytes,
            font_name: font_name.into(),
            font_size: 48.0,
            chars: crate::charset::preset_chars(crate::charset::CharsetPreset::Ascii),
            style: StyleStack::default(),
            extra_letter_spacing: 0,
            extra_line_height: 0,
            tabular_nums: false,
            preview_text: "金币 +1280\nFntForge  Agj".into(),
            align_h: AlignH::Center,
            align_v: AlignV::Baseline,
            pack: crate::pack::PackOptions::default(),
        }
    }
}

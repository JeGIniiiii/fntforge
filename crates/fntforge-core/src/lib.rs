//! FntForge core: rasterize styled glyphs, pack an atlas, write AngelCode BMFont text.

mod charset;
mod fnt;
mod generate;
mod pack;
mod project;

pub use charset::{extract_chars, extract_from_source, preset_chars, CharsetPreset};
pub use fnt::{write_files as write_font_files, write_fnt, BmFont};
pub use generate::{
    generate, generate_scaled, lua_snippet, missing_report, GeneratedFont, GlyphImage,
};
pub use pack::{pack_glyphs, PackOptions, PackedGlyph, PackedPage};
pub use project::{AlignH, AlignV, Project, ProjectFile};

pub use fntforge_fx::{
    Bevel, BlendMode, Contour, Fill, Glow, GradientOverlay, Overlay, Rgba8, Satin, Shadow, Stroke,
    StrokePosition, StyleStack,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to parse font: {0}")]
    Font(String),
    #[error("no glyphs to pack")]
    Empty,
    #[error("glyph does not fit in atlas {0}x{0}")]
    AtlasTooSmall(u32),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("image: {0}")]
    Image(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, Error>;

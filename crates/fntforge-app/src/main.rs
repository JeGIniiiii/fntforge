use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fntforge_core::{
    extract_chars, generate, write_fnt, Fill, Project, Rgba8, StrokePosition,
};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "fntforge", version, about = "Bitmap font generator for Cocos2d-x Lua")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Rasterize a font and write AngelCode .fnt + PNG
    Export {
        /// TrueType / OpenType file
        #[arg(long)]
        font: PathBuf,
        /// Output directory
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// File stem (writes {stem}.fnt and {stem}.png)
        #[arg(long, default_value = "font")]
        name: String,
        #[arg(long, default_value_t = 48)]
        size: u32,
        /// Characters to include. Default: printable ASCII
        #[arg(long)]
        chars: Option<String>,
        #[arg(long)]
        chars_file: Option<PathBuf>,
        #[arg(long)]
        fill: Option<String>,
        #[arg(long)]
        stroke: Option<u32>,
        #[arg(long)]
        stroke_color: Option<String>,
        #[arg(long)]
        shadow: bool,
        #[arg(long)]
        tabular: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Export {
            font,
            out,
            name,
            size,
            chars,
            chars_file,
            fill,
            stroke,
            stroke_color,
            shadow,
            tabular,
        }) => {
            run_export(
                font,
                out,
                name,
                size,
                chars,
                chars_file,
                fill,
                stroke,
                stroke_color,
                shadow,
                tabular,
            )?;
        }
        None => {
            #[cfg(feature = "gui")]
            {
                gui::run()?;
            }
            #[cfg(not(feature = "gui"))]
            {
                anyhow::bail!("GUI was not compiled. Use `fntforge export --help`.");
            }
        }
    }
    Ok(())
}

fn run_export(
    font: PathBuf,
    out: PathBuf,
    name: String,
    size: u32,
    chars: Option<String>,
    chars_file: Option<PathBuf>,
    fill: Option<String>,
    stroke: Option<u32>,
    stroke_color: Option<String>,
    shadow: bool,
    tabular: bool,
) -> Result<()> {
    let bytes = std::fs::read(&font).with_context(|| format!("read {}", font.display()))?;
    let face = font
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("font")
        .to_string();
    let mut project = Project::new(bytes, face);
    project.font_size = size as f32;
    project.tabular_nums = tabular;
    let mut set = String::new();
    if let Some(p) = chars_file {
        set.push_str(&std::fs::read_to_string(p)?);
    }
    if let Some(c) = chars {
        set.push_str(&c);
    }
    if set.is_empty() {
        set = fntforge_core::preset_chars(fntforge_core::CharsetPreset::Ascii);
    }
    project.chars = extract_chars(&set);
    if let Some(hex) = fill {
        project.style.fill = Fill::Solid(Rgba8::from_hex(&hex));
    }
    if let Some(px) = stroke {
        project.style.stroke.enabled = true;
        project.style.stroke.size = px as f32;
        project.style.stroke.position = StrokePosition::Outer;
        if let Some(hex) = stroke_color {
            project.style.stroke.color = Rgba8::from_hex(&hex);
        }
    }
    if shadow {
        project.style.drop_shadow.enabled = true;
    }
    let font = generate(&project).context("generate")?;
    fntforge_core::write_font_files(&font, &out, &name).context("write")?;
    let desc = write_fnt(&font, &name);
    println!(
        "Wrote {} glyphs, {} page(s), {} bytes of .fnt → {}",
        font.glyphs.len(),
        font.pages.len(),
        desc.text.len(),
        out.join(format!("{name}.fnt")).display()
    );
    Ok(())
}

#[cfg(feature = "gui")]
mod gui;


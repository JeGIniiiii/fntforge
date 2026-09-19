#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fntforge_core::{
    extract_chars, generate, write_fnt, Fill, Project, Rgba8, StrokePosition,
};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "fntforge",
    version,
    about = "面向 Cocos2d-x Lua 的位图字体生成器"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// 栅格化字体并写出 AngelCode .fnt + PNG
    Export {
        /// TrueType / OpenType 文件
        #[arg(long)]
        font: PathBuf,
        /// 输出目录
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// 文件名（写出 {stem}.fnt 和 {stem}.png）
        #[arg(long, default_value = "font")]
        name: String,
        #[arg(long, default_value_t = 48)]
        size: u32,
        /// 要包含的字符。默认：可打印 ASCII
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
                anyhow::bail!("未编译图形界面。请使用 `fntforge export --help`。");
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
    let bytes = std::fs::read(&font).with_context(|| format!("读取 {}", font.display()))?;
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
    let font = generate(&project).context("生成")?;
    fntforge_core::write_font_files(&font, &out, &name).context("写出")?;
    let desc = write_fnt(&font, &name);
    println!(
        "已写出 {} 个字形、{} 页、{} 字节 .fnt → {}",
        font.glyphs.len(),
        font.pages.len(),
        desc.text.len(),
        out.join(format!("{name}.fnt")).display()
    );
    Ok(())
}

#[cfg(feature = "gui")]
mod gui;

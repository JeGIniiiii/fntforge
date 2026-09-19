#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fntforge_core::{
    extract_chars, extract_from_source, generate, generate_scaled, write_fnt, write_font_files, Fill,
    Project, ProjectFile, Rgba8, StrokePosition,
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
        #[arg(long)]
        font: Option<PathBuf>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        out: PathBuf,
        #[arg(long, default_value = "font")]
        name: String,
        #[arg(long, default_value_t = 48)]
        size: u32,
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
        gold: bool,
        #[arg(long)]
        tabular: bool,
        /// 额外倍率，例如 2 会再写一份 @2x
        #[arg(long)]
        scale: Vec<f32>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Export {
            font,
            project,
            out,
            name,
            size,
            chars,
            chars_file,
            fill,
            stroke,
            stroke_color,
            shadow,
            gold,
            tabular,
            scale,
        }) => {
            run_export(
                font,
                project,
                out,
                name,
                size,
                chars,
                chars_file,
                fill,
                stroke,
                stroke_color,
                shadow,
                gold,
                tabular,
                scale,
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
    font: Option<PathBuf>,
    project_path: Option<PathBuf>,
    out: PathBuf,
    name: String,
    size: u32,
    chars: Option<String>,
    chars_file: Option<PathBuf>,
    fill: Option<String>,
    stroke: Option<u32>,
    stroke_color: Option<String>,
    shadow: bool,
    gold: bool,
    tabular: bool,
    scales: Vec<f32>,
) -> Result<()> {
    let mut project = if let Some(p) = &project_path {
        let pf = ProjectFile::load(p).with_context(|| format!("读取工程 {}", p.display()))?;
        let font_path = font
            .clone()
            .or_else(|| pf.font_path.as_ref().map(PathBuf::from))
            .ok_or_else(|| anyhow::anyhow!("工程未记录字体路径，请加 --font"))?;
        let bytes = std::fs::read(&font_path)?;
        let mut proj = Project::new(bytes, pf.font_name.clone());
        pf.apply_to(&mut proj);
        proj.font_path = Some(font_path);
        proj
    } else {
        let font = font.ok_or_else(|| anyhow::anyhow!("请提供 --font 或 --project"))?;
        let bytes = std::fs::read(&font).with_context(|| format!("读取 {}", font.display()))?;
        let face = font
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("font")
            .to_string();
        let mut project = Project::new(bytes, face);
        project.font_path = Some(font);
        project.font_size = size as f32;
        project.tabular_nums = tabular;
        project
    };

    let mut set = String::new();
    if let Some(p) = chars_file {
        set.push_str(&extract_from_source(&std::fs::read_to_string(p)?));
    }
    if let Some(c) = chars {
        set.push_str(&c);
    }
    if !set.is_empty() {
        project.chars = extract_chars(&set);
    }
    if gold {
        project.style = fntforge_core::StyleStack::preset_gold();
    }
    if let Some(hex) = fill {
        project.style.fill = Fill::solid(Rgba8::from_hex(&hex));
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
    write_font_files(&font, &out, &name).context("写出")?;
    let desc = write_fnt(&font, &name);
    println!(
        "已写出 {} 个字形、{} 页、{} 字节 .fnt → {}{}",
        font.glyphs.len(),
        font.pages.len(),
        desc.text.len(),
        out.join(format!("{name}.fnt")).display(),
        if font.missing.is_empty() {
            String::new()
        } else {
            format!("；缺字 {}", font.missing.len())
        }
    );

    let extra = if scales.is_empty() {
        project.extra_scales.clone()
    } else {
        scales
    };
    for sc in extra {
        if (sc - 1.0).abs() < 0.01 {
            continue;
        }
        let scaled = generate_scaled(&project, sc).context("生成倍率")?;
        let stem = format!("{name}@{sc:.0}x");
        write_font_files(&scaled, &out, &stem)?;
        println!("  额外 {} → {}", sc, out.join(format!("{stem}.fnt")).display());
    }
    Ok(())
}

#[cfg(feature = "gui")]
mod gui;

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, Pos2, Rect, RichText, TextureHandle,
    TextureOptions, Vec2,
};
use fntforge_core::{
    extract_chars, generate, write_fnt, write_font_files, AlignH, CharsetPreset, Fill, Project,
    Rgba8, StrokePosition, StyleStack,
};
use std::path::PathBuf;
use std::sync::Arc;

struct GlyphSpot {
    id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    xoffset: f32,
    yoffset: f32,
    xadvance: f32,
}

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title("FntForge"),
        ..Default::default()
    };
    eframe::run_native(
        "FntForge",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

struct App {
    project: Project,
    charset_edit: String,
    preview_edit: String,
    fill_hex: String,
    stroke_hex: String,
    shadow_hex: String,
    glow_hex: String,
    status: String,
    atlas_tex: Option<TextureHandle>,
    dirty: bool,
    last_fnt_preview: String,
    font_path: Option<PathBuf>,
    glyphs: Vec<GlyphSpot>,
    line_height: f32,
    atlas_size: Vec2,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        apply_fonts(&cc.egui_ctx);
        let bytes = bundled_font();
        let mut project = Project::new(bytes, "DejaVu Sans");
        project.style.stroke.enabled = true;
        project.style.stroke.size = 2.0;
        project.style.stroke.color = Rgba8::new(20, 24, 28, 255);
        project.style.fill = Fill::Solid(Rgba8::new(236, 239, 241, 255));
        project.style.drop_shadow.enabled = true;
        project.chars = fntforge_core::preset_chars(CharsetPreset::GameHud);
        let charset_edit = project.chars.clone();
        let preview_edit = project.preview_text.clone();
        let mut app = Self {
            project,
            charset_edit,
            preview_edit,
            fill_hex: "eceff1".into(),
            stroke_hex: "14181c".into(),
            shadow_hex: "000000a0".into(),
            glow_hex: "6a9aa3".into(),
            status: "打开一款含中文的 TTF，调整图层样式，导出给 Cocos2d-x Lua。".into(),
            atlas_tex: None,
            dirty: true,
            last_fnt_preview: String::new(),
            font_path: None,
            glyphs: Vec::new(),
            line_height: 48.0,
            atlas_size: Vec2::splat(1.0),
        };
        app.rebuild(&cc.egui_ctx);
        app
    }

    fn rebuild(&mut self, ctx: &egui::Context) {
        self.project.chars = extract_chars(&self.charset_edit);
        self.project.preview_text = self.preview_edit.clone();
        self.project.style.fill = Fill::Solid(Rgba8::from_hex(&self.fill_hex));
        self.project.style.stroke.color = Rgba8::from_hex(&self.stroke_hex);
        self.project.style.drop_shadow.color = Rgba8::from_hex(&self.shadow_hex);
        self.project.style.outer_glow.color = Rgba8::from_hex(&self.glow_hex);
        match generate(&self.project) {
            Ok(font) => {
                if let Some(page) = font.pages.first() {
                    let size = [page.width() as usize, page.height() as usize];
                    let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
                    for p in page.pixels() {
                        rgba.extend_from_slice(&p.0);
                    }
                    let img = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                    self.atlas_tex =
                        Some(ctx.load_texture("atlas", img, TextureOptions::NEAREST));
                    self.atlas_size = Vec2::new(page.width() as f32, page.height() as f32);
                }
                self.glyphs = font
                    .glyphs
                    .iter()
                    .map(|g| GlyphSpot {
                        id: g.id,
                        x: g.x as f32,
                        y: g.y as f32,
                        w: g.width as f32,
                        h: g.height as f32,
                        xoffset: g.xoffset as f32,
                        yoffset: g.yoffset as f32,
                        xadvance: g.xadvance as f32,
                    })
                    .collect();
                self.line_height = font.line_height as f32;
                self.last_fnt_preview = write_fnt(&font, "preview").text;
                self.status = format!(
                    "{} 个字形 · {}×{} · 行高 {} · 基线 {}",
                    font.glyphs.len(),
                    font.scale_w,
                    font.scale_h,
                    font.line_height,
                    font.base
                );
                self.dirty = false;
            }
            Err(e) => {
                self.status = format!("生成失败：{e}");
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("FntForge").strong().size(18.0));
                ui.label(
                    RichText::new("  面向 Cocos2d-x Lua 的位图字体")
                        .color(Color32::from_rgb(139, 145, 154)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("导出 .fnt").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("font.fnt")
                            .add_filter("位图字体", &["fnt"])
                            .save_file()
                        {
                            match generate(&self.project) {
                                Ok(font) => {
                                    let stem = path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("font");
                                    let dir = path.parent().unwrap_or(std::path::Path::new("."));
                                    match write_font_files(&font, dir, stem) {
                                        Ok(_) => {
                                            self.status = format!("已导出 {}", path.display())
                                        }
                                        Err(e) => self.status = format!("写出失败：{e}"),
                                    }
                                }
                                Err(e) => self.status = format!("生成失败：{e}"),
                            }
                        }
                    }
                    if ui.button("打开字体").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("字体文件", &["ttf", "otf", "ttc"])
                            .pick_file()
                        {
                            match std::fs::read(&path) {
                                Ok(bytes) => {
                                    let name = path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("font")
                                        .to_string();
                                    self.project.font_bytes = bytes;
                                    self.project.font_name = name;
                                    self.font_path = Some(path);
                                    self.dirty = true;
                                }
                                Err(e) => self.status = format!("读取失败：{e}"),
                            }
                        }
                    }
                });
            });
            ui.add_space(6.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                if let Some(p) = &self.font_path {
                    ui.separator();
                    ui.label(p.display().to_string());
                }
            });
        });

        egui::SidePanel::left("font")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("字体").strong());
                ui.add(egui::Slider::new(&mut self.project.font_size, 12.0..=128.0).text("字号"));
                ui.checkbox(&mut self.project.tabular_nums, "等宽数字");
                ui.add(
                    egui::Slider::new(&mut self.project.extra_letter_spacing, -8..=16)
                        .text("字距"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.extra_line_height, -16..=48)
                        .text("行距补偿"),
                );
                ui.separator();
                ui.label(RichText::new("字符集").strong());
                ui.horizontal_wrapped(|ui| {
                    if ui.button("ASCII").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::Ascii);
                        self.dirty = true;
                    }
                    if ui.button("数字").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::Numbers);
                        self.dirty = true;
                    }
                    if ui.button("HUD 中文").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::GameHud);
                        self.dirty = true;
                    }
                    if ui.button("标点").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::CommonPunct);
                        self.dirty = true;
                    }
                });
                ui.label("字符");
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut self.charset_edit)
                            .desired_rows(6)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                ui.separator();
                ui.label(RichText::new("图集").strong());
                ui.add(
                    egui::Slider::new(&mut self.project.pack.max_size, 256..=4096)
                        .text("最大边长"),
                );
                ui.checkbox(&mut self.project.pack.power_of_two, "二次幂");
                ui.checkbox(&mut self.project.pack.square, "正方形");
            });

        egui::SidePanel::right("style")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("图层样式").strong());
                ui.horizontal(|ui| {
                    ui.label("填充");
                    if ui.text_edit_singleline(&mut self.fill_hex).changed() {
                        self.dirty = true;
                    }
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.stroke.enabled, "描边")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.style.stroke.size, 0.5..=12.0).text("大小"),
                );
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.project.style.stroke.position,
                        StrokePosition::Outer,
                        "外",
                    );
                    ui.selectable_value(
                        &mut self.project.style.stroke.position,
                        StrokePosition::Center,
                        "中",
                    );
                    ui.selectable_value(
                        &mut self.project.style.stroke.position,
                        StrokePosition::Inner,
                        "内",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("颜色");
                    ui.text_edit_singleline(&mut self.stroke_hex);
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.drop_shadow.enabled, "投影")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.distance, 0.0..=16.0)
                        .text("距离"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.size, 0.0..=16.0)
                        .text("模糊"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.angle_deg, 0.0..=360.0)
                        .text("角度"),
                );
                ui.horizontal(|ui| {
                    ui.label("颜色");
                    ui.text_edit_singleline(&mut self.shadow_hex);
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.outer_glow.enabled, "外发光")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.style.outer_glow.size, 0.5..=24.0)
                        .text("大小"),
                );
                ui.horizontal(|ui| {
                    ui.label("颜色");
                    ui.text_edit_singleline(&mut self.glow_hex);
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.inner_shadow.enabled, "内阴影")
                    .changed()
                {
                    self.dirty = true;
                }
                if ui
                    .checkbox(&mut self.project.style.inner_glow.enabled, "内发光")
                    .changed()
                {
                    self.dirty = true;
                }
                if ui
                    .checkbox(&mut self.project.style.color_overlay.enabled, "颜色叠加")
                    .changed()
                {
                    self.dirty = true;
                }
                if ui.button("应用样式").clicked() {
                    self.dirty = true;
                }
                if ui.button("重置样式").clicked() {
                    self.project.style = StyleStack::default();
                    self.dirty = true;
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("预览");
                ui.selectable_value(&mut self.project.align_h, AlignH::Left, "左对齐");
                ui.selectable_value(&mut self.project.align_h, AlignH::Center, "居中");
                ui.selectable_value(&mut self.project.align_h, AlignH::Right, "右对齐");
            });
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.preview_edit)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                self.project.preview_text = self.preview_edit.clone();
            }
            self.paint_preview(ui);
            ui.separator();
            ui.label("图集");
            if let Some(tex) = &self.atlas_tex {
                let avail = ui.available_size();
                let size = tex.size_vec2();
                let scale = (avail.x / size.x).min(avail.y * 0.45 / size.y).min(4.0);
                ui.image((tex.id(), size * scale.max(0.1)));
            }
            ui.separator();
            ui.collapsing(".fnt 文本", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.last_fnt_preview)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                );
            });
        });

        if self.dirty {
            self.rebuild(ctx);
        }
    }
}

impl App {
    fn paint_preview(&self, ui: &mut egui::Ui) {
        let Some(tex) = &self.atlas_tex else { return };
        let lines: Vec<&str> = self.preview_edit.split('\n').collect();
        let pad = 16.0;
        let widths: Vec<f32> = lines
            .iter()
            .map(|line| {
                line.chars()
                    .map(|ch| {
                        self.glyphs
                            .iter()
                            .find(|g| g.id == ch as u32)
                            .map(|g| g.xadvance)
                            .unwrap_or(self.project.font_size * 0.5)
                    })
                    .sum()
            })
            .collect();
        let w = widths
            .iter()
            .copied()
            .fold(320.0f32, f32::max)
            .max(ui.available_width())
            + pad * 2.0;
        let h = (lines.len() as f32 * self.line_height + pad * 2.0).max(72.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w.min(ui.available_width()), h), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 6.0, Color32::from_rgb(20, 24, 31));
        let tw = self.atlas_size.x.max(1.0);
        let th = self.atlas_size.y.max(1.0);
        for (li, line) in lines.iter().enumerate() {
            let line_w = widths[li];
            let mut x = match self.project.align_h {
                AlignH::Left => rect.left() + pad,
                AlignH::Right => rect.right() - pad - line_w,
                AlignH::Center => rect.left() + (rect.width() - line_w) * 0.5,
            };
            let y = rect.top() + pad + li as f32 * self.line_height;
            for ch in line.chars() {
                if let Some(g) = self.glyphs.iter().find(|g| g.id == ch as u32) {
                    let uv = Rect::from_min_max(
                        Pos2::new(g.x / tw, g.y / th),
                        Pos2::new((g.x + g.w) / tw, (g.y + g.h) / th),
                    );
                    let dest = Rect::from_min_size(
                        Pos2::new(x + g.xoffset, y + g.yoffset),
                        Vec2::new(g.w.max(1.0), g.h.max(1.0)),
                    );
                    ui.painter().image(tex.id(), dest, uv, Color32::WHITE);
                    x += g.xadvance;
                } else {
                    x += self.project.font_size * 0.5;
                }
            }
        }
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(12, 14, 18);
    visuals.window_fill = Color32::from_rgb(20, 24, 31);
    visuals.extreme_bg_color = Color32::from_rgb(20, 24, 31);
    visuals.override_text_color = Some(Color32::from_rgb(232, 234, 237));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(32, 38, 46);
    visuals.selection.bg_fill = Color32::from_rgb(106, 154, 163);
    ctx.set_visuals(visuals);
}

fn apply_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".to_owned(),
        Arc::new(FontData::from_static(include_bytes!("../assets/ui_cjk.ttf"))),
    );
    if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
        prop.insert(0, "cjk".to_owned());
    }
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.push("cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn bundled_font() -> Vec<u8> {
    let candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "testdata/DejaVuSans.ttf",
    ];
    for p in candidates {
        if let Ok(b) = std::fs::read(p) {
            return b;
        }
    }
    include_bytes!("../../../testdata/DejaVuSans.ttf").to_vec()
}

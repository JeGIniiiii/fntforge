use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, Pos2, Rect, RichText, TextureHandle,
    TextureOptions, Vec2,
};
use fntforge_core::{
    extract_chars, extract_from_source, generate, write_fnt, write_font_files, AlignH, CharsetPreset,
    Fill, Project, ProjectFile, Rgba8, StrokePosition, StyleStack,
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
            .with_inner_size([1360.0, 840.0])
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
    grad_a: String,
    grad_b: String,
    status: String,
    atlas_tex: Option<TextureHandle>,
    dirty: bool,
    last_fnt_preview: String,
    font_path: Option<PathBuf>,
    glyphs: Vec<GlyphSpot>,
    line_height: f32,
    atlas_size: Vec2,
    history: Vec<StyleStack>,
    missing: String,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        apply_fonts(&cc.egui_ctx);
        let bytes = bundled_font();
        let mut project = Project::new(bytes, "DejaVu Sans");
        project.style = StyleStack::preset_gold();
        let charset_edit = project.chars.clone();
        let preview_edit = project.preview_text.clone();
        let mut app = Self {
            project,
            charset_edit,
            preview_edit,
            fill_hex: "e8c44a".into(),
            stroke_hex: "2a1808".into(),
            shadow_hex: "000000a0".into(),
            glow_hex: "e6d060aa".into(),
            grad_a: "fff3a0".into(),
            grad_b: "b8841c".into(),
            status: "打开含中文的 TTF。标题字缺「+」时会自动用后备字体，避免画成圆环。".into(),
            atlas_tex: None,
            dirty: true,
            last_fnt_preview: String::new(),
            font_path: None,
            glyphs: Vec::new(),
            line_height: 48.0,
            atlas_size: Vec2::splat(1.0),
            history: Vec::new(),
            missing: String::new(),
        };
        app.rebuild(&cc.egui_ctx);
        app
    }

    fn push_history(&mut self) {
        self.history.push(self.project.style.clone());
        if self.history.len() > 32 {
            self.history.remove(0);
        }
    }

    fn rebuild(&mut self, ctx: &egui::Context) {
        self.project.chars = extract_chars(&self.charset_edit);
        self.project.preview_text = self.preview_edit.clone();
        if self.project.style.gradient_overlay.enabled {
            self.project.style.fill = Fill::Linear {
                angle_deg: self.project.style.gradient_overlay.angle_deg,
                stops: vec![
                    (0.0, Rgba8::from_hex(&self.grad_a)),
                    (1.0, Rgba8::from_hex(&self.grad_b)),
                ],
            };
        } else {
            self.project.style.fill = Fill::solid(Rgba8::from_hex(&self.fill_hex));
        }
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
                self.missing = fntforge_core::missing_report(&font);
                self.status = format!(
                    "{} 个字形 · {}×{} · {} 页 · 行高 {} · 缺字 {} · 后备 {}",
                    font.glyphs.len(),
                    font.scale_w,
                    font.scale_h,
                    font.pages.len(),
                    font.line_height,
                    font.missing.len(),
                    font.fallback_used.len()
                );
                self.dirty = false;
            }
            Err(e) => self.status = format!("生成失败：{e}"),
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
                        self.export();
                    }
                    if ui.button("保存工程").clicked() {
                        self.save_project();
                    }
                    if ui.button("打开工程").clicked() {
                        self.open_project();
                    }
                    if ui.button("导入 ASL").clicked() {
                        self.import_asl();
                    }
                    if ui.button("打开字体").clicked() {
                        self.open_font();
                    }
                    if ui.button("撤销").clicked() {
                        if let Some(s) = self.history.pop() {
                            self.project.style = s;
                            self.dirty = true;
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
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("字体").strong());
                ui.add(egui::Slider::new(&mut self.project.font_size, 12.0..=128.0).text("字号"));
                ui.checkbox(&mut self.project.tabular_nums, "等宽数字");
                if ui
                    .checkbox(&mut self.project.ascii_fallback, "缺字时用后备字体")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.extra_letter_spacing, -8..=16).text("字距"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.extra_line_height, -16..=48).text("行距补偿"),
                );
                ui.separator();
                ui.label(RichText::new("字符集").strong());
                ui.horizontal_wrapped(|ui| {
                    for (name, p) in [
                        ("ASCII", CharsetPreset::Ascii),
                        ("数字", CharsetPreset::Numbers),
                        ("HUD", CharsetPreset::GameHud),
                        ("标点", CharsetPreset::CommonPunct),
                        ("常用字", CharsetPreset::CommonZh),
                    ] {
                        if ui.button(name).clicked() {
                            self.charset_edit = fntforge_core::preset_chars(p);
                            self.dirty = true;
                        }
                    }
                });
                if ui.button("从文案文件抽取").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("文本", &["txt", "lua", "csv", "json", "xml"])
                        .pick_file()
                    {
                        if let Ok(t) = std::fs::read_to_string(&path) {
                            self.charset_edit = extract_from_source(&t);
                            self.dirty = true;
                        }
                    }
                }
                ui.label("字符");
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut self.charset_edit)
                            .desired_rows(5)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                ui.separator();
                ui.label(RichText::new("图集").strong());
                ui.add(
                    egui::Slider::new(&mut self.project.pack.max_size, 256..=4096).text("最大边长"),
                );
                ui.checkbox(&mut self.project.pack.power_of_two, "二次幂");
                ui.checkbox(&mut self.project.pack.square, "正方形");
                if !self.missing.is_empty() {
                    ui.separator();
                    ui.collapsing("缺字 / 后备", |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.missing)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        );
                    });
                }
            });

        egui::SidePanel::right("style")
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("图层样式").strong());
                ui.horizontal_wrapped(|ui| {
                    for (name, make) in [
                        ("金币", StyleStack::preset_gold as fn() -> StyleStack),
                        ("血量", StyleStack::preset_hp_red),
                        ("白描边", StyleStack::preset_white_outline),
                        ("暴击", StyleStack::preset_crit),
                        ("霓虹", StyleStack::preset_neon),
                        ("禁用", StyleStack::preset_disabled),
                        ("石刻", StyleStack::preset_stone),
                        ("像素", StyleStack::preset_pixel),
                    ] {
                        if ui.button(name).clicked() {
                            self.push_history();
                            self.project.style = make();
                            self.dirty = true;
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("填充");
                    if ui.text_edit_singleline(&mut self.fill_hex).changed() {
                        self.dirty = true;
                    }
                });
                if ui
                    .checkbox(&mut self.project.style.gradient_overlay.enabled, "渐变叠加")
                    .changed()
                {
                    self.dirty = true;
                }
                if self.project.style.gradient_overlay.enabled {
                    ui.horizontal(|ui| {
                        ui.label("A");
                        ui.text_edit_singleline(&mut self.grad_a);
                        ui.label("B");
                        ui.text_edit_singleline(&mut self.grad_b);
                    });
                    ui.add(
                        egui::Slider::new(&mut self.project.style.gradient_overlay.angle_deg, 0.0..=360.0)
                            .text("角度"),
                    );
                }
                ui.separator();
                effect_toggle(ui, &mut self.project.style.stroke.enabled, "描边", &mut self.dirty);
                ui.add(egui::Slider::new(&mut self.project.style.stroke.size, 0.5..=12.0).text("大小"));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.project.style.stroke.position, StrokePosition::Outer, "外");
                    ui.selectable_value(&mut self.project.style.stroke.position, StrokePosition::Center, "中");
                    ui.selectable_value(&mut self.project.style.stroke.position, StrokePosition::Inner, "内");
                });
                ui.horizontal(|ui| {
                    ui.label("颜色");
                    ui.text_edit_singleline(&mut self.stroke_hex);
                });
                ui.separator();
                effect_toggle(ui, &mut self.project.style.drop_shadow.enabled, "投影", &mut self.dirty);
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.distance, 0.0..=16.0).text("距离"),
                );
                ui.add(egui::Slider::new(&mut self.project.style.drop_shadow.size, 0.0..=16.0).text("模糊"));
                ui.add(
                    egui::Slider::new(&mut self.project.style.global_light.angle_deg, 0.0..=360.0)
                        .text("全局光角度"),
                );
                ui.horizontal(|ui| {
                    ui.label("颜色");
                    ui.text_edit_singleline(&mut self.shadow_hex);
                });
                ui.separator();
                effect_toggle(ui, &mut self.project.style.outer_glow.enabled, "外发光", &mut self.dirty);
                ui.add(egui::Slider::new(&mut self.project.style.outer_glow.size, 0.5..=24.0).text("大小"));
                ui.horizontal(|ui| {
                    ui.label("颜色");
                    ui.text_edit_singleline(&mut self.glow_hex);
                });
                effect_toggle(ui, &mut self.project.style.inner_shadow.enabled, "内阴影", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.inner_glow.enabled, "内发光", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.color_overlay.enabled, "颜色叠加", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.bevel.enabled, "斜面浮雕", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.satin.enabled, "光泽", &mut self.dirty);
                if ui.button("应用样式").clicked() {
                    self.dirty = true;
                }
                if ui.button("重置样式").clicked() {
                    self.push_history();
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
                        .desired_rows(8)
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
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(w.min(ui.available_width()), h), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 6.0, Color32::from_rgb(12, 16, 22));
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

    fn open_font(&mut self) {
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
                    self.project.font_path = Some(path.clone());
                    self.font_path = Some(path);
                    self.dirty = true;
                }
                Err(e) => self.status = format!("读取失败：{e}"),
            }
        }
    }

    fn export(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("font.fnt")
            .add_filter("位图字体", &["fnt"])
            .save_file()
        {
            match generate(&self.project) {
                Ok(font) => {
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("font");
                    let dir = path.parent().unwrap_or(std::path::Path::new("."));
                    match write_font_files(&font, dir, stem) {
                        Ok(_) => {
                            for sc in &self.project.extra_scales {
                                if (*sc - 1.0).abs() < 0.01 {
                                    continue;
                                }
                                if let Ok(scaled) = fntforge_core::generate_scaled(&self.project, *sc)
                                {
                                    let _ = write_font_files(
                                        &scaled,
                                        dir,
                                        &format!("{stem}@{:.0}x", sc),
                                    );
                                }
                            }
                            self.status = format!("已导出 {}", path.display());
                        }
                        Err(e) => self.status = format!("写出失败：{e}"),
                    }
                }
                Err(e) => self.status = format!("生成失败：{e}"),
            }
        }
    }

    fn save_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("font.fntproj")
            .add_filter("FntForge 工程", &["fntproj", "json"])
            .save_file()
        {
            let pf = ProjectFile::from_project(&self.project);
            match pf.save(&path) {
                Ok(_) => self.status = format!("已保存工程 {}", path.display()),
                Err(e) => self.status = format!("保存失败：{e}"),
            }
        }
    }

    fn open_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("FntForge 工程", &["fntproj", "json"])
            .pick_file()
        {
            match ProjectFile::load(&path) {
                Ok(pf) => {
                    if let Some(fp) = pf.font_path.as_ref() {
                        if let Ok(bytes) = std::fs::read(fp) {
                            self.project.font_bytes = bytes;
                            self.font_path = Some(PathBuf::from(fp));
                        }
                    }
                    pf.apply_to(&mut self.project);
                    self.charset_edit = self.project.chars.clone();
                    self.preview_edit = self.project.preview_text.clone();
                    self.dirty = true;
                    self.status = format!("已打开工程 {}", path.display());
                }
                Err(e) => self.status = format!("打开工程失败：{e}"),
            }
        }
    }

    fn import_asl(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Photoshop 样式", &["asl", "psd"])
            .pick_file()
        {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    self.push_history();
                    if path.extension().and_then(|s| s.to_str()) == Some("psd") {
                        match fntforge_import::list_psd_layers(&bytes) {
                            Ok(layers) => {
                                self.project.style = fntforge_import::style_from_descriptor_bytes(&bytes);
                                self.status = format!(
                                    "已读 PSD，{} 个图层：{}",
                                    layers.len(),
                                    layers
                                        .iter()
                                        .take(6)
                                        .map(|l| l.name.as_str())
                                        .collect::<Vec<_>>()
                                        .join(" / ")
                                );
                                self.dirty = true;
                            }
                            Err(e) => self.status = format!("PSD：{e}"),
                        }
                    } else {
                        match fntforge_import::import_asl(&bytes) {
                            Ok(style) => {
                                self.project.style = style;
                                self.dirty = true;
                                self.status = format!("已导入 ASL {}", path.display());
                            }
                            Err(e) => self.status = format!("ASL：{e}"),
                        }
                    }
                }
                Err(e) => self.status = format!("读取失败：{e}"),
            }
        }
    }
}

fn effect_toggle(ui: &mut egui::Ui, on: &mut bool, label: &str, dirty: &mut bool) {
    if ui.checkbox(on, label).changed() {
        *dirty = true;
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
    include_bytes!("../../../testdata/DejaVuSans.ttf").to_vec()
}

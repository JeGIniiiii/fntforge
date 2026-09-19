use eframe::egui::{
    self, color_picker::Alpha, Color32, FontData, FontDefinitions, FontFamily, Pos2, Rect, RichText,
    TextureHandle, TextureOptions, Vec2,
};
use fntforge_core::{
    extract_chars, extract_from_source, generate, merge_chars, parse_fnt, write_fnt, write_font_files,
    AlignH, CharsetPreset, Fill, Project, ProjectFile, Rgba8, StrokePosition, StyleStack,
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
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_title("FntForge")
            .with_decorations(true),
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
    append_edit: String,
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
            status: "打开标题 TTF。字符集可导入已有 .fnt 再追加。".into(),
            atlas_tex: None,
            dirty: true,
            last_fnt_preview: String::new(),
            font_path: None,
            glyphs: Vec::new(),
            line_height: 48.0,
            atlas_size: Vec2::splat(1.0),
            history: Vec::new(),
            missing: String::new(),
            append_edit: String::new(),
        };
        app.sync_hex();
        app.rebuild(&cc.egui_ctx);
        app
    }

    fn push_history(&mut self) {
        self.history.push(self.project.style.clone());
        if self.history.len() > 32 {
            self.history.remove(0);
        }
    }

    fn sync_hex(&mut self) {
        self.stroke_hex = self.project.style.stroke.color.to_hex(true);
        self.shadow_hex = self.project.style.drop_shadow.color.to_hex(true);
        self.glow_hex = self.project.style.outer_glow.color.to_hex(true);
        if let Some((_, c)) = self.project.style.gradient_overlay.stops.first() {
            self.grad_a = c.to_hex(false);
        }
        if let Some((_, c)) = self.project.style.gradient_overlay.stops.last() {
            self.grad_b = c.to_hex(false);
        }
        match &self.project.style.fill {
            Fill::Solid { color } => self.fill_hex = color.to_hex(false),
            Fill::Linear { stops, .. } => {
                if let Some((_, c)) = stops.first() {
                    self.fill_hex = c.to_hex(false);
                }
            }
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
        apply_theme(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(
            egui::viewport::SystemTheme::Dark,
        ));

        egui::TopBottomPanel::top("bar")
            .exact_height(52.0)
            .frame(
                egui::Frame::new()
                    .fill(C::BG)
                    .stroke(egui::Stroke::new(1.0_f32, C::LINE))
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new("FntForge").size(16.0).color(C::FG).strong());
                    ui.add_space(10.0);
                    if ui.add(tool_btn("打开字体")).clicked() {
                        self.open_font();
                    }
                    if ui.add(tool_btn("打开工程")).clicked() {
                        self.open_project();
                    }
                    if ui.add(tool_btn("保存工程")).clicked() {
                        self.save_project();
                    }
                    ui.label(RichText::new("·").color(C::SUBTLE));
                    if ui.add(tool_btn("导入 .fnt")).clicked() {
                        self.import_fnt();
                    }
                    if ui.add(tool_btn("导入配置")).clicked() {
                        self.import_style();
                    }
                    if ui.add(tool_btn("导入 ASL")).clicked() {
                        self.import_asl();
                    }
                    ui.label(RichText::new("·").color(C::SUBTLE));
                    if ui.add(tool_btn("导出配置")).clicked() {
                        self.export_style();
                    }
                    if ui.add(tool_btn("撤销")).clicked() {
                        if let Some(s) = self.history.pop() {
                            self.project.style = s;
                            self.sync_hex();
                            self.dirty = true;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let export = egui::Button::new(
                            RichText::new("  导出 .fnt  ").color(C::ACCENT_FG).strong(),
                        )
                        .fill(C::ACCENT)
                        .min_size(Vec2::new(108.0, 32.0));
                        if ui.add(export).clicked() {
                            self.export();
                        }
                    });
                });
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(32.0)
            .frame(
                egui::Frame::new()
                    .fill(C::BG)
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(12.0).color(C::MUTED));
                    if let Some(p) = &self.font_path {
                        ui.separator();
                        ui.label(
                            RichText::new(p.display().to_string())
                                .size(12.0)
                                .color(C::SUBTLE),
                        );
                    }
                });
            });

        egui::SidePanel::left("font")
            .default_width(280.0)
            .width_range(240.0..=360.0)
            .frame(
                egui::Frame::new()
                    .fill(C::PANEL)
                    .inner_margin(egui::Margin::symmetric(14, 12)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                section_label(ui, "字体");
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
                ui.add_space(10.0);
                section_label(ui, "字符集");
                ui.horizontal_wrapped(|ui| {
                    for (name, p) in [
                        ("ASCII", CharsetPreset::Ascii),
                        ("数字", CharsetPreset::Numbers),
                        ("HUD", CharsetPreset::GameHud),
                        ("标点", CharsetPreset::CommonPunct),
                        ("常用字", CharsetPreset::CommonZh),
                    ] {
                        if ui.button(name).clicked() {
                            self.charset_edit =
                                merge_chars(&self.charset_edit, &fntforge_core::preset_chars(p));
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
                            self.charset_edit = merge_chars(&self.charset_edit, &extract_from_source(&t));
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
                ui.label("追加（不覆盖已有字）");
                ui.add(
                    egui::TextEdit::multiline(&mut self.append_edit)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text("粘贴要加的字，例如：关卡BOSS"),
                );
                if ui.button("合并进字符集").clicked() {
                    let n0 = extract_chars(&self.charset_edit).chars().count();
                    self.charset_edit = merge_chars(&self.charset_edit, &self.append_edit);
                    self.append_edit.clear();
                    let n1 = extract_chars(&self.charset_edit).chars().count();
                    self.status = format!("已追加 {} 字，当前 {} 字", n1.saturating_sub(n0), n1);
                    self.dirty = true;
                }
                ui.add_space(10.0);
                section_label(ui, "图集");
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
            });

        egui::SidePanel::right("style")
            .default_width(300.0)
            .width_range(260.0..=380.0)
            .frame(
                egui::Frame::new()
                    .fill(C::PANEL)
                    .inner_margin(egui::Margin::symmetric(14, 12)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                section_label(ui, "图层样式");
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
                        if ui.add(tool_btn(name)).clicked() {
                            self.push_history();
                            self.project.style = make();
                            self.sync_hex();
                            self.dirty = true;
                        }
                    }
                });
                ui.add_space(8.0);
                if color_row(ui, "填充", &mut self.fill_hex, false) {
                    self.dirty = true;
                }
                if ui
                    .checkbox(&mut self.project.style.gradient_overlay.enabled, "渐变叠加")
                    .changed()
                {
                    self.dirty = true;
                }
                if self.project.style.gradient_overlay.enabled {
                    if color_row(ui, "渐变 A", &mut self.grad_a, false) {
                        self.dirty = true;
                    }
                    if color_row(ui, "渐变 B", &mut self.grad_b, false) {
                        self.dirty = true;
                    }
                    ui.add(
                        egui::Slider::new(&mut self.project.style.gradient_overlay.angle_deg, 0.0..=360.0)
                            .text("角度"),
                    );
                }
                ui.add_space(6.0);
                effect_toggle(ui, &mut self.project.style.stroke.enabled, "描边", &mut self.dirty);
                if self.project.style.stroke.enabled {
                    ui.add(egui::Slider::new(&mut self.project.style.stroke.size, 0.5..=12.0).text("大小"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.project.style.stroke.position, StrokePosition::Outer, "外");
                        ui.selectable_value(&mut self.project.style.stroke.position, StrokePosition::Center, "中");
                        ui.selectable_value(&mut self.project.style.stroke.position, StrokePosition::Inner, "内");
                    });
                    if color_row(ui, "描边色", &mut self.stroke_hex, true) {
                        self.dirty = true;
                    }
                }
                ui.add_space(6.0);
                effect_toggle(ui, &mut self.project.style.drop_shadow.enabled, "投影", &mut self.dirty);
                if self.project.style.drop_shadow.enabled {
                    ui.add(
                        egui::Slider::new(&mut self.project.style.drop_shadow.distance, 0.0..=16.0).text("距离"),
                    );
                    ui.add(egui::Slider::new(&mut self.project.style.drop_shadow.size, 0.0..=16.0).text("模糊"));
                    ui.add(
                        egui::Slider::new(&mut self.project.style.global_light.angle_deg, 0.0..=360.0)
                            .text("光照角度"),
                    );
                    if color_row(ui, "投影色", &mut self.shadow_hex, true) {
                        self.dirty = true;
                    }
                }
                ui.add_space(6.0);
                effect_toggle(ui, &mut self.project.style.outer_glow.enabled, "外发光", &mut self.dirty);
                if self.project.style.outer_glow.enabled {
                    ui.add(egui::Slider::new(&mut self.project.style.outer_glow.size, 0.5..=24.0).text("大小"));
                    if color_row(ui, "发光色", &mut self.glow_hex, true) {
                        self.dirty = true;
                    }
                }
                effect_toggle(ui, &mut self.project.style.inner_shadow.enabled, "内阴影", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.inner_glow.enabled, "内发光", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.color_overlay.enabled, "颜色叠加", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.bevel.enabled, "斜面浮雕", &mut self.dirty);
                effect_toggle(ui, &mut self.project.style.satin.enabled, "光泽", &mut self.dirty);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(tool_btn("应用样式")).clicked() {
                        self.dirty = true;
                    }
                    if ui.add(tool_btn("重置样式")).clicked() {
                        self.push_history();
                        self.project.style = StyleStack::default();
                        self.sync_hex();
                        self.dirty = true;
                    }
                });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(C::BG)
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                section_label(ui, "预览");
                ui.add_space(12.0);
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
            ui.add_space(14.0);
            section_label(ui, "图集");
            if let Some(tex) = &self.atlas_tex {
                let avail = ui.available_size();
                let size = tex.size_vec2();
                let scale = (avail.x / size.x).min((avail.y - 80.0).max(80.0) / size.y).min(4.0);
                ui.add(
                    egui::Image::new((tex.id(), size * scale.max(0.1)))
                        .bg_fill(C::PANEL),
                );
            }
            ui.add_space(8.0);
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
            .rect_filled(rect, 0.0, Color32::BLACK);
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0_f32, C::LINE),
            egui::StrokeKind::Inside,
        );
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

    fn import_fnt(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("BMFont", &["fnt"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => match parse_fnt(&text) {
                    Ok(imp) => {
                        let n0 = extract_chars(&self.charset_edit).chars().count();
                        self.charset_edit = merge_chars(&self.charset_edit, &imp.chars);
                        if imp.size > 0 {
                            self.project.font_size = imp.size as f32;
                        }
                        if !imp.face.is_empty() {
                            self.project.font_name = imp.face;
                        }
                        let n1 = extract_chars(&self.charset_edit).chars().count();
                        self.status = format!(
                            "已导入 {}：{} 字，新增 {}。用「追加」继续加字。",
                            path.display(),
                            extract_chars(&imp.chars).chars().count(),
                            n1.saturating_sub(n0)
                        );
                        self.dirty = true;
                    }
                    Err(e) => self.status = format!("解析 .fnt 失败：{e}"),
                },
                Err(e) => self.status = format!("读取失败：{e}"),
            }
        }
    }

    fn export_style(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("font.style.json")
            .add_filter("样式 JSON", &["json"])
            .save_file()
        {
            match ProjectFile::save_style(&self.project.style, &path) {
                Ok(_) => {
                    if let Some(proj) = path
                        .parent()
                        .map(|d| d.join(format!(
                            "{}.fntproj",
                            path.file_stem().and_then(|s| s.to_str()).unwrap_or("font")
                        )))
                    {
                        let _ = ProjectFile::from_project(&self.project).save(&proj);
                    }
                    self.status = format!("已导出配置 {}", path.display());
                }
                Err(e) => self.status = format!("导出配置失败：{e}"),
            }
        }
    }

    fn import_style(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("配置", &["json", "fntproj"])
            .pick_file()
        {
            if let Ok(pf) = ProjectFile::load(&path) {
                self.push_history();
                pf.apply_to(&mut self.project);
                self.charset_edit = merge_chars(&self.charset_edit, &self.project.chars);
                self.preview_edit = self.project.preview_text.clone();
                self.sync_hex();
                self.dirty = true;
                self.status = format!("已导入工程配置 {}", path.display());
                return;
            }
            match ProjectFile::load_style(&path) {
                Ok(style) => {
                    self.push_history();
                    self.project.style = style;
                    self.sync_hex();
                    self.dirty = true;
                    self.status = format!("已套用样式 {}", path.display());
                }
                Err(e) => self.status = format!("导入配置失败：{e}"),
            }
        }
    }
}

fn effect_toggle(ui: &mut egui::Ui, on: &mut bool, label: &str, dirty: &mut bool) {
    if ui.checkbox(on, label).changed() {
        *dirty = true;
    }
}

struct C;
impl C {
    const BG: Color32 = Color32::from_rgb(0, 0, 0);
    const PANEL: Color32 = Color32::from_rgb(8, 8, 9);
    const RAISED: Color32 = Color32::from_rgb(18, 18, 20);
    const FG: Color32 = Color32::from_rgb(240, 240, 242);
    const MUTED: Color32 = Color32::from_rgb(140, 140, 146);
    const SUBTLE: Color32 = Color32::from_rgb(88, 88, 94);
    const LINE: Color32 = Color32::from_rgb(32, 32, 36);
    const ACCENT: Color32 = Color32::from_rgb(232, 232, 236);
    const ACCENT_FG: Color32 = Color32::from_rgb(0, 0, 0);
}

fn tool_btn(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_owned()).size(13.0).color(C::FG))
        .fill(C::RAISED)
        .stroke(egui::Stroke::new(1.0_f32, C::LINE))
        .min_size(Vec2::new(0.0, 30.0))
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.painter().rect_filled(
            Rect::from_min_size(ui.cursor().min, Vec2::new(2.0, 12.0)),
            0.0,
            C::ACCENT,
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(text)
                .size(11.0)
                .color(C::MUTED)
                .strong(),
        );
    });
    ui.add_space(8.0);
}

fn color_row(ui: &mut egui::Ui, label: &str, hex: &mut String, with_alpha: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(C::MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let c = Rgba8::from_hex(hex);
            let mut col = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
            let alpha = if with_alpha {
                Alpha::OnlyBlend
            } else {
                Alpha::Opaque
            };
            if egui::color_picker::color_edit_button_srgba(ui, &mut col, alpha).changed() {
                let n = Rgba8::new(col.r(), col.g(), col.b(), col.a());
                *hex = n.to_hex(with_alpha);
                changed = true;
            }
            let edit = egui::TextEdit::singleline(hex)
                .desired_width(86.0)
                .font(egui::TextStyle::Monospace);
            if ui.add(edit).changed() {
                changed = true;
            }
        });
    });
    changed
}

fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.panel_fill = C::PANEL;
    visuals.window_fill = C::PANEL;
    visuals.extreme_bg_color = C::RAISED;
    visuals.faint_bg_color = C::BG;
    visuals.code_bg_color = C::RAISED;
    visuals.override_text_color = Some(C::FG);
    visuals.hyperlink_color = C::ACCENT;
    visuals.selection.bg_fill = Color32::from_rgb(61, 70, 84);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, C::ACCENT);
    visuals.widgets.noninteractive.bg_fill = C::PANEL;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, C::MUTED);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, C::LINE);
    visuals.widgets.inactive.bg_fill = C::RAISED;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, C::FG);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, C::LINE);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 36, 44);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, C::FG);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(70, 78, 88));
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.active.bg_fill = Color32::from_rgb(38, 45, 54);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.open.bg_fill = C::RAISED;
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(4);
    visuals.window_corner_radius = egui::CornerRadius::ZERO;
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;

    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals_of(egui::Theme::Light, visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.slider_width = 140.0;
        style.spacing.interact_size.y = 28.0;
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
    });
}

fn apply_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    if let Some(bytes) = load_system_cjk() {
        fonts.font_data.insert(
            "cjk_system".into(),
            Arc::new(FontData::from_owned(bytes)),
        );
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            if let Some(list) = fonts.families.get_mut(&family) {
                list.push("cjk_system".into());
            }
        }
    }
    fonts.font_data.insert(
        "cjk".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/ui_cjk.ttf"))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            list.push("cjk".into());
        }
    }
    ctx.set_fonts(fonts);
}

fn load_system_cjk() -> Option<Vec<u8>> {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\msjh.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(ttf) = font_bytes_to_ttf(&bytes) {
                return Some(ttf);
            }
        }
    }
    None
}

fn font_bytes_to_ttf(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.starts_with(b"ttcf") {
        if bytes.len() < 16 {
            return None;
        }
        let n = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
        if n == 0 {
            return None;
        }
        let off = u32::from_be_bytes(bytes[12..16].try_into().ok()?) as usize;
        extract_sfnt(bytes, off)
    } else if bytes.len() > 16 {
        Some(bytes.to_vec())
    } else {
        None
    }
}

fn extract_sfnt(bytes: &[u8], start: usize) -> Option<Vec<u8>> {
    if bytes.len() < start + 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes(bytes[start + 4..start + 6].try_into().ok()?) as usize;
    let header_len = 12 + num_tables * 16;
    if bytes.len() < start + header_len {
        return None;
    }
    let mut recs = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let o = start + 12 + i * 16;
        let tag = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
        let cs = u32::from_be_bytes(bytes[o + 4..o + 8].try_into().ok()?);
        let off = u32::from_be_bytes(bytes[o + 8..o + 12].try_into().ok()?) as usize;
        let len = u32::from_be_bytes(bytes[o + 12..o + 16].try_into().ok()?) as usize;
        recs.push((tag, cs, off, len));
    }
    let mut cursor = header_len;
    let mut new_recs = Vec::new();
    let mut blobs = Vec::new();
    for (tag, cs, off, len) in recs {
        cursor = (cursor + 3) & !3;
        new_recs.push((tag, cs, cursor as u32, len as u32));
        let blob = bytes.get(off..off + len)?.to_vec();
        blobs.push((cursor, blob));
        cursor += len;
    }
    let mut out = Vec::with_capacity(cursor);
    out.extend_from_slice(&bytes[start..start + 12]);
    for (tag, cs, off, len) in new_recs {
        out.extend_from_slice(&tag);
        out.extend_from_slice(&cs.to_be_bytes());
        out.extend_from_slice(&off.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
    }
    let mut pos = out.len();
    for (want, blob) in blobs {
        while pos < want {
            out.push(0);
            pos += 1;
        }
        out.extend_from_slice(&blob);
        pos += blob.len();
    }
    Some(out)
}

fn bundled_font() -> Vec<u8> {
    include_bytes!("../../../testdata/DejaVuSans.ttf").to_vec()
}


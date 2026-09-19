use eframe::egui::{self, Color32, RichText, TextureHandle, TextureOptions, Vec2};
use fntforge_core::{
    extract_chars, generate, write_fnt, write_font_files, AlignH, CharsetPreset, Fill, Project,
    Rgba8, StrokePosition, StyleStack,
};
use std::path::PathBuf;

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
    preview_tex: Option<TextureHandle>,
    dirty: bool,
    last_fnt_preview: String,
    font_path: Option<PathBuf>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let bytes = bundled_font();
        let mut project = Project::new(bytes, "DejaVu Sans");
        project.style.stroke.enabled = true;
        project.style.stroke.size = 2.0;
        project.style.stroke.color = Rgba8::new(20, 24, 28, 255);
        project.style.fill = Fill::Solid(Rgba8::new(236, 239, 241, 255));
        project.style.drop_shadow.enabled = true;
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
            status: "Load a TTF, tune styles, export .fnt for Cocos2d-x Lua.".into(),
            atlas_tex: None,
            preview_tex: None,
            dirty: true,
            last_fnt_preview: String::new(),
            font_path: None,
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
                }
                self.last_fnt_preview = write_fnt(&font, "preview").text;
                self.status = format!(
                    "{} glyphs · {}×{} · lineHeight {} · base {}",
                    font.glyphs.len(),
                    font.scale_w,
                    font.scale_h,
                    font.line_height,
                    font.base
                );
                self.dirty = false;
            }
            Err(e) => {
                self.status = format!("Generate failed: {e}");
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
                    RichText::new("  BMFont for Cocos2d-x Lua")
                        .color(Color32::from_rgb(139, 145, 154)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export .fnt").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("font.fnt")
                            .add_filter("BMFont", &["fnt"])
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
                                        Ok(_) => self.status = format!("Exported {}", path.display()),
                                        Err(e) => self.status = format!("Write failed: {e}"),
                                    }
                                }
                                Err(e) => self.status = format!("Generate failed: {e}"),
                            }
                        }
                    }
                    if ui.button("Open TTF").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Font", &["ttf", "otf", "ttc"])
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
                                Err(e) => self.status = format!("Read failed: {e}"),
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
                ui.label(RichText::new("Font").strong());
                ui.add(egui::Slider::new(&mut self.project.font_size, 12.0..=128.0).text("Size"));
                ui.checkbox(&mut self.project.tabular_nums, "Tabular digits");
                ui.add(
                    egui::Slider::new(&mut self.project.extra_letter_spacing, -8..=16)
                        .text("Letter spacing"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.extra_line_height, -16..=48)
                        .text("Line height +"),
                );
                ui.separator();
                ui.label(RichText::new("Charset").strong());
                ui.horizontal_wrapped(|ui| {
                    if ui.button("ASCII").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::Ascii);
                        self.dirty = true;
                    }
                    if ui.button("Digits").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::Numbers);
                        self.dirty = true;
                    }
                    if ui.button("Latin-1").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::Latin1);
                        self.dirty = true;
                    }
                    if ui.button("Punct").clicked() {
                        self.charset_edit = fntforge_core::preset_chars(CharsetPreset::CommonPunct);
                        self.dirty = true;
                    }
                });
                ui.label("Characters");
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
                ui.label(RichText::new("Pack").strong());
                ui.add(
                    egui::Slider::new(&mut self.project.pack.max_size, 256..=4096).text("Max atlas"),
                );
                ui.checkbox(&mut self.project.pack.power_of_two, "Power of two");
                ui.checkbox(&mut self.project.pack.square, "Square");
            });

        egui::SidePanel::right("style")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("Layer styles").strong());
                ui.horizontal(|ui| {
                    ui.label("Fill");
                    if ui.text_edit_singleline(&mut self.fill_hex).changed() {
                        self.dirty = true;
                    }
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.stroke.enabled, "Stroke")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.style.stroke.size, 0.5..=12.0).text("Size"),
                );
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.project.style.stroke.position,
                        StrokePosition::Outer,
                        "Outer",
                    );
                    ui.selectable_value(
                        &mut self.project.style.stroke.position,
                        StrokePosition::Center,
                        "Center",
                    );
                    ui.selectable_value(
                        &mut self.project.style.stroke.position,
                        StrokePosition::Inner,
                        "Inner",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Color");
                    ui.text_edit_singleline(&mut self.stroke_hex);
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.drop_shadow.enabled, "Drop shadow")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.distance, 0.0..=16.0)
                        .text("Distance"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.size, 0.0..=16.0)
                        .text("Blur"),
                );
                ui.add(
                    egui::Slider::new(&mut self.project.style.drop_shadow.angle_deg, 0.0..=360.0)
                        .text("Angle"),
                );
                ui.horizontal(|ui| {
                    ui.label("Color");
                    ui.text_edit_singleline(&mut self.shadow_hex);
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.outer_glow.enabled, "Outer glow")
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.project.style.outer_glow.size, 0.5..=24.0)
                        .text("Size"),
                );
                ui.horizontal(|ui| {
                    ui.label("Color");
                    ui.text_edit_singleline(&mut self.glow_hex);
                });
                ui.separator();
                if ui
                    .checkbox(&mut self.project.style.inner_shadow.enabled, "Inner shadow")
                    .changed()
                {
                    self.dirty = true;
                }
                if ui
                    .checkbox(&mut self.project.style.inner_glow.enabled, "Inner glow")
                    .changed()
                {
                    self.dirty = true;
                }
                if ui
                    .checkbox(&mut self.project.style.color_overlay.enabled, "Color overlay")
                    .changed()
                {
                    self.dirty = true;
                }
                if ui.button("Apply styles").clicked() {
                    self.dirty = true;
                }
                if ui.button("Reset styles").clicked() {
                    self.project.style = StyleStack::default();
                    self.dirty = true;
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Preview");
                ui.selectable_value(&mut self.project.align_h, AlignH::Left, "Left");
                ui.selectable_value(&mut self.project.align_h, AlignH::Center, "Center");
                ui.selectable_value(&mut self.project.align_h, AlignH::Right, "Right");
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
            ui.separator();
            ui.label("Atlas");
            if let Some(tex) = &self.atlas_tex {
                let avail = ui.available_size();
                let size = tex.size_vec2();
                let scale = (avail.x / size.x).min(avail.y * 0.55 / size.y).min(4.0);
                ui.image((tex.id(), size * scale.max(0.1)));
            }
            ui.separator();
            ui.collapsing(".fnt preview", |ui| {
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

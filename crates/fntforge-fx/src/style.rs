use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(hex: &str) -> Self {
        let h = hex.trim().trim_start_matches('#');
        let parse = |i| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
        if h.len() >= 8 {
            Self::new(parse(0), parse(2), parse(4), parse(6))
        } else {
            Self::new(parse(0), parse(2), parse(4), 255)
        }
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    LinearDodge,
    ColorDodge,
    LinearBurn,
    ColorBurn,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrokePosition {
    Outer,
    Center,
    Inner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contour {
    Linear,
    Gaussian,
    Cone,
    Ring,
}

impl Contour {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Contour::Linear => t,
            Contour::Gaussian => {
                let x = (t - 0.5) * 3.0;
                (-(x * x)).exp()
            }
            Contour::Cone => 1.0 - (t - 0.5).abs() * 2.0,
            Contour::Ring => ((t * std::f32::consts::PI * 2.0).sin().abs()).clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fill {
    Solid { color: Rgba8 },
    Linear {
        angle_deg: f32,
        stops: Vec<(f32, Rgba8)>,
    },
}

impl Default for Fill {
    fn default() -> Self {
        Fill::Solid {
            color: Rgba8::WHITE,
        }
    }
}

impl Fill {
    pub fn solid(c: Rgba8) -> Self {
        Fill::Solid { color: c }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub enabled: bool,
    pub size: f32,
    pub position: StrokePosition,
    pub color: Rgba8,
    pub opacity: f32,
    pub blend: BlendMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub enabled: bool,
    pub color: Rgba8,
    pub distance: f32,
    pub angle_deg: f32,
    pub size: f32,
    pub spread: f32,
    pub opacity: f32,
    pub blend: BlendMode,
    pub use_global_light: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glow {
    pub enabled: bool,
    pub color: Rgba8,
    pub size: f32,
    pub spread: f32,
    pub opacity: f32,
    pub blend: BlendMode,
    pub contour: Contour,
    pub from_center: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overlay {
    pub enabled: bool,
    pub color: Rgba8,
    pub opacity: f32,
    pub blend: BlendMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientOverlay {
    pub enabled: bool,
    pub angle_deg: f32,
    pub opacity: f32,
    pub blend: BlendMode,
    pub stops: Vec<(f32, Rgba8)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bevel {
    pub enabled: bool,
    pub size: f32,
    pub soften: f32,
    pub depth: f32,
    pub highlight: Rgba8,
    pub shadow: Rgba8,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Satin {
    pub enabled: bool,
    pub color: Rgba8,
    pub distance: f32,
    pub angle_deg: f32,
    pub size: f32,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalLight {
    pub angle_deg: f32,
    pub altitude_deg: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleStack {
    pub fill: Fill,
    pub stroke: Stroke,
    pub drop_shadow: Shadow,
    pub inner_shadow: Shadow,
    pub outer_glow: Glow,
    pub inner_glow: Glow,
    pub color_overlay: Overlay,
    pub gradient_overlay: GradientOverlay,
    pub bevel: Bevel,
    pub satin: Satin,
    pub global_light: GlobalLight,
}

impl Default for StyleStack {
    fn default() -> Self {
        Self {
            fill: Fill::solid(Rgba8::WHITE),
            stroke: Stroke {
                enabled: false,
                size: 2.0,
                position: StrokePosition::Outer,
                color: Rgba8::BLACK,
                opacity: 1.0,
                blend: BlendMode::Normal,
            },
            drop_shadow: Shadow {
                enabled: false,
                color: Rgba8::new(0, 0, 0, 160),
                distance: 2.0,
                angle_deg: 120.0,
                size: 2.0,
                spread: 0.0,
                opacity: 1.0,
                blend: BlendMode::Multiply,
                use_global_light: true,
            },
            inner_shadow: Shadow {
                enabled: false,
                color: Rgba8::new(0, 0, 0, 140),
                distance: 1.0,
                angle_deg: 120.0,
                size: 2.0,
                spread: 0.0,
                opacity: 1.0,
                blend: BlendMode::Multiply,
                use_global_light: true,
            },
            outer_glow: Glow {
                enabled: false,
                color: Rgba8::new(255, 200, 80, 180),
                size: 4.0,
                spread: 0.0,
                opacity: 1.0,
                blend: BlendMode::Screen,
                contour: Contour::Linear,
                from_center: false,
            },
            inner_glow: Glow {
                enabled: false,
                color: Rgba8::new(255, 255, 255, 120),
                size: 3.0,
                spread: 0.0,
                opacity: 1.0,
                blend: BlendMode::Screen,
                contour: Contour::Linear,
                from_center: false,
            },
            color_overlay: Overlay {
                enabled: false,
                color: Rgba8::new(255, 210, 80, 255),
                opacity: 1.0,
                blend: BlendMode::Normal,
            },
            gradient_overlay: GradientOverlay {
                enabled: false,
                angle_deg: 90.0,
                opacity: 1.0,
                blend: BlendMode::Normal,
                stops: vec![
                    (0.0, Rgba8::new(255, 244, 180, 255)),
                    (1.0, Rgba8::new(196, 146, 40, 255)),
                ],
            },
            bevel: Bevel {
                enabled: false,
                size: 3.0,
                soften: 1.0,
                depth: 1.0,
                highlight: Rgba8::new(255, 255, 255, 180),
                shadow: Rgba8::new(0, 0, 0, 160),
                opacity: 0.85,
            },
            satin: Satin {
                enabled: false,
                color: Rgba8::new(0, 0, 0, 90),
                distance: 4.0,
                angle_deg: 20.0,
                size: 4.0,
                opacity: 0.7,
            },
            global_light: GlobalLight {
                angle_deg: 120.0,
                altitude_deg: 30.0,
            },
        }
    }
}

impl StyleStack {
    pub fn padding(&self) -> i32 {
        let mut p = 1.0f32;
        if self.stroke.enabled {
            let extra = match self.stroke.position {
                StrokePosition::Inner => 0.0,
                StrokePosition::Center => self.stroke.size * 0.5,
                StrokePosition::Outer => self.stroke.size,
            };
            p = p.max(extra + 1.0);
        }
        if self.drop_shadow.enabled {
            p = p.max(self.drop_shadow.distance + self.drop_shadow.size + 2.0);
        }
        if self.outer_glow.enabled {
            p = p.max(self.outer_glow.size + self.outer_glow.spread + 2.0);
        }
        if self.bevel.enabled {
            p = p.max(self.bevel.size);
        }
        p.ceil() as i32
    }

    /// 金币标题：金渐变 + 深描边 + 柔光，对应游戏 HUD。
    pub fn preset_gold() -> Self {
        let mut s = Self::default();
        s.fill = Fill::Linear {
            angle_deg: 90.0,
            stops: vec![
                (0.0, Rgba8::new(255, 243, 160, 255)),
                (0.45, Rgba8::new(232, 196, 74, 255)),
                (1.0, Rgba8::new(184, 132, 28, 255)),
            ],
        };
        s.stroke.enabled = true;
        s.stroke.size = 1.5;
        s.stroke.position = StrokePosition::Outer;
        s.stroke.color = Rgba8::new(42, 24, 8, 255);
        s.outer_glow.enabled = true;
        s.outer_glow.size = 6.0;
        s.outer_glow.color = Rgba8::new(230, 208, 96, 170);
        s.outer_glow.blend = BlendMode::Screen;
        s.drop_shadow.enabled = false;
        s.inner_shadow.enabled = true;
        s.inner_shadow.distance = 1.0;
        s.inner_shadow.size = 1.5;
        s.inner_shadow.color = Rgba8::new(80, 40, 0, 90);
        s
    }

    pub fn preset_hp_red() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::new(255, 72, 72, 255));
        s.stroke.enabled = true;
        s.stroke.size = 2.0;
        s.stroke.color = Rgba8::new(70, 8, 8, 255);
        s.drop_shadow.enabled = true;
        s
    }

    pub fn preset_white_outline() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::WHITE);
        s.stroke.enabled = true;
        s.stroke.size = 2.5;
        s.stroke.color = Rgba8::new(20, 24, 28, 255);
        s.drop_shadow.enabled = true;
        s.drop_shadow.distance = 1.0;
        s
    }

    pub fn preset_crit() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::new(255, 230, 80, 255));
        s.stroke.enabled = true;
        s.stroke.size = 2.0;
        s.stroke.color = Rgba8::new(120, 40, 0, 255);
        s.outer_glow.enabled = true;
        s.outer_glow.color = Rgba8::new(255, 120, 40, 200);
        s.outer_glow.size = 5.0;
        s
    }

    pub fn preset_neon() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::new(180, 255, 255, 255));
        s.outer_glow.enabled = true;
        s.outer_glow.color = Rgba8::new(0, 220, 255, 220);
        s.outer_glow.size = 8.0;
        s.stroke.enabled = true;
        s.stroke.size = 1.0;
        s.stroke.color = Rgba8::new(0, 80, 120, 255);
        s
    }

    pub fn preset_disabled() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::new(160, 164, 170, 255));
        s.stroke.enabled = true;
        s.stroke.size = 1.5;
        s.stroke.color = Rgba8::new(60, 64, 70, 255);
        s
    }

    pub fn preset_stone() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::new(186, 176, 158, 255));
        s.bevel.enabled = true;
        s.bevel.size = 3.0;
        s.drop_shadow.enabled = true;
        s.inner_shadow.enabled = true;
        s
    }

    pub fn preset_pixel() -> Self {
        let mut s = Self::default();
        s.fill = Fill::solid(Rgba8::new(255, 236, 180, 255));
        s.stroke.enabled = true;
        s.stroke.size = 1.0;
        s.stroke.color = Rgba8::BLACK;
        s.drop_shadow.enabled = false;
        s
    }
}

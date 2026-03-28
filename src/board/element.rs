use glam::Vec2;
use serde::{Deserialize, Serialize};
use crate::palette;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShapeType {
    Rect,
    Ellipse,
    Line,
    Image,
}

pub const DEFAULT_TEXT_COLOR: [f32; 4] = palette::GRAY_3;
pub const DEFAULT_RECT_COLOR: [f32; 4] = palette::OLIVE_LIGHT;
pub const DEFAULT_ELLIPSE_COLOR: [f32; 4] = palette::TEAL;
pub const DEFAULT_STICKY_COLOR: [f32; 4] = palette::YELLOW_PALE;
pub const DEFAULT_LINE_COLOR: [f32; 4] = palette::BLACK;
pub const DEFAULT_STROKE_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
pub const DEFAULT_BORDER_WIDTH: u8 = 2;
pub const DEFAULT_LINE_STROKE_WIDTH: u8 = 2;
pub const DEFAULT_BOX_STROKE_COLOR: [f32; 4] = palette::BLACK;
pub const DEFAULT_LINE_ARROW_START: bool = false;
pub const DEFAULT_LINE_ARROW_END: bool = true;

pub fn default_text_box_color() -> [f32; 4] {
    let mut color = DEFAULT_TEXT_COLOR;
    color[3] = 0.0;
    color
}

pub fn default_stroke_color() -> [f32; 4] {
    DEFAULT_STROKE_COLOR
}

pub fn default_border_width() -> u8 {
    DEFAULT_BORDER_WIDTH
}

pub fn default_line_stroke_width() -> u8 {
    DEFAULT_LINE_STROKE_WIDTH
}

pub fn default_line_arrow_disabled() -> bool {
    false
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxToolStyle {
    pub fill_color: [f32; 4],
    pub stroke_color: [f32; 4],
    pub border_width: u8,
    pub text_color: [f32; 4],
}

impl BoxToolStyle {
    pub fn rect_default() -> Self {
        Self {
            fill_color: DEFAULT_RECT_COLOR,
            stroke_color: DEFAULT_BOX_STROKE_COLOR,
            border_width: DEFAULT_BORDER_WIDTH,
            text_color: DEFAULT_TEXT_COLOR,
        }
    }

    pub fn ellipse_default() -> Self {
        Self {
            fill_color: DEFAULT_ELLIPSE_COLOR,
            stroke_color: DEFAULT_BOX_STROKE_COLOR,
            border_width: DEFAULT_BORDER_WIDTH,
            text_color: DEFAULT_TEXT_COLOR,
        }
    }

    pub fn text_default() -> Self {
        Self {
            fill_color: default_text_box_color(),
            stroke_color: palette::TRANSPARENT,
            border_width: 0,
            text_color: DEFAULT_TEXT_COLOR,
        }
    }

    pub fn sticky_default() -> Self {
        Self {
            fill_color: DEFAULT_STICKY_COLOR,
            stroke_color: palette::TRANSPARENT,
            border_width: 0,
            text_color: palette::BLACK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineToolStyle {
    pub color: [f32; 4],
    pub stroke_width: u8,
    pub arrow_start: bool,
    pub arrow_end: bool,
}

impl LineToolStyle {
    pub fn default_line() -> Self {
        Self {
            color: DEFAULT_LINE_COLOR,
            stroke_width: DEFAULT_LINE_STROKE_WIDTH,
            arrow_start: DEFAULT_LINE_ARROW_START,
            arrow_end: DEFAULT_LINE_ARROW_END,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolStyleDefaults {
    pub rect: BoxToolStyle,
    pub ellipse: BoxToolStyle,
    pub sticky: BoxToolStyle,
    pub text: BoxToolStyle,
    pub line: LineToolStyle,
}

impl Default for ToolStyleDefaults {
    fn default() -> Self {
        Self {
            rect: BoxToolStyle::rect_default(),
            ellipse: BoxToolStyle::ellipse_default(),
            sticky: BoxToolStyle::sticky_default(),
            text: BoxToolStyle::text_default(),
            line: LineToolStyle::default_line(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementStyleSnapshot {
    pub fill_color: [f32; 4],
    pub stroke_color: [f32; 4],
    pub border_width: Option<u8>,
    pub stroke_width: Option<u8>,
    pub line_arrow_start: Option<bool>,
    pub line_arrow_end: Option<bool>,
    pub text_color: Option<[f32; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub asset_path: String,
    #[serde(default)]
    pub hires_asset_path: Option<String>,
    pub original_width: u32,
    pub original_height: u32,
    pub base_width: u32,
    pub base_height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextData {
    pub content: String,
    pub font_size: f32,
    pub color: [f32; 4],
}

impl Default for TextData {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_size: 24.0,
            color: DEFAULT_TEXT_COLOR,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineAnchor {
    pub target_id: u64,
    pub norm_pos: Vec2,
}

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineEndpoints {
    pub start: Option<LineAnchor>,
    pub end: Option<LineAnchor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Element {
    pub id: u64,
    pub shape: ShapeType,
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
    pub color: [f32; 4],
    #[serde(default = "default_stroke_color")]
    pub stroke_color: [f32; 4],
    #[serde(default = "default_border_width")]
    pub border_width: u8,
    #[serde(default = "default_line_stroke_width")]
    pub stroke_width: u8,
    #[serde(default = "default_line_arrow_disabled")]
    pub line_arrow_start: bool,
    #[serde(default = "default_line_arrow_disabled")]
    pub line_arrow_end: bool,
    pub selected: bool,
    #[serde(default)]
    pub text: Option<TextData>,
    #[serde(default)]
    pub image: Option<ImageData>,
    #[serde(skip, default)]
    pub text_layout_generation: u64,
}

impl Element {
    pub fn uses_border_width(&self) -> bool {
        matches!(self.shape, ShapeType::Rect | ShapeType::Ellipse)
    }

    pub fn uses_stroke_width(&self) -> bool {
        self.shape == ShapeType::Line
    }

    pub fn current_text_color(&self) -> Option<[f32; 4]> {
        self.can_host_text().then(|| {
            self.text
                .as_ref()
                .map(|text| text.color)
                .unwrap_or(DEFAULT_TEXT_COLOR)
        })
    }

    pub fn effective_stroke_color(&self) -> [f32; 4] {
        if self.shape == ShapeType::Line && self.stroke_color[3] <= 0.0 {
            self.color
        } else {
            self.stroke_color
        }
    }

    pub fn style_snapshot(&self) -> ElementStyleSnapshot {
        ElementStyleSnapshot {
            fill_color: self.color,
            stroke_color: self.effective_stroke_color(),
            border_width: self.uses_border_width().then_some(self.border_width),
            stroke_width: self.uses_stroke_width().then_some(self.stroke_width.max(1)),
            line_arrow_start: self.uses_stroke_width().then_some(self.line_arrow_start),
            line_arrow_end: self.uses_stroke_width().then_some(self.line_arrow_end),
            text_color: self.current_text_color(),
        }
    }

    pub fn apply_style_snapshot(&mut self, style: ElementStyleSnapshot) {
        self.color = style.fill_color;
        self.stroke_color = style.stroke_color;
        if self.uses_border_width() {
            self.border_width = style.border_width.unwrap_or(0);
        }
        if self.uses_stroke_width() {
            self.stroke_width = style.stroke_width.unwrap_or(DEFAULT_LINE_STROKE_WIDTH).max(1);
            self.line_arrow_start = style.line_arrow_start.unwrap_or(false);
            self.line_arrow_end = style.line_arrow_end.unwrap_or(false);
        }

        if self.shape == ShapeType::Line {
            self.color = style.stroke_color;
        }

        if self.can_host_text() {
            if let Some(text_color) = style.text_color {
                match self.text.as_mut() {
                    Some(text) => text.color = text_color,
                    None => {
                        self.text = Some(TextData {
                            color: text_color,
                            ..TextData::default()
                        });
                    }
                }
            }
        }
    }

    pub fn aabb(&self) -> (Vec2, Vec2) {
        match self.shape {
            ShapeType::Line => {
                let end = self.pos + self.size;
                let min = self.pos.min(end);
                let max = self.pos.max(end);
                (min, max)
            }
            _ => {
                let center = self.pos + self.size * 0.5;
                let hs = self.size * 0.5;
                let cos_r = self.rotation.cos().abs();
                let sin_r = self.rotation.sin().abs();
                let rx = hs.x * cos_r + hs.y * sin_r;
                let ry = hs.x * sin_r + hs.y * cos_r;
                let extents = Vec2::new(rx, ry);
                (center - extents, center + extents)
            }
        }
    }

    pub fn can_host_text(&self) -> bool {
        matches!(self.shape, ShapeType::Rect | ShapeType::Ellipse)
    }

    pub fn bump_text_generation(&mut self) {
        self.text_layout_generation = self.text_layout_generation.wrapping_add(1);
    }

    pub fn text_bounds(&self) -> Option<(Vec2, Vec2)> {
        if !self.can_host_text() {
            return None;
        }

        let padding = Vec2::splat(12.0);
        let min = self.pos + padding;
        let max = min + (self.size - padding * 2.0).max(Vec2::splat(1.0));
        Some((min, max))
    }
}

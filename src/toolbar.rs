use glam::Vec2;
use miniquad::{
    FilterMode, MipmapFilterMode, RenderingBackend, TextureAccess, TextureFormat, TextureId,
    TextureParams, TextureSource, TextureWrap,
};

use crate::input::SelectionBounds;
use crate::palette;
use crate::renderer::{ImageInstanceData, InstanceData, PreparedImageDraw};
use crate::stats::emit_text;

pub const TOOLBAR_HEIGHT: f32 = 48.0;
pub const BTN_W: f32 = 52.0;
pub const BTN_H: f32 = 48.0;
pub const BTN_PAD: f32 = 4.0;
pub const TOOLBAR_BOTTOM_MARGIN: f32 = 16.0;

const TOOLBAR_BG_COLOR: [f32; 4] = palette::PALETTE_GRAY_0;
const TOOLBAR_HOVER_COLOR: [f32; 4] = palette::PALETTE_GRAY_1;
const TOOLBAR_ACTIVE_COLOR: [f32; 4] = palette::PALETTE_GRAY_2;
const TOOLBAR_ACTIVE_HOVER_COLOR: [f32; 4] = palette::PALETTE_BLUE_GRAY;
const TOOLBAR_ICON_COLOR: [f32; 4] = palette::PALETTE_BLACK;
const TOOLBAR_ICON_SIZE: f32 = 32.0;


// When free drag on screen
const MARQUEE_COLOR: [f32; 4] = palette::PALETTE_BLUE;

// When creating new element or dragging existing one
const CREATION_OUTLINE_COLOR: [f32; 4] = palette::PALETTE_BLUE;


const MULTI_SELECTION_BOUNDS_COLOR: [f32; 4] = palette::PALETTE_BLUE;


const FIXED_SCREEN_OUTLINE_SHAPE_TYPE: f32 = 5.0;
const FIXED_SCREEN_ELLIPSE_OUTLINE_SHAPE_TYPE: f32 = 6.0;
const FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Line,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolbarAction {
    SetTool(Tool),
    ImportImage,
    Save,
    Load,
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug)]
pub struct ToolbarLayout {
    pub origin: Vec2,
    pub size: Vec2,
}

#[derive(Clone, Copy, Debug)]
enum BtnKind {
    Select,
    Rect,
    Ellipse,
    Line,
    Text,
    Image,
    Save,
    Load,
    Undo,
    Redo,
}

struct Button {
    kind: BtnKind,
    x: f32,  // left edge in screen pixels
}

pub struct ToolbarIcons {
    select: TextureId,
    rect: TextureId,
    ellipse: TextureId,
    line: TextureId,
    text: TextureId,
    image: TextureId,
    save: TextureId,
    load: TextureId,
}

impl ToolbarIcons {
    pub fn new(ctx: &mut dyn RenderingBackend) -> Self {
        Self {
            select: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/select.png")),
            rect: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/rect.png")),
            ellipse: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/ellipse.png")),
            line: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/line.png")),
            text: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/text.png")),
            image: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/image.png")),
            save: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/save.png")),
            load: load_toolbar_icon(ctx, include_bytes!("../assets/toolbar/load.png")),
        }
    }

    pub fn destroy(&self, ctx: &mut dyn RenderingBackend) {
        for texture in [
            self.select,
            self.rect,
            self.ellipse,
            self.line,
            self.text,
            self.image,
            self.save,
            self.load,
        ] {
            ctx.delete_texture(texture);
        }
    }

    fn texture_for(&self, kind: BtnKind) -> Option<TextureId> {
        Some(match kind {
            BtnKind::Select => self.select,
            BtnKind::Rect => self.rect,
            BtnKind::Ellipse => self.ellipse,
            BtnKind::Line => self.line,
            BtnKind::Text => self.text,
            BtnKind::Image => self.image,
            BtnKind::Save => self.save,
            BtnKind::Load => self.load,
            BtnKind::Undo | BtnKind::Redo => return None,
        })
    }
}

pub struct Toolbar {
    pub active_tool: Tool,
    buttons: [Button; 10],
}

impl Toolbar {
    pub fn new() -> Self {
        let kinds = [
            BtnKind::Select,
            BtnKind::Rect,
            BtnKind::Ellipse,
            BtnKind::Line,
            BtnKind::Text,
            BtnKind::Image,
            BtnKind::Save,
            BtnKind::Load,
            BtnKind::Undo,
            BtnKind::Redo,
        ];
        let buttons = std::array::from_fn(|i| Button {
            kind: kinds[i],
            x: BTN_PAD + i as f32 * (BTN_W + BTN_PAD),
        });
        Self { active_tool: Tool::Select, buttons }
    }

    pub fn layout(&self, screen_size: Vec2) -> ToolbarLayout {
        let width = self.buttons.len() as f32 * BTN_W + (self.buttons.len() as f32 + 1.0) * BTN_PAD;
        let origin = Vec2::new(
            ((screen_size.x - width) * 0.5).max(0.0),
            (screen_size.y - TOOLBAR_HEIGHT - TOOLBAR_BOTTOM_MARGIN).max(0.0),
        );
        ToolbarLayout {
            origin,
            size: Vec2::new(width, TOOLBAR_HEIGHT),
        }
    }

    pub fn contains_point(&self, screen_size: Vec2, x: f32, y: f32) -> bool {
        let layout = self.layout(screen_size);
        x >= layout.origin.x
            && x < layout.origin.x + layout.size.x
            && y >= layout.origin.y
            && y < layout.origin.y + layout.size.y
    }

    pub fn hit_test(&self, screen_size: Vec2, x: f32, y: f32) -> Option<ToolbarAction> {
        let layout = self.layout(screen_size);
        let local_x = x - layout.origin.x;
        let local_y = y - layout.origin.y;

        if local_y < 0.0 || local_y >= TOOLBAR_HEIGHT {
            return None;
        }
        for btn in &self.buttons {
            if local_x >= btn.x && local_x < btn.x + BTN_W {
                return Some(match btn.kind {
                    BtnKind::Select  => ToolbarAction::SetTool(Tool::Select),
                    BtnKind::Rect    => ToolbarAction::SetTool(Tool::Rect),
                    BtnKind::Ellipse => ToolbarAction::SetTool(Tool::Ellipse),
                    BtnKind::Line    => ToolbarAction::SetTool(Tool::Line),
                    BtnKind::Text    => ToolbarAction::SetTool(Tool::Text),
                    BtnKind::Image   => ToolbarAction::ImportImage,
                    BtnKind::Save    => ToolbarAction::Save,
                    BtnKind::Load    => ToolbarAction::Load,
                    BtnKind::Undo    => ToolbarAction::Undo,
                    BtnKind::Redo    => ToolbarAction::Redo,
                });
            }
        }
        None
    }

    pub fn hovered_action(&self, screen_size: Vec2, x: f32, y: f32) -> Option<ToolbarAction> {
        self.hit_test(screen_size, x, y)
    }

    /// Build screen-space instance data for the toolbar background, buttons,
    /// and icons in a bottom-centered island rect.
    pub fn build_instances(
        &self,
        screen_size: Vec2,
        mouse_pos: Vec2,
        can_undo: bool,
        can_redo: bool,
    ) -> Vec<InstanceData> {
        let mut out: Vec<InstanceData> = Vec::new();
        let layout = self.layout(screen_size);
        let hovered_action = self.hovered_action(screen_size, mouse_pos.x, mouse_pos.y);

        // Toolbar island background
        out.push(InstanceData::new(
            layout.origin.to_array(),
            layout.size.to_array(),
            0.0,
            TOOLBAR_BG_COLOR,
            0.0,
            1.0,
        ));

        for btn in &self.buttons {
            let is_active = matches!(
                (&btn.kind, self.active_tool),
                (BtnKind::Select, Tool::Select)
                | (BtnKind::Rect, Tool::Rect)
                | (BtnKind::Ellipse, Tool::Ellipse)
                | (BtnKind::Line, Tool::Line)
                | (BtnKind::Text, Tool::Text)
            );

            let dimmed = matches!(
                &btn.kind,
                BtnKind::Undo if !can_undo
            ) || matches!(
                &btn.kind,
                BtnKind::Redo if !can_redo
            );

            let is_hovered = !dimmed
                && hovered_action
                    .map(|action| matches_button_action(btn.kind, action))
                    .unwrap_or(false);

            let button_color = if is_active && is_hovered {
                Some(TOOLBAR_ACTIVE_HOVER_COLOR)
            } else if is_active {
                Some(TOOLBAR_ACTIVE_COLOR)
            } else if is_hovered {
                Some(TOOLBAR_HOVER_COLOR)
            } else {
                None
            };

            if let Some(button_color) = button_color {
                out.push(InstanceData::new(
                    [layout.origin.x + btn.x + 2.0, layout.origin.y + 4.0],
                    [BTN_W - 4.0, BTN_H - 8.0],
                    0.0,
                    button_color,
                    0.0,
                    1.0,
                ));
            }

            let icon_alpha = if dimmed {
                0.3
            } else if is_hovered {
                1.0
            } else {
                0.9
            };
            let icon_color = [
                TOOLBAR_ICON_COLOR[0],
                TOOLBAR_ICON_COLOR[1],
                TOOLBAR_ICON_COLOR[2],
                icon_alpha,
            ];
            let cx = layout.origin.x + btn.x + BTN_W * 0.5;
            let cy = layout.origin.y + BTN_H * 0.5;

            // Text label: 3×5 bitmap font, scale=2 → glyph is 6px wide, 10px tall
            // stride per char = (3+1)*2 = 8px
            const SCALE: f32 = 2.0;
            const CHAR_W: f32 = 3.0 * SCALE; // 6
            const GAP: f32 = SCALE;           // 2
            const STRIDE: f32 = CHAR_W + GAP; // 8
            const GLYPH_H: f32 = 5.0 * SCALE; // 10

            let label = match btn.kind {
                BtnKind::Undo => "UNDO",
                BtnKind::Redo => "REDO",
                _ => continue,
            };

            let text_w = label.len() as f32 * STRIDE - GAP;
            let tx = cx - text_w * 0.5;
            let ty = cy - GLYPH_H * 0.5;
            emit_text(label, tx, ty, SCALE, icon_color, &mut out);
        }
        out
    }

    pub fn build_icon_draws(
        &self,
        screen_size: Vec2,
        mouse_pos: Vec2,
        can_undo: bool,
        can_redo: bool,
        icons: &ToolbarIcons,
    ) -> Vec<PreparedImageDraw> {
        let mut out = Vec::new();
        let layout = self.layout(screen_size);
        let hovered_action = self.hovered_action(screen_size, mouse_pos.x, mouse_pos.y);

        for btn in &self.buttons {
            let Some(texture) = icons.texture_for(btn.kind) else {
                continue;
            };

            let dimmed = matches!(btn.kind, BtnKind::Undo) && !can_undo
                || matches!(btn.kind, BtnKind::Redo) && !can_redo;
            let is_hovered = !dimmed
                && hovered_action
                    .map(|action| matches_button_action(btn.kind, action))
                    .unwrap_or(false);
            let icon_alpha = if dimmed {
                0.3
            } else if is_hovered {
                1.0
            } else {
                0.9
            };
            let tint = [1.0, 1.0, 1.0, icon_alpha];
            let origin_x = layout.origin.x + btn.x + (BTN_W - TOOLBAR_ICON_SIZE) * 0.5;
            let origin_y = layout.origin.y + (BTN_H - TOOLBAR_ICON_SIZE) * 0.5;

            out.push(PreparedImageDraw {
                texture,
                instance: ImageInstanceData::new(
                    [origin_x, origin_y],
                    [TOOLBAR_ICON_SIZE, TOOLBAR_ICON_SIZE],
                    [origin_x, origin_y],
                    0.0,
                    [0.0, 0.0],
                    [1.0, 1.0],
                    tint,
                ),
            });
        }

        out
    }
}

fn matches_button_action(kind: BtnKind, action: ToolbarAction) -> bool {
    matches!(
        (kind, action),
        (BtnKind::Select, ToolbarAction::SetTool(Tool::Select))
            | (BtnKind::Rect, ToolbarAction::SetTool(Tool::Rect))
            | (BtnKind::Ellipse, ToolbarAction::SetTool(Tool::Ellipse))
            | (BtnKind::Line, ToolbarAction::SetTool(Tool::Line))
            | (BtnKind::Text, ToolbarAction::SetTool(Tool::Text))
            | (BtnKind::Image, ToolbarAction::ImportImage)
            | (BtnKind::Save, ToolbarAction::Save)
            | (BtnKind::Load, ToolbarAction::Load)
            | (BtnKind::Undo, ToolbarAction::Undo)
            | (BtnKind::Redo, ToolbarAction::Redo)
    )
}

fn load_toolbar_icon(ctx: &mut dyn RenderingBackend, bytes: &[u8]) -> TextureId {
    let image = image::load_from_memory(bytes)
        .expect("toolbar icon should decode")
        .to_rgba8();
    let (width, height) = image.dimensions();
    debug_assert_eq!(width, height, "toolbar icons should be square");

    ctx.new_texture(
        TextureAccess::Static,
        TextureSource::Bytes(image.as_raw()),
        TextureParams {
            width,
            height,
            format: TextureFormat::RGBA8,
            wrap: TextureWrap::Clamp,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::None,
            allocate_mipmaps: false,
            ..Default::default()
        },
    )
}

/// Convert a world-space element into one or more InstanceData entries.
/// `selected` adds a highlight border.
pub fn element_instance(
    e: &crate::board::Element,
    alpha: f32,
) -> InstanceData {
    let st = match e.shape {
        crate::board::ShapeType::Rect => 0.0,
        crate::board::ShapeType::Ellipse => 1.0,
        crate::board::ShapeType::Line => 2.0,
        crate::board::ShapeType::Text => 3.0,
        crate::board::ShapeType::Image => 255.0,
    };

    InstanceData::new(
        e.pos.to_array(),
        e.size.to_array(),
        e.rotation,
        e.color,
        st,
        alpha,
    )
}

pub fn selection_instance(
    e: &crate::board::Element,
    zoom: f32,
    alpha: f32,
) -> Option<InstanceData> {
    if !e.selected {
        return None;
    }

    Some(selection_outline_instance(e, zoom, alpha))
}

fn selection_outline_instance(
    e: &crate::board::Element,
    zoom: f32,
    alpha: f32,
) -> InstanceData {
    let expand = 1.0 / zoom.max(0.0001);
    let st = match e.shape {
        crate::board::ShapeType::Rect | crate::board::ShapeType::Text | crate::board::ShapeType::Image => FIXED_SCREEN_OUTLINE_SHAPE_TYPE,
        crate::board::ShapeType::Ellipse => FIXED_SCREEN_ELLIPSE_OUTLINE_SHAPE_TYPE,
        crate::board::ShapeType::Line => FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE,
    };

    InstanceData::new(
        (e.pos - Vec2::splat(expand)).to_array(),
        (e.size + Vec2::splat(expand * 2.0)).to_array(),
        e.rotation,
        CREATION_OUTLINE_COLOR,
        st,
        alpha,
    )
}

pub fn selection_bounds_instance(
    bounds: SelectionBounds,
    zoom: f32,
    alpha: f32,
) -> InstanceData {
    bounds_outline_instance(bounds, zoom, MULTI_SELECTION_BOUNDS_COLOR, alpha)
}

pub fn marquee_instance(
    bounds: SelectionBounds,
    zoom: f32,
    alpha: f32,
) -> InstanceData {
    bounds_outline_instance(bounds, zoom, MARQUEE_COLOR, alpha)
}

fn bounds_outline_instance(
    bounds: SelectionBounds,
    zoom: f32,
    color: [f32; 4],
    alpha: f32,
) -> InstanceData {
    let expand = 1.0 / zoom.max(0.0001);
    InstanceData::new(
        (bounds.pos - Vec2::splat(expand)).to_array(),
        (bounds.size + Vec2::splat(expand * 2.0)).to_array(),
        bounds.rotation,
        color,
        FIXED_SCREEN_OUTLINE_SHAPE_TYPE,
        alpha,
    )
}

pub fn preview_instances(
    e: &crate::board::Element,
    zoom: f32,
    alpha: f32,
) -> Vec<InstanceData> {
    let mut instances = element_to_instances(e, alpha);

    if e.shape != crate::board::ShapeType::Line {
        instances.push(selection_outline_instance(e, zoom, 1.0));
    }

    instances
}

pub fn element_to_instances(
    e: &crate::board::Element,
    alpha: f32,
) -> Vec<InstanceData> {
    vec![element_instance(e, alpha)]
}

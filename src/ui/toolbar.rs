use glam::Vec2;
use miniquad::{
    FilterMode, MipmapFilterMode, RenderingBackend, TextureAccess, TextureFormat, TextureId,
    TextureParams, TextureSource, TextureWrap,
};

use crate::palette;
use crate::rendering::renderer::{ImageInstanceData, InstanceData, PreparedImageDraw};
use crate::text::UiTextSpec;
use crate::ui::tool::Tool;

pub const TOOLBAR_HEIGHT: f32 = 48.0;
pub const BTN_W: f32 = 52.0;
pub const BTN_H: f32 = 48.0;
pub const BTN_PAD: f32 = 4.0;
pub const TOOLBAR_BOTTOM_MARGIN: f32 = 16.0;

const TOOLBAR_BG_COLOR: [f32; 4] = palette::GRAY_0;
const TOOLBAR_HOVER_COLOR: [f32; 4] = palette::GRAY_1;
const TOOLBAR_ACTIVE_COLOR: [f32; 4] = palette::GRAY_2;
const TOOLBAR_ACTIVE_HOVER_COLOR: [f32; 4] = palette::BLUE_GRAY;
const TOOLBAR_BORDER_HIGHLIGHT: [f32; 4] = [239.0 / 255.0, 239.0 / 255.0, 239.0 / 255.0, 1.0];
const TOOLBAR_BORDER_SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const TOOLBAR_ICON_COLOR: [f32; 4] = palette::BLACK;
const TOOLBAR_ICON_SIZE: f32 = 32.0;
const TOOLBAR_LABEL_FONT_SIZE: f32 = 12.0;
const TOOLBAR_ICON_BYTES: [&[u8]; 9] = [
    include_bytes!("../../assets/toolbar/select.png"),
    include_bytes!("../../assets/toolbar/rect.png"),
    include_bytes!("../../assets/toolbar/ellipse.png"),
    include_bytes!("../../assets/toolbar/line.png"),
    include_bytes!("../../assets/toolbar/sticky.png"),
    include_bytes!("../../assets/toolbar/text.png"),
    include_bytes!("../../assets/toolbar/image.png"),
    include_bytes!("../../assets/toolbar/load.png"),
    include_bytes!("../../assets/toolbar/save.png"),
];

const ICON_SELECT: usize = 0;
const ICON_RECT: usize = 1;
const ICON_ELLIPSE: usize = 2;
const ICON_LINE: usize = 3;
const ICON_STICKY: usize = 4;
const ICON_TEXT: usize = 5;
const ICON_IMAGE: usize = 6;
const ICON_LOAD: usize = 7;
const ICON_SAVE: usize = 8;

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
    Sticky,
    Text,
    Image,
    Save,
    Load,
    Undo,
    Redo,
}

struct Button {
    kind: BtnKind,
    x: f32, // left edge in screen pixels
}

pub struct ToolbarIcons {
    atlas: TextureId,
    uv_rects: [[[f32; 2]; 2]; 9],
}

impl ToolbarIcons {
    pub fn new(ctx: &mut dyn RenderingBackend) -> Self {
        let (atlas, uv_rects) = load_toolbar_atlas(ctx, &TOOLBAR_ICON_BYTES);
        Self { atlas, uv_rects }
    }

    pub fn destroy(&self, ctx: &mut dyn RenderingBackend) {
        ctx.delete_texture(self.atlas);
    }

    fn atlas_texture(&self) -> TextureId {
        self.atlas
    }

    fn uv_for(&self, kind: BtnKind) -> Option<([f32; 2], [f32; 2])> {
        let index = match kind {
            BtnKind::Select => ICON_SELECT,
            BtnKind::Rect => ICON_RECT,
            BtnKind::Ellipse => ICON_ELLIPSE,
            BtnKind::Line => ICON_LINE,
            BtnKind::Sticky => ICON_STICKY,
            BtnKind::Text => ICON_TEXT,
            BtnKind::Image => ICON_IMAGE,
            BtnKind::Load => ICON_LOAD,
            BtnKind::Save => ICON_SAVE,
            BtnKind::Undo | BtnKind::Redo => return None,
        };
        let [uv_min, uv_max] = self.uv_rects[index];
        Some((uv_min, uv_max))
    }
}

pub struct Toolbar {
    pub active_tool: Tool,
    buttons: [Button; 11],
}

impl Toolbar {
    pub fn new() -> Self {
        let kinds = [
            BtnKind::Select,
            BtnKind::Rect,
            BtnKind::Ellipse,
            BtnKind::Line,
            BtnKind::Sticky,
            BtnKind::Text,
            BtnKind::Image,
            BtnKind::Load,
            BtnKind::Save,
            BtnKind::Undo,
            BtnKind::Redo,
        ];
        let buttons = std::array::from_fn(|i| Button {
            kind: kinds[i],
            x: BTN_PAD + i as f32 * (BTN_W + BTN_PAD),
        });
        Self {
            active_tool: Tool::Select,
            buttons,
        }
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
                    BtnKind::Select => ToolbarAction::SetTool(Tool::Select),
                    BtnKind::Rect => ToolbarAction::SetTool(Tool::Rect),
                    BtnKind::Ellipse => ToolbarAction::SetTool(Tool::Ellipse),
                    BtnKind::Line => ToolbarAction::SetTool(Tool::Line),
                    BtnKind::Sticky => ToolbarAction::SetTool(Tool::Sticky),
                    BtnKind::Text => ToolbarAction::SetTool(Tool::Text),
                    BtnKind::Image => ToolbarAction::ImportImage,
                    BtnKind::Load => ToolbarAction::Load,
                    BtnKind::Save => ToolbarAction::Save,
                    BtnKind::Undo => ToolbarAction::Undo,
                    BtnKind::Redo => ToolbarAction::Redo,
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
            false,
        ));
        out.push(InstanceData::new(
            layout.origin.to_array(),
            [layout.size.x, 1.0],
            0.0,
            TOOLBAR_BORDER_HIGHLIGHT,
            0.0,
            1.0,
            false,
        ));
        out.push(InstanceData::new(
            layout.origin.to_array(),
            [1.0, layout.size.y],
            0.0,
            TOOLBAR_BORDER_HIGHLIGHT,
            0.0,
            1.0,
            false,
        ));
        out.push(InstanceData::new(
            [layout.origin.x, layout.origin.y + layout.size.y - 1.0],
            [layout.size.x, 1.0],
            0.0,
            TOOLBAR_BORDER_SHADOW,
            0.0,
            1.0,
            false,
        ));
        out.push(InstanceData::new(
            [layout.origin.x + layout.size.x - 1.0, layout.origin.y],
            [1.0, layout.size.y],
            0.0,
            TOOLBAR_BORDER_SHADOW,
            0.0,
            1.0,
            false,
        ));

        for btn in &self.buttons {
            let is_active = matches!(
                (&btn.kind, self.active_tool),
                (BtnKind::Select, Tool::Select)
                    | (BtnKind::Rect, Tool::Rect)
                    | (BtnKind::Ellipse, Tool::Ellipse)
                    | (BtnKind::Line, Tool::Line)
                    | (BtnKind::Sticky, Tool::Sticky)
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
                    false,
                ));
            }
        }
        out
    }

    pub fn build_text_specs(
        &self,
        screen_size: Vec2,
        mouse_pos: Vec2,
        can_undo: bool,
        can_redo: bool,
    ) -> Vec<UiTextSpec> {
        let layout = self.layout(screen_size);
        let hovered_action = self.hovered_action(screen_size, mouse_pos.x, mouse_pos.y);
        let mut out = Vec::new();

        for btn in &self.buttons {
            let label = match btn.kind {
                BtnKind::Undo => "UNDO",
                BtnKind::Redo => "REDO",
                _ => continue,
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
            let color = [
                TOOLBAR_ICON_COLOR[0],
                TOOLBAR_ICON_COLOR[1],
                TOOLBAR_ICON_COLOR[2],
                icon_alpha,
            ];
            let cx = layout.origin.x + btn.x + BTN_W * 0.5;
            let cy = layout.origin.y + BTN_H * 0.5;

            out.push(
                UiTextSpec::top_center(
                    label,
                    Vec2::new(cx, cy - 6.0),
                    TOOLBAR_LABEL_FONT_SIZE,
                    color,
                )
                .with_line_height(TOOLBAR_LABEL_FONT_SIZE),
            );
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
        let atlas = icons.atlas_texture();

        for btn in &self.buttons {
            let Some((uv_min, uv_max)) = icons.uv_for(btn.kind) else {
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
                texture: atlas,
                instance: ImageInstanceData::new(
                    [origin_x, origin_y],
                    [TOOLBAR_ICON_SIZE, TOOLBAR_ICON_SIZE],
                    [origin_x, origin_y],
                    0.0,
                    uv_min,
                    uv_max,
                    tint,
                    false,
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
            | (BtnKind::Sticky, ToolbarAction::SetTool(Tool::Sticky))
            | (BtnKind::Text, ToolbarAction::SetTool(Tool::Text))
            | (BtnKind::Image, ToolbarAction::ImportImage)
            | (BtnKind::Load, ToolbarAction::Load)
            | (BtnKind::Save, ToolbarAction::Save)
            | (BtnKind::Undo, ToolbarAction::Undo)
            | (BtnKind::Redo, ToolbarAction::Redo)
    )
}

fn load_toolbar_atlas(
    ctx: &mut dyn RenderingBackend,
    icon_bytes: &[&[u8]],
) -> (TextureId, [[[f32; 2]; 2]; 9]) {
    debug_assert_eq!(icon_bytes.len(), 9, "toolbar atlas table must stay in sync");

    let decoded: Vec<_> = icon_bytes
        .iter()
        .map(|bytes| {
            image::load_from_memory(bytes)
                .expect("toolbar icon should decode")
                .to_rgba8()
        })
        .collect();

    let (icon_width, icon_height) = decoded[0].dimensions();
    debug_assert_eq!(icon_width, icon_height, "toolbar icons should be square");
    for image in &decoded[1..] {
        let dims = image.dimensions();
        assert_eq!(
            dims,
            (icon_width, icon_height),
            "toolbar icons should share dimensions"
        );
    }

    let atlas_width = icon_width * decoded.len() as u32;
    let atlas_height = icon_height;
    let mut atlas_pixels = vec![0u8; atlas_width as usize * atlas_height as usize * 4];
    let mut uv_rects = [[[0.0; 2]; 2]; 9];

    for (index, image) in decoded.iter().enumerate() {
        let x_offset = index * icon_width as usize;
        let row_bytes = icon_width as usize * 4;
        for row in 0..icon_height as usize {
            let src_start = row * row_bytes;
            let dst_start = (row * atlas_width as usize + x_offset) * 4;
            atlas_pixels[dst_start..dst_start + row_bytes]
                .copy_from_slice(&image.as_raw()[src_start..src_start + row_bytes]);
        }

        let uv_min_x = x_offset as f32 / atlas_width as f32;
        let uv_max_x = (x_offset + icon_width as usize) as f32 / atlas_width as f32;
        uv_rects[index] = [[uv_min_x, 0.0], [uv_max_x, 1.0]];
    }

    let texture = ctx.new_texture(
        TextureAccess::Static,
        TextureSource::Bytes(&atlas_pixels),
        TextureParams {
            width: atlas_width,
            height: atlas_height,
            format: TextureFormat::RGBA8,
            wrap: TextureWrap::Clamp,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::None,
            allocate_mipmaps: false,
            ..Default::default()
        },
    );

    (texture, uv_rects)
}

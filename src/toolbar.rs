use glam::Vec2;
use crate::renderer::InstanceData;
use crate::stats::emit_text;

pub const TOOLBAR_HEIGHT: f32 = 48.0;
pub const BTN_W: f32 = 52.0;
pub const BTN_H: f32 = 48.0;
pub const BTN_PAD: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Line,
    Text,
}

#[derive(Clone, Copy, Debug)]
pub enum ToolbarAction {
    SetTool(Tool),
    Save,
    Load,
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug)]
enum BtnKind {
    Select,
    Rect,
    Ellipse,
    Line,
    Text,
    Save,
    Load,
    Undo,
    Redo,
}

struct Button {
    kind: BtnKind,
    x: f32,  // left edge in screen pixels
}

pub struct Toolbar {
    pub active_tool: Tool,
    buttons: [Button; 9],
}

impl Toolbar {
    pub fn new() -> Self {
        let kinds = [
            BtnKind::Select,
            BtnKind::Rect,
            BtnKind::Ellipse,
            BtnKind::Line,
            BtnKind::Text,
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

    pub fn hit_test(&self, x: f32, y: f32) -> Option<ToolbarAction> {
        if y < 0.0 || y >= TOOLBAR_HEIGHT {
            return None;
        }
        for btn in &self.buttons {
            if x >= btn.x && x < btn.x + BTN_W {
                return Some(match btn.kind {
                    BtnKind::Select  => ToolbarAction::SetTool(Tool::Select),
                    BtnKind::Rect    => ToolbarAction::SetTool(Tool::Rect),
                    BtnKind::Ellipse => ToolbarAction::SetTool(Tool::Ellipse),
                    BtnKind::Line    => ToolbarAction::SetTool(Tool::Line),
                    BtnKind::Text    => ToolbarAction::SetTool(Tool::Text),
                    BtnKind::Save    => ToolbarAction::Save,
                    BtnKind::Load    => ToolbarAction::Load,
                    BtnKind::Undo    => ToolbarAction::Undo,
                    BtnKind::Redo    => ToolbarAction::Redo,
                });
            }
        }
        None
    }

    /// Build screen-space instance data for the toolbar background, buttons,
    /// and icons.  `screen_w` is used to draw the full-width background bar.
    pub fn build_instances(
        &self,
        screen_w: f32,
        can_undo: bool,
        can_redo: bool,
    ) -> Vec<InstanceData> {
        let mut out: Vec<InstanceData> = Vec::new();

        // Full toolbar background
        out.push(InstanceData::new(
            [0.0, 0.0],
            [screen_w, TOOLBAR_HEIGHT],
            0.0,
            [0.13, 0.14, 0.18, 1.0],
            0.0,
            1.0,
        ));

        // Separator line at bottom
        out.push(InstanceData::new(
            [0.0, TOOLBAR_HEIGHT - 1.0],
            [screen_w, 1.0],
            0.0,
            [0.25, 0.26, 0.30, 1.0],
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

            // Button background (highlight if active)
            if is_active {
                out.push(InstanceData::new(
                    [btn.x + 2.0, 4.0],
                    [BTN_W - 4.0, BTN_H - 8.0],
                    0.0,
                    [0.25, 0.48, 0.82, 0.35],
                    0.0,
                    1.0,
                ));
            }

            let icon_alpha = if dimmed { 0.3 } else { 0.9 };
            let icon_color = [0.85f32, 0.87, 0.90, icon_alpha];
            let cx = btn.x + BTN_W * 0.5;
            let cy = BTN_H * 0.5;

            // Text label: 3×5 bitmap font, scale=2 → glyph is 6px wide, 10px tall
            // stride per char = (3+1)*2 = 8px
            const SCALE: f32 = 2.0;
            const CHAR_W: f32 = 3.0 * SCALE; // 6
            const GAP: f32 = SCALE;           // 2
            const STRIDE: f32 = CHAR_W + GAP; // 8
            const GLYPH_H: f32 = 5.0 * SCALE; // 10

            let label = match btn.kind {
                BtnKind::Select  => "SEL",
                BtnKind::Rect    => "RECT",
                BtnKind::Ellipse => "ELPS",
                BtnKind::Line    => "LINE",
                BtnKind::Text    => "TEXT",
                BtnKind::Undo    => "UNDO",
                BtnKind::Redo    => "REDO",
                BtnKind::Save    => "SAVE",
                BtnKind::Load    => "LOAD",
            };

            let text_w = label.len() as f32 * STRIDE - GAP;
            let tx = cx - text_w * 0.5;
            let ty = cy - GLYPH_H * 0.5;
            emit_text(label, tx, ty, SCALE, icon_color, &mut out);
        }
        out
    }
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
    alpha: f32,
) -> Option<InstanceData> {
    if !e.selected {
        return None;
    }

    Some(outline_instance(e, alpha))
}

fn outline_instance(
    e: &crate::board::Element,
    alpha: f32,
) -> InstanceData {
    let (st, expand) = match e.shape {
        crate::board::ShapeType::Rect | crate::board::ShapeType::Text => (3.0f32, 4.0f32),
        crate::board::ShapeType::Ellipse => (4.0, 4.0),
        crate::board::ShapeType::Line => (2.0, 3.0),
    };

    InstanceData::new(
        (e.pos - Vec2::splat(expand)).to_array(),
        (e.size + Vec2::splat(expand * 2.0)).to_array(),
        e.rotation,
        [0.35, 0.65, 1.0, 1.0],
        st,
        alpha,
    )
}

pub fn preview_instances(
    e: &crate::board::Element,
    alpha: f32,
) -> Vec<InstanceData> {
    let mut instances = element_to_instances(e, alpha);

    if e.shape != crate::board::ShapeType::Line {
        instances.push(outline_instance(e, 1.0));
    }

    instances
}

pub fn element_to_instances(
    e: &crate::board::Element,
    alpha: f32,
) -> Vec<InstanceData> {
    vec![element_instance(e, alpha)]
}

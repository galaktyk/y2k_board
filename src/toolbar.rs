use glam::Vec2;
use crate::renderer::InstanceData;

pub const TOOLBAR_HEIGHT: f32 = 48.0;
pub const BTN_W: f32 = 48.0;
pub const BTN_H: f32 = 48.0;
pub const BTN_PAD: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Line,
}

#[derive(Clone, Copy, Debug)]
pub enum ToolbarAction {
    SetTool(Tool),
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug)]
enum BtnKind {
    Select,
    Rect,
    Ellipse,
    Line,
    Undo,
    Redo,
}

struct Button {
    kind: BtnKind,
    x: f32,  // left edge in screen pixels
}

pub struct Toolbar {
    pub active_tool: Tool,
    buttons: [Button; 6],
}

impl Toolbar {
    pub fn new() -> Self {
        let kinds = [
            BtnKind::Select,
            BtnKind::Rect,
            BtnKind::Ellipse,
            BtnKind::Line,
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
        out.push(InstanceData {
            pos: [0.0, 0.0],
            size: [screen_w, TOOLBAR_HEIGHT],
            color: [0.13, 0.14, 0.18, 1.0],
            shape_type: 0.0,
            alpha: 1.0,
        });

        // Separator line at bottom
        out.push(InstanceData {
            pos: [0.0, TOOLBAR_HEIGHT - 1.0],
            size: [screen_w, 1.0],
            color: [0.25, 0.26, 0.30, 1.0],
            shape_type: 0.0,
            alpha: 1.0,
        });

        for btn in &self.buttons {
            let is_active = matches!(
                (&btn.kind, self.active_tool),
                (BtnKind::Select, Tool::Select)
                | (BtnKind::Rect, Tool::Rect)
                | (BtnKind::Ellipse, Tool::Ellipse)
                | (BtnKind::Line, Tool::Line)
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
                out.push(InstanceData {
                    pos: [btn.x + 2.0, 4.0],
                    size: [BTN_W - 4.0, BTN_H - 8.0],
                    color: [0.25, 0.48, 0.82, 0.35],
                    shape_type: 0.0,
                    alpha: 1.0,
                });
            }

            let icon_alpha = if dimmed { 0.3 } else { 0.9 };
            let icon_color = [0.85f32, 0.87, 0.90, icon_alpha];
            let cx = btn.x + BTN_W * 0.5;
            let cy = BTN_H * 0.5;

            match btn.kind {
                BtnKind::Select => {
                    // Cursor: a filled triangle pointing down-right, made of 2 thin rects
                    let s = 14.0f32;
                    // vertical stroke
                    push_line(&mut out, [cx - 2.0, cy - s * 0.5], [cx - 2.0, cy + s * 0.5], 2.5, icon_color);
                    // diagonal stroke
                    push_line(&mut out, [cx - 2.0, cy + s * 0.5], [cx + s * 0.4, cy + s * 0.05], 2.5, icon_color);
                    push_line(&mut out, [cx - 2.0, cy - s * 0.5], [cx + s * 0.4, cy + s * 0.05], 2.5, icon_color);
                }
                BtnKind::Rect => {
                    let hw = 10.0f32;
                    let hh = 7.0f32;
                    // outline: 4 thin rectangles
                    push_line(&mut out, [cx - hw, cy - hh], [cx + hw, cy - hh], 2.0, icon_color); // top
                    push_line(&mut out, [cx - hw, cy + hh], [cx + hw, cy + hh], 2.0, icon_color); // bottom
                    push_line(&mut out, [cx - hw, cy - hh], [cx - hw, cy + hh], 2.0, icon_color); // left
                    push_line(&mut out, [cx + hw, cy - hh], [cx + hw, cy + hh], 2.0, icon_color); // right
                }
                BtnKind::Ellipse => {
                    let s = 18.0f32;
                    out.push(InstanceData {
                        pos: [cx - s * 0.5, cy - s * 0.4],
                        size: [s, s * 0.8],
                        color: icon_color,
                        shape_type: 1.0,
                        alpha: 1.0,
                    });
                    // cut out the inside (dark ellipse slightly smaller)
                    out.push(InstanceData {
                        pos: [cx - s * 0.5 + 3.0, cy - s * 0.4 + 3.0],
                        size: [s - 6.0, s * 0.8 - 6.0],
                        color: if is_active { [0.07, 0.09, 0.13, 1.0] } else { [0.13, 0.14, 0.18, 1.0] },
                        shape_type: 1.0,
                        alpha: 1.0,
                    });
                }
                BtnKind::Line => {
                    let s = 12.0f32;
                    push_line(&mut out, [cx - s, cy + s * 0.5], [cx + s, cy - s * 0.5], 2.5, icon_color);
                }
                BtnKind::Undo => {
                    // Left-pointing arc approximated with 3 line segments
                    let s = 8.0f32;
                    push_line(&mut out, [cx,       cy - s], [cx - s,   cy - s * 0.1], 2.0, icon_color);
                    push_line(&mut out, [cx - s,   cy - s * 0.1], [cx - s * 0.3, cy + s], 2.0, icon_color);
                    push_line(&mut out, [cx,       cy - s], [cx + s * 0.6, cy - s * 0.4], 2.0, icon_color);
                }
                BtnKind::Redo => {
                    let s = 8.0f32;
                    push_line(&mut out, [cx,       cy - s], [cx + s,   cy - s * 0.1], 2.0, icon_color);
                    push_line(&mut out, [cx + s,   cy - s * 0.1], [cx + s * 0.3, cy + s], 2.0, icon_color);
                    push_line(&mut out, [cx,       cy - s], [cx - s * 0.6, cy - s * 0.4], 2.0, icon_color);
                }
            }
        }
        out
    }
}

// ── Helper: encode a line segment as a tightly-fit instanced rect ─────────────

fn push_line(out: &mut Vec<InstanceData>, a: [f32; 2], b: [f32; 2], thickness: f32, color: [f32; 4]) {
    let ax = a[0]; let ay = a[1];
    let bx = b[0]; let by = b[1];
    let dx = bx - ax; let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt().max(0.001);

    // The instance rect spans from a to b; we use the LINE shape shader.
    // pos = min corner of AABB, size = (length, thickness)
    // But the shader expects pos to be the start of the segment and size = (dx, dy)
    // Actually, for line shape_type=2 the shader uses uv.x along the segment.
    // We pass: pos = start, size = (dx, dy), and the bounding quad is computed
    // by the vertex shader which expands by thickness on both sides.
    //
    // For simplicity in the current shader design (which just draws a horizontal
    // bar and ignores rotation), we instead use many thin rects aligned per segment.
    // We'll draw a thin rotated rect using the rect shader by computing the AABB
    // and accepting a bit of overdraw.
    //
    // REAL approach: encode line start/end, then clip in fragment shader.
    // We approximate here with a 2px-wide AABB rect for toolbar icons.
    let _len = len; // suppress warning
    let min_x = ax.min(bx) - thickness * 0.5;
    let min_y = ay.min(by) - thickness * 0.5;
    let max_x = ax.max(bx) + thickness * 0.5;
    let max_y = ay.max(by) + thickness * 0.5;

    out.push(InstanceData {
        pos: [min_x, min_y],
        size: [max_x - min_x, max_y - min_y],
        color,
        shape_type: 0.0,  // rect for icon strokes (good enough at small scale)
        alpha: 1.0,
    });
}

/// Convert a world-space element into one or more InstanceData entries.
/// `selected` adds a highlight border.
pub fn element_to_instances(
    e: &crate::board::Element,
    alpha: f32,
) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let st = match e.shape {
        crate::board::ShapeType::Rect    => 0.0,
        crate::board::ShapeType::Ellipse => 1.0,
        crate::board::ShapeType::Line    => 2.0,
    };

    out.push(InstanceData {
        pos:        e.pos.to_array(),
        size:       e.size.to_array(),
        color:      e.color,
        shape_type: st,
        alpha,
    });

    // Selection highlight: a slightly larger, blue-ish semi-transparent copy
    if e.selected {
        let expand = 3.0f32;
        let sel_color = [0.25, 0.55, 1.0, 0.45];
        out.push(InstanceData {
            pos:  (e.pos - Vec2::splat(expand)).to_array(),
            size: (e.size + Vec2::splat(expand * 2.0)).to_array(),
            color: sel_color,
            shape_type: st,
            alpha,
        });
    }

    out
}

use glam::Vec2;
use serde::{Deserialize, Serialize};
use crate::palette;

use crate::input::SelectionBounds;

mod geometry;

#[cfg(test)]
mod tests;

use geometry::element_hit;
pub use geometry::{rotate_point, world_to_local_norm};

// ── Types ────────────────────────────────────────────────────────────────────

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
pub const DEFAULT_LINE_COLOR: [f32; 4] = palette::RED;
pub const DEFAULT_STROKE_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
pub const DEFAULT_BORDER_WIDTH: u8 = 1;
pub const DEFAULT_LINE_STROKE_WIDTH: u8 = 1;
pub const DEFAULT_BOX_STROKE_COLOR: [f32; 4] = palette::BLACK;

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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineToolStyle {
    pub color: [f32; 4],
    pub stroke_width: u8,
}

impl LineToolStyle {
    pub fn default_line() -> Self {
        Self {
            color: DEFAULT_LINE_COLOR,
            stroke_width: DEFAULT_LINE_STROKE_WIDTH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolStyleDefaults {
    pub rect: BoxToolStyle,
    pub ellipse: BoxToolStyle,
    pub text: BoxToolStyle,
    pub line: LineToolStyle,
}

impl Default for ToolStyleDefaults {
    fn default() -> Self {
        Self {
            rect: BoxToolStyle::rect_default(),
            ellipse: BoxToolStyle::ellipse_default(),
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
    /// World-space top-left for Rect/Ellipse; start point for Line.
    pub pos: Vec2,
    /// (width, height) for Rect/Ellipse; (dx, dy) end-delta for Line.
    pub size: Vec2,
    pub rotation: f32,
    pub color: [f32; 4],
    #[serde(default = "default_stroke_color")]
    pub stroke_color: [f32; 4],
    #[serde(default = "default_border_width")]
    pub border_width: u8,
    #[serde(default = "default_line_stroke_width")]
    pub stroke_width: u8,
    pub selected: bool,
    #[serde(default)]
    pub text: Option<TextData>,
    #[serde(default)]
    pub image: Option<ImageData>,
    /// Bumped when any text-layout-affecting property changes.
    /// Not serialized — starts at 0 on load.
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

    /// Axis-aligned bounding box for spatial queries.
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

    /// Bump the text layout generation counter, invalidating cached layouts.
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

// ── Operations ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementTransform {
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}

impl ElementTransform {
    pub fn new(pos: Vec2, size: Vec2, rotation: f32) -> Self {
        Self { pos, size, rotation }
    }
}

fn apply_transform(element: &mut Element, before: Option<ElementTransform>, after: ElementTransform) {
    let size_changed = before.map_or(element.size != after.size, |b| b.size != after.size);
    element.pos = after.pos;
    element.size = after.size;
    element.rotation = after.rotation;
    if size_changed {
        element.bump_text_generation();
    }
}

fn move_element(element: &mut Element, delta: Vec2) {
    element.pos += delta;
}

fn rotate_element(element: &mut Element, center: Vec2, angle: f32) {
    if element.shape == ShapeType::Line {
        let start = rotate_point(element.pos, center, angle);
        let end = rotate_point(element.pos + element.size, center, angle);
        element.pos = start;
        element.size = end - start;
    } else {
        let element_center = element.pos + element.size * 0.5;
        let rotated_center = rotate_point(element_center, center, angle);
        element.pos = rotated_center - element.size * 0.5;
        element.rotation += angle;
    }
}

#[derive(Clone, Debug)]
pub enum ElementPropertyPatch {
    Transform {
        before: ElementTransform,
        after: ElementTransform,
    },
    Style {
        before: ElementStyleSnapshot,
        after: ElementStyleSnapshot,
    },
    Text {
        before: Option<TextData>,
        after: Option<TextData>,
    },
}

#[derive(Clone, Debug)]
pub struct ElementPropertyChange {
    pub id: u64,
    pub patch: ElementPropertyPatch,
}

#[derive(Clone, Copy, Debug)]
pub struct ElementRotationChange {
    pub id: u64,
    pub before: f32,
    pub after: f32,
}

#[derive(Clone, Debug)]
pub struct LineConnectionChange {
    pub id: u64,
    pub before: LineEndpoints,
    pub after: LineEndpoints,
}

#[derive(Clone, Debug)]
pub enum BoardOperation {
    AddElement(Element),
    DeleteElement(Element),
    MoveElements { ids: Vec<u64>, delta: Vec2 },
    RotateElements { ids: Vec<u64>, center: Vec2, angle: f32 },
    SetElementRotations { changes: Vec<ElementRotationChange> },
    SetProperty {
        changes: Vec<ElementPropertyChange>,
        sync_connected_lines: bool,
    },
    SetLineConnections { changes: Vec<LineConnectionChange> },
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    undo: BoardOperation,
    redo: BoardOperation,
}

fn inverse(op: &BoardOperation) -> BoardOperation {
    match op {
        BoardOperation::AddElement(element) => BoardOperation::DeleteElement(element.clone()),
        BoardOperation::DeleteElement(element) => BoardOperation::AddElement(element.clone()),
        BoardOperation::MoveElements { ids, delta } => BoardOperation::MoveElements {
            ids: ids.clone(),
            delta: -*delta,
        },
        BoardOperation::RotateElements { ids, center, angle } => BoardOperation::RotateElements {
            ids: ids.clone(),
            center: *center,
            angle: -*angle,
        },
        BoardOperation::SetElementRotations { changes } => BoardOperation::SetElementRotations {
            changes: changes
                .iter()
                .map(|change| ElementRotationChange {
                    id: change.id,
                    before: change.after,
                    after: change.before,
                })
                .collect(),
        },
        BoardOperation::SetProperty {
            changes,
            sync_connected_lines,
        } => BoardOperation::SetProperty {
            changes: changes
                .iter()
                .map(|change| ElementPropertyChange {
                    id: change.id,
                    patch: match &change.patch {
                        ElementPropertyPatch::Transform { before, after } => {
                            ElementPropertyPatch::Transform {
                                before: *after,
                                after: *before,
                            }
                        }
                        ElementPropertyPatch::Style { before, after } => {
                            ElementPropertyPatch::Style {
                                before: *after,
                                after: *before,
                            }
                        }
                        ElementPropertyPatch::Text { before, after } => ElementPropertyPatch::Text {
                            before: after.clone(),
                            after: before.clone(),
                        },
                    },
                })
                .collect(),
            sync_connected_lines: *sync_connected_lines,
        },
        BoardOperation::SetLineConnections { changes } => BoardOperation::SetLineConnections {
            changes: changes
                .iter()
                .map(|change| LineConnectionChange {
                    id: change.id,
                    before: change.after.clone(),
                    after: change.before.clone(),
                })
                .collect(),
        },
    }
}

fn log_operation(op: &BoardOperation) {
    match op {
        BoardOperation::AddElement(element) => {
            println!(
                "[ops] ADD_ELEMENT id={} shape={:?} pos=({:.1}, {:.1}) size=({:.1}, {:.1})",
                element.id,
                element.shape,
                element.pos.x,
                element.pos.y,
                element.size.x,
                element.size.y,
            );
        }
        BoardOperation::DeleteElement(element) => {
            println!("[ops] DELETE_ELEMENT id={} shape={:?}", element.id, element.shape);
        }
        BoardOperation::MoveElements { ids, delta } => {
            println!(
                "[ops] MOVE_ELEMENTS count={} delta=({:.1}, {:.1})",
                ids.len(),
                delta.x,
                delta.y,
            );
        }
        BoardOperation::RotateElements { ids, center, angle } => {
            println!(
                "[ops] ROTATE_ELEMENTS count={} center=({:.1}, {:.1}) angle={:.3}",
                ids.len(),
                center.x,
                center.y,
                angle,
            );
        }
        BoardOperation::SetElementRotations { changes } => {
            println!("[ops] SET_ELEMENT_ROTATIONS count={}", changes.len());
            for change in changes {
                println!(
                    "[ops]   id={} rot={:.3}->{:.3}",
                    change.id,
                    change.before,
                    change.after,
                );
            }
        }
        BoardOperation::SetProperty {
            changes,
            sync_connected_lines,
        } => {
            println!(
                "[ops] SET_PROPERTY count={} sync_connected_lines={}",
                changes.len(),
                sync_connected_lines,
            );
            for change in changes {
                match &change.patch {
                    ElementPropertyPatch::Transform { before, after } => {
                        println!(
                            "[ops]   id={} transform pos=({:.1}, {:.1})->({:.1}, {:.1}) size=({:.1}, {:.1})->({:.1}, {:.1}) rot={:.3}->{:.3}",
                            change.id,
                            before.pos.x,
                            before.pos.y,
                            after.pos.x,
                            after.pos.y,
                            before.size.x,
                            before.size.y,
                            after.size.x,
                            after.size.y,
                            before.rotation,
                            after.rotation,
                        );
                    }
                    ElementPropertyPatch::Style { before, after } => {
                        println!(
                            "[ops]   id={} style fill={:?}->{:?} stroke={:?}->{:?} border={:?}->{:?} line={:?}->{:?} text={:?}->{:?}",
                            change.id,
                            before.fill_color,
                            after.fill_color,
                            before.stroke_color,
                            after.stroke_color,
                            before.border_width,
                            after.border_width,
                            before.stroke_width,
                            after.stroke_width,
                            before.text_color,
                            after.text_color,
                        );
                    }
                    ElementPropertyPatch::Text { before, after } => {
                        println!(
                            "[ops]   id={} text len={} -> {}",
                            change.id,
                            before.as_ref().map(|text| text.content.chars().count()).unwrap_or(0),
                            after.as_ref().map(|text| text.content.chars().count()).unwrap_or(0),
                        );
                    }
                }
            }
        }
        BoardOperation::SetLineConnections { changes } => {
            println!("[ops] SET_LINE_CONNECTIONS count={}", changes.len());
            for change in changes {
                println!(
                    "[ops]   id={} start: id={:?} -> {:?} end: id={:?} -> {:?}",
                    change.id,
                    change.before.start.as_ref().map(|s| s.target_id),
                    change.after.start.as_ref().map(|s| s.target_id),
                    change.before.end.as_ref().map(|e| e.target_id),
                    change.after.end.as_ref().map(|e| e.target_id),
                );
            }
        }
    }
}

// ── Board ────────────────────────────────────────────────────────────────────

pub struct Board {
    pub elements: Vec<Element>,
    pub line_attachments: std::collections::HashMap<u64, LineEndpoints>,
    connected_lines: std::collections::HashMap<u64, Vec<u64>>,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    emitted_ops: Vec<BoardOperation>,
    next_id: u64,
}

impl Board {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            line_attachments: std::collections::HashMap::new(),
            connected_lines: std::collections::HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            emitted_ops: Vec::new(),
            next_id: 1,
        }
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn next_available_id(&self) -> u64 {
        self.next_id
    }

    pub fn apply_operation(&mut self, op: BoardOperation) {
        let entry = HistoryEntry {
            undo: inverse(&op),
            redo: op.clone(),
        };
        self.execute(&op);
        self.undo_stack.push(entry);
        self.redo_stack.clear();
        log_operation(&op);
        self.emitted_ops.push(op);
    }

    pub fn insert_element_untracked(&mut self, element: Element) {
        self.execute(&BoardOperation::AddElement(element));
    }

    pub fn restore_snapshot(&mut self, data: crate::snapshot::SnapshotData) {
        let mut elements = data.elements;
        for element in &mut elements {
            element.selected = false;
        }
        self.elements = elements;
        self.line_attachments = data.line_attachments;
        
        let mut connected_lines = std::collections::HashMap::new();
        for (line_id, endpoints) in &self.line_attachments {
            if let Some(start) = &endpoints.start {
                connected_lines.entry(start.target_id).or_insert_with(Vec::new).push(*line_id);
            }
            if let Some(end) = &endpoints.end {
                connected_lines.entry(end.target_id).or_insert_with(Vec::new).push(*line_id);
            }
        }
        self.connected_lines = connected_lines;
        
        self.next_id = data.next_id.max(1);
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.emitted_ops.clear();
    }

    pub fn update_connected_lines(&mut self, target_id: u64) {

        println!("[HOT] Updating connected lines for target_id={target_id}");
        
        self.update_connected_lines_filtered(target_id, None);
    }

    pub fn update_connected_lines_filtered(
        &mut self,
        target_id: u64,
        visible_ids: Option<&std::collections::HashSet<u64>>,
    ) {
        // This walk can touch every line anchored to the target, so it is one of the
        // more expensive board-side transform paths. Keep drag preview on GPU offsets
        // where possible and reserve this for commit-time updates or targeted refreshes.
        let (target_pos, target_size, target_rotation) = if let Some(target) = self.element(target_id) {
            (target.pos, target.size, target.rotation)
        } else {
            return;
        };

        let line_ids = self.connected_lines.get(&target_id).cloned().unwrap_or_default();

        for line_id in line_ids {
            if visible_ids.is_some_and(|visible| !visible.contains(&line_id)) {
                continue;
            }
            let endpoints = self.line_attachments.get(&line_id).cloned();
            if let Some(endpoints) = endpoints {
                if let Some(line) = self.element_mut(line_id) {
                    let mut start_pos = line.pos;
                    let mut end_pos = line.pos + line.size;

                    let origin = target_pos + target_size * 0.5;

                    if let Some(start) = &endpoints.start {
                        if start.target_id == target_id {
                            let local = (start.norm_pos - Vec2::splat(0.5)) * target_size;
                            start_pos = rotate_point(origin + local, origin, target_rotation);
                        }
                    }
                    if let Some(end) = &endpoints.end {
                        if end.target_id == target_id {
                            let local = (end.norm_pos - Vec2::splat(0.5)) * target_size;
                            end_pos = rotate_point(origin + local, origin, target_rotation);
                        }
                    }

                    line.pos = start_pos;
                    line.size = end_pos - start_pos;
                }
            }
        }
    }

    pub fn update_connected_lines_for_targets<I>(&mut self, target_ids: I)
    where
        I: IntoIterator<Item = u64>,
    {
        self.update_connected_lines_for_targets_filtered(target_ids, None);
    }

    pub fn update_connected_lines_for_targets_filtered<I>(
        &mut self,
        target_ids: I,
        visible_ids: Option<&std::collections::HashSet<u64>>,
    )
    where
        I: IntoIterator<Item = u64>,
    {
        // Potentially CPU-heavy for large selections: this deduplicates the input set and then
        // walks each target's connected lines. Avoid calling it from per-frame pointer updates.
        let mut unique_ids: Vec<u64> = target_ids.into_iter().collect();
        unique_ids.sort_unstable();
        unique_ids.dedup();

        for target_id in unique_ids {
            self.update_connected_lines_filtered(target_id, visible_ids);
        }
    }

    #[allow(dead_code)]
    pub fn transform_related_ids<I>(&self, ids: I) -> Vec<u64>
    where
        I: IntoIterator<Item = u64>,
    {
        self.transform_related_ids_filtered(ids, None)
    }

    pub fn transform_related_ids_filtered<I>(
        &self,
        ids: I,
        visible_ids: Option<&std::collections::HashSet<u64>>,
    ) -> Vec<u64>
    where
        I: IntoIterator<Item = u64>,
    {
        let mut related = std::collections::HashSet::new();

        for id in ids {
            related.insert(id);
            if let Some(line_ids) = self.connected_lines.get(&id) {
                related.extend(
                    line_ids
                        .iter()
                        .copied()
                        .filter(|line_id| visible_ids.is_none_or(|visible| visible.contains(line_id))),
                );
            }
        }

        let mut related: Vec<u64> = related.into_iter().collect();
        related.sort_unstable();
        related
    }

    #[allow(dead_code)]
    pub fn take_emitted_ops(&mut self) -> Vec<BoardOperation> {
        std::mem::take(&mut self.emitted_ops)
    }

    fn execute(&mut self, op: &BoardOperation) {
        match op {
            BoardOperation::AddElement(element) => {
                self.next_id = self.next_id.max(element.id.saturating_add(1));
                self.elements.retain(|existing| existing.id != element.id);
                self.elements.push(element.clone());
            }
            BoardOperation::DeleteElement(element) => {
                self.elements.retain(|existing| existing.id != element.id);
            }
            BoardOperation::MoveElements { ids, delta } => {
                for id in ids {
                    if let Some(element) = self.elements.iter_mut().find(|element| &element.id == id) {
                        move_element(element, *delta);
                    }
                    self.update_connected_lines(*id);
                }
            }
            BoardOperation::RotateElements { ids, center, angle } => {
                for id in ids {
                    if let Some(element) = self.elements.iter_mut().find(|element| &element.id == id) {
                        rotate_element(element, *center, *angle);
                    }
                    self.update_connected_lines(*id);
                }
            }
            BoardOperation::SetElementRotations { changes } => {
                for change in changes {
                    if let Some(element) = self.elements.iter_mut().find(|element| element.id == change.id)
                    {
                        element.rotation = change.after;
                    }
                    self.update_connected_lines(change.id);
                }
            }
            BoardOperation::SetProperty {
                changes,
                sync_connected_lines,
            } => {
                for change in changes {
                    let (before, after) = match &change.patch {
                        ElementPropertyPatch::Transform { before, after } => (Some(*before), *after),
                        ElementPropertyPatch::Style { .. } | ElementPropertyPatch::Text { .. } => continue,
                    };
                    if let Some(element) = self.elements.iter_mut().find(|e| e.id == change.id) {
                        apply_transform(element, before, after);
                    }
                    if *sync_connected_lines {
                        self.update_connected_lines(change.id);
                    }
                }
                for change in changes {
                    let after = match &change.patch {
                        ElementPropertyPatch::Style { after, .. } => after,
                        ElementPropertyPatch::Transform { .. } | ElementPropertyPatch::Text { .. } => continue,
                    };
                    if let Some(element) = self.elements.iter_mut().find(|e| e.id == change.id) {
                        element.apply_style_snapshot(*after);
                    }
                }
                for change in changes {
                    let after = match &change.patch {
                        ElementPropertyPatch::Text { after, .. } => after,
                        ElementPropertyPatch::Transform { .. } | ElementPropertyPatch::Style { .. } => continue,
                    };
                    if let Some(element) = self.elements.iter_mut().find(|e| e.id == change.id) {
                        element.text = after.clone();
                        element.bump_text_generation();
                    }
                }
            }
            BoardOperation::SetLineConnections { changes } => {
                let mut affected_targets = Vec::new();
                for change in changes {
                    if let Some(before_start) = &change.before.start {
                        affected_targets.push(before_start.target_id);
                        if let Some(lines) = self.connected_lines.get_mut(&before_start.target_id) {
                            lines.retain(|id| *id != change.id);
                        }
                    }
                    if let Some(before_end) = &change.before.end {
                        affected_targets.push(before_end.target_id);
                        if let Some(lines) = self.connected_lines.get_mut(&before_end.target_id) {
                            lines.retain(|id| *id != change.id);
                        }
                    }

                    if let Some(after_start) = &change.after.start {
                        affected_targets.push(after_start.target_id);
                        let lines = self
                            .connected_lines
                            .entry(after_start.target_id)
                            .or_insert_with(Vec::new);
                        if !lines.contains(&change.id) {
                            lines.push(change.id);
                        }
                    }
                    if let Some(after_end) = &change.after.end {
                        affected_targets.push(after_end.target_id);
                        let lines = self
                            .connected_lines
                            .entry(after_end.target_id)
                            .or_insert_with(Vec::new);
                        if !lines.contains(&change.id) {
                            lines.push(change.id);
                        }
                    }

                    if change.after.start.is_none() && change.after.end.is_none() {
                        self.line_attachments.remove(&change.id);
                    } else {
                        self.line_attachments.insert(change.id, change.after.clone());
                    }
                }

                self.update_connected_lines_for_targets(affected_targets);
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            self.execute(&entry.undo);
            self.redo_stack.push(entry);
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            self.execute(&entry.redo);
            self.undo_stack.push(entry);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn delete_selected(&mut self) {
        let selected: Vec<Element> = self.elements.iter().filter(|e| e.selected).cloned().collect();
        if selected.is_empty() {
            return;
        }

        let deleted_ids: std::collections::HashSet<u64> = selected.iter().map(|e| e.id).collect();
        let mut conn_changes = Vec::new();

        for (line_id, endpoints) in &self.line_attachments {
            let mut after = endpoints.clone();
            let is_line_deleted = deleted_ids.contains(line_id);

            if is_line_deleted {
                after.start = None;
                after.end = None;
            } else {
                if let Some(start) = &after.start {
                    if deleted_ids.contains(&start.target_id) {
                        after.start = None;
                    }
                }
                if let Some(end) = &after.end {
                    if deleted_ids.contains(&end.target_id) {
                        after.end = None;
                    }
                }
            }

            if after != *endpoints {
                conn_changes.push(LineConnectionChange {
                    id: *line_id,
                    before: endpoints.clone(),
                    after,
                });
            }
        }

        if !conn_changes.is_empty() {
            self.apply_operation(BoardOperation::SetLineConnections { changes: conn_changes });
        }

        for element in selected {
            self.apply_operation(BoardOperation::DeleteElement(element));
        }
    }

    #[allow(dead_code)]
    pub fn move_selected(&mut self, delta: Vec2) {
        let ids = self.selected_ids();
        if !ids.is_empty() && delta != Vec2::ZERO {
            self.apply_operation(BoardOperation::MoveElements { ids, delta });
        }
    }

    pub fn deselect_all(&mut self) {
        for element in &mut self.elements {
            element.selected = false;
        }
    }

    pub fn selected_ids(&self) -> Vec<u64> {
        self.elements
            .iter()
            .filter(|element| element.selected)
            .map(|element| element.id)
            .collect()
    }

    pub fn selected_count(&self) -> usize {
        self.elements.iter().filter(|element| element.selected).count()
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.elements
            .iter()
            .find(|element| element.id == id)
            .map(|element| element.selected)
            .unwrap_or(false)
    }

    pub fn toggle_selected(&mut self, id: u64) {
        if let Some(element) = self.elements.iter_mut().find(|element| element.id == id) {
            element.selected = !element.selected;
        }
    }

    pub fn bring_to_front(&mut self, id: u64) -> bool {
        if let Some(index) = self.elements.iter().position(|element| element.id == id) {
            if index < self.elements.len() - 1 {
                let element = self.elements.remove(index);
                self.elements.push(element);
                return true;
            }
        }
        false
    }

    pub fn send_to_back(&mut self, id: u64) -> bool {
        if let Some(index) = self.elements.iter().position(|element| element.id == id) {
            if index > 0 {
                let element = self.elements.remove(index);
                self.elements.insert(0, element);
                return true;
            }
        }
        false
    }

    // Keep the old one for compatibility just in case, or replace it if not used elsewhere.

    pub fn selected_bounds(&self) -> Option<SelectionBounds> {
        let mut bounds: Option<(Vec2, Vec2)> = None;

        for element in self.elements.iter().filter(|element| element.selected) {
            let (min, max) = element.aabb();
            bounds = Some(match bounds {
                Some((current_min, current_max)) => (current_min.min(min), current_max.max(max)),
                None => (min, max),
            });
        }

        bounds.map(|(min, max)| SelectionBounds::new(min, max - min))
    }

    pub fn select_intersecting_bounds(&mut self, bounds: SelectionBounds, additive: bool) {
        let min = bounds.min();
        let max = bounds.max();

        if !additive {
            self.deselect_all();
        }

        for element in &mut self.elements {
            let (element_min, element_max) = element.aabb();
            let intersects = element_min.x <= max.x
                && element_max.x >= min.x
                && element_min.y <= max.y
                && element_max.y >= min.y;
            if intersects {
                element.selected = true;
            }
        }
    }

    pub fn select_only(&mut self, id: u64) {
        for element in &mut self.elements {
            element.selected = element.id == id;
        }
    }

    pub fn selected_transform_changes(
        &self,
        originals: &[(u64, Vec2, Vec2, f32)],
    ) -> Vec<ElementPropertyChange> {
        self.elements
            .iter()
            .filter(|element| element.selected)
            .filter_map(|element| {
                originals
                    .iter()
                    .find(|&&(id, _, _, _)| id == element.id)
                    .and_then(|&(_, old_pos, old_size, old_rotation)| {
                        let before = ElementTransform::new(old_pos, old_size, old_rotation);
                        let after = ElementTransform::new(element.pos, element.size, element.rotation);
                        (before != after).then_some(ElementPropertyChange {
                            id: element.id,
                            patch: ElementPropertyPatch::Transform { before, after },
                        })
                    })
            })
            .collect()
    }

    /// Hit-test a world-space point against elements (last-on-top).
    pub fn hit_test(&self, p: Vec2) -> Option<u64> {
        // This is an O(n) scan over painter order. It is fine for user-driven pointer events,
        // but should not be moved into any per-frame update loop.
        // The two-pass walk is intentional: non-image shapes win over images at the same point.
        for element in self.elements.iter().rev() {
            if element.shape != ShapeType::Image && element_hit(element, p) {
                return Some(element.id);
            }
        }

        for element in self.elements.iter().rev() {
            if element.shape == ShapeType::Image && element_hit(element, p) {
                return Some(element.id);
            }
        }

        None
    }

    /// Hit-test a world-space point against elements, returning all hits in top-to-bottom order.
    pub fn hit_test_all(&self, p: Vec2) -> Vec<u64> {
        // This is also O(n) and intentionally preserves board order plus image-layer priority.
        // Use it only when the caller really needs the full hit stack.
        let mut hits = Vec::new();
        
        for element in self.elements.iter().rev() {
            if element.shape != ShapeType::Image && element_hit(element, p) {
                hits.push(element.id);
            }
        }

        for element in self.elements.iter().rev() {
            if element.shape == ShapeType::Image && element_hit(element, p) {
                hits.push(element.id);
            }
        }

        hits
    }

    pub fn element(&self, id: u64) -> Option<&Element> {
        self.elements.iter().find(|element| element.id == id)
    }

    pub fn element_mut(&mut self, id: u64) -> Option<&mut Element> {
        self.elements.iter_mut().find(|element| element.id == id)
    }
}

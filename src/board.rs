use glam::Vec2;
use serde::{Deserialize, Serialize};
use crate::palette;

use crate::input::SelectionBounds;

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

fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center + Vec2::new(
        offset.x * cos_a - offset.y * sin_a,
        offset.x * sin_a + offset.y * cos_a,
    )
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
pub enum BoardOperation {
    AddElement(Element),
    DeleteElement(Element),
    MoveElements { ids: Vec<u64>, delta: Vec2 },
    RotateElements { ids: Vec<u64>, center: Vec2, angle: f32 },
    SetElementRotations { changes: Vec<ElementRotationChange> },
    SetProperty { changes: Vec<ElementPropertyChange> },
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
        BoardOperation::SetProperty { changes } => BoardOperation::SetProperty {
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
        BoardOperation::SetProperty { changes } => {
            println!("[ops] SET_PROPERTY count={}", changes.len());
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
    }
}

// ── Board ────────────────────────────────────────────────────────────────────

pub struct Board {
    pub elements: Vec<Element>,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    emitted_ops: Vec<BoardOperation>,
    next_id: u64,
}

impl Board {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
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

    pub fn restore_snapshot(&mut self, mut elements: Vec<Element>, next_id: u64) {
        for element in &mut elements {
            element.selected = false;
        }
        self.elements = elements;
        self.next_id = next_id.max(1);
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.emitted_ops.clear();
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
                }
            }
            BoardOperation::RotateElements { ids, center, angle } => {
                for id in ids {
                    if let Some(element) = self.elements.iter_mut().find(|element| &element.id == id) {
                        rotate_element(element, *center, *angle);
                    }
                }
            }
            BoardOperation::SetElementRotations { changes } => {
                for change in changes {
                    if let Some(element) = self.elements.iter_mut().find(|element| element.id == change.id)
                    {
                        element.rotation = change.after;
                    }
                }
            }
            BoardOperation::SetProperty { changes } => {
                for change in changes {
                    let (before, after) = match &change.patch {
                        ElementPropertyPatch::Transform { before, after } => (Some(*before), *after),
                        ElementPropertyPatch::Style { .. } | ElementPropertyPatch::Text { .. } => continue,
                    };
                    if let Some(element) = self.elements.iter_mut().find(|e| e.id == change.id) {
                        apply_transform(element, before, after);
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

    pub fn bring_shape_to_front(&mut self, id: u64) -> bool {
        let Some(index) = self.elements.iter().position(|element| element.id == id) else {
            return false;
        };

        if self.elements[index].shape == ShapeType::Image {
            return false;
        }

        let mut current_index = index;
        let mut changed = false;

        while let Some(next_shape_index) = self
            .elements
            .iter()
            .enumerate()
            .skip(current_index + 1)
            .find_map(|(candidate_index, element)| {
                (element.shape != ShapeType::Image).then_some(candidate_index)
            })
        {
            self.elements.swap(current_index, next_shape_index);
            current_index = next_shape_index;
            changed = true;
        }

        if !changed {
            return false;
        }

        true
    }

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
        for element in self.elements.iter().rev() {
            if element.shape == ShapeType::Rect && element_hit(element, p) {
                return Some(element.id);
            }
        }

        for element in self.elements.iter().rev() {
            if element.shape == ShapeType::Image && element_hit(element, p) {
                return Some(element.id);
            }
        }

        for element in self.elements.iter().rev() {
            if element.shape != ShapeType::Image  && element_hit(element, p) {
                return Some(element.id);
            }
        }

        None
    }

    pub fn element(&self, id: u64) -> Option<&Element> {
        self.elements.iter().find(|element| element.id == id)
    }

    pub fn element_mut(&mut self, id: u64) -> Option<&mut Element> {
        self.elements.iter_mut().find(|element| element.id == id)
    }
}

fn element_hit(e: &Element, mut p: Vec2) -> bool {
    let center = e.pos + e.size * 0.5;
    let cos_r = e.rotation.cos();
    let sin_r = e.rotation.sin();
    let dx = p.x - center.x;
    let dy = p.y - center.y;
    p.x = center.x + dx * cos_r + dy * sin_r;
    p.y = center.y - dx * sin_r + dy * cos_r;

    match e.shape {
        ShapeType::Rect | ShapeType::Image => {
            let min_x = e.pos.x.min(e.pos.x + e.size.x);
            let max_x = e.pos.x.max(e.pos.x + e.size.x);
            let min_y = e.pos.y.min(e.pos.y + e.size.y);
            let max_y = e.pos.y.max(e.pos.y + e.size.y);
            p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
        }
        ShapeType::Ellipse => {
            let c = e.pos + e.size * 0.5;
            let r = (e.size * 0.5).abs();
            if r.x == 0.0 || r.y == 0.0 {
                return false;
            }
            let d = (p - c) / r;
            d.dot(d) <= 1.0
        }
        ShapeType::Line => {
            let a = e.pos;
            let b = e.pos + e.size;
            dist_point_segment(p, a, b) <= (f32::from(e.stroke_width.max(1)) * 0.5 + 8.0)
        }
    }
}

fn dist_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 == 0.0 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_bounds_keep_inner_box_centered() {
        let element = Element {
            id: 1,
            shape: ShapeType::Text,
            pos: Vec2::new(100.0, 50.0),
            size: Vec2::new(200.0, 120.0),
            rotation: 0.4,
            color: [0.0, 0.0, 0.0, 0.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            selected: false,
            text: Some(TextData::default()),
            image: None,
            text_layout_generation: 0,
        };

        let (min, max) = element.text_bounds().unwrap();
        let inner_center = (min + max) * 0.5;

        assert_eq!(min, Vec2::new(112.0, 62.0));
        assert_eq!(max, Vec2::new(288.0, 158.0));
        assert_eq!(inner_center, element.pos + element.size * 0.5);
    }

    #[test]
    fn bring_shape_to_front_keeps_images_after_shapes() {
        let mut board = Board::new();
        board.elements = vec![
            Element {
                id: 1,
                shape: ShapeType::Rect,
                pos: Vec2::ZERO,
                size: Vec2::splat(10.0),
                rotation: 0.0,
                color: [1.0, 0.0, 0.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: None,
                text_layout_generation: 0,
            },
            Element {
                id: 2,
                shape: ShapeType::Image,
                pos: Vec2::ZERO,
                size: Vec2::splat(10.0),
                rotation: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: Some(ImageData {
                    asset_path: "img.webp".to_string(),
                    hires_asset_path: None,
                    original_width: 10,
                    original_height: 10,
                    base_width: 10,
                    base_height: 10,
                }),
                text_layout_generation: 0,
            },
            Element {
                id: 3,
                shape: ShapeType::Ellipse,
                pos: Vec2::ZERO,
                size: Vec2::splat(10.0),
                rotation: 0.0,
                color: [0.0, 1.0, 0.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: None,
                text_layout_generation: 0,
            },
        ];

        assert!(board.bring_shape_to_front(1));
        assert_eq!(board.elements.iter().map(|element| element.id).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn hit_test_prioritizes_images_over_shape_layer() {
        let mut board = Board::new();
        board.elements = vec![
            Element {
                id: 1,
                shape: ShapeType::Image,
                pos: Vec2::ZERO,
                size: Vec2::splat(20.0),
                rotation: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: Some(ImageData {
                    asset_path: "img.webp".to_string(),
                    hires_asset_path: None,
                    original_width: 20,
                    original_height: 20,
                    base_width: 20,
                    base_height: 20,
                }),
                text_layout_generation: 0,
            },
            Element {
                id: 2,
                shape: ShapeType::Rect,
                pos: Vec2::ZERO,
                size: Vec2::splat(20.0),
                rotation: 0.0,
                color: [1.0, 0.0, 0.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: None,
                text_layout_generation: 0,
            },
        ];

        assert_eq!(board.hit_test(Vec2::new(10.0, 10.0)), Some(1));
    }

    #[test]
    fn hit_test_uses_board_order_within_shape_layer() {
        let mut board = Board::new();
        board.elements = vec![
            Element {
                id: 1,
                shape: ShapeType::Rect,
                pos: Vec2::ZERO,
                size: Vec2::splat(20.0),
                rotation: 0.0,
                color: [1.0, 0.0, 0.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: None,
                text_layout_generation: 0,
            },
            Element {
                id: 2,
                shape: ShapeType::Ellipse,
                pos: Vec2::ZERO,
                size: Vec2::splat(20.0),
                rotation: 0.0,
                color: [0.0, 1.0, 0.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: None,
                text_layout_generation: 0,
            },
        ];

        assert_eq!(board.hit_test(Vec2::new(10.0, 10.0)), Some(2));
    }

    #[test]
    fn hit_test_prioritizes_text_elements_over_images() {
        let mut board = Board::new();
        board.elements = vec![
            Element {
                id: 1,
                shape: ShapeType::Image,
                pos: Vec2::ZERO,
                size: Vec2::splat(20.0),
                rotation: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: None,
                image: Some(ImageData {
                    asset_path: "img.webp".to_string(),
                    hires_asset_path: None,
                    original_width: 20,
                    original_height: 20,
                    base_width: 20,
                    base_height: 20,
                }),
                text_layout_generation: 0,
            },
            Element {
                id: 2,
                shape: ShapeType::Text,
                pos: Vec2::ZERO,
                size: Vec2::splat(20.0),
                rotation: 0.0,
                color: [0.0, 0.0, 0.0, 0.0],
                stroke_color: default_stroke_color(),
                border_width: default_border_width(),
                stroke_width: default_line_stroke_width(),
                selected: false,
                text: Some(TextData {
                    content: "hello".to_string(),
                    font_size: 24.0,
                    color: DEFAULT_TEXT_COLOR,
                }),
                image: None,
                text_layout_generation: 0,
            },
        ];

        assert_eq!(board.hit_test(Vec2::new(10.0, 10.0)), Some(2));
    }
}

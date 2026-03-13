use glam::Vec2;
use serde::{Deserialize, Serialize};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShapeType {
    Rect,
    Ellipse,
    Line,
    Text,
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
            color: [0.96, 0.97, 0.99, 1.0],
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
    pub selected: bool,
    pub text: Option<TextData>,
}

impl Element {
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
        matches!(self.shape, ShapeType::Rect | ShapeType::Ellipse | ShapeType::Text)
    }

    pub fn text_bounds(&self) -> Option<(Vec2, Vec2)> {
        if !self.can_host_text() {
            return None;
        }

        let padding = Vec2::splat(12.0);
        let min = self.pos + padding;
        let max = self.pos + (self.size - padding * 2.0).max(Vec2::splat(1.0));
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

#[derive(Clone, Debug)]
pub enum ElementPropertyPatch {
    Transform {
        before: ElementTransform,
        after: ElementTransform,
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

#[derive(Clone, Debug)]
pub enum BoardOperation {
    AddElement(Element),
    DeleteElement(Element),
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
            BoardOperation::SetProperty { changes } => {
                for change in changes {
                    let after = match &change.patch {
                        ElementPropertyPatch::Transform { after, .. } => after,
                        ElementPropertyPatch::Text { .. } => continue,
                    };
                    if let Some(element) = self.elements.iter_mut().find(|e| e.id == change.id) {
                        element.pos = after.pos;
                        element.size = after.size;
                        element.rotation = after.rotation;
                    }
                }
                for change in changes {
                    let after = match &change.patch {
                        ElementPropertyPatch::Text { after, .. } => after,
                        ElementPropertyPatch::Transform { .. } => continue,
                    };
                    if let Some(element) = self.elements.iter_mut().find(|e| e.id == change.id) {
                        element.text = after.clone();
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
        let changes: Vec<ElementPropertyChange> = self
            .elements
            .iter()
            .filter(|e| e.selected)
            .map(|element| ElementPropertyChange {
                id: element.id,
                patch: ElementPropertyPatch::Transform {
                    before: ElementTransform::new(element.pos, element.size, element.rotation),
                    after: ElementTransform::new(
                        element.pos + delta,
                        element.size,
                        element.rotation,
                    ),
                },
            })
            .collect();
        if !changes.is_empty() {
            self.apply_operation(BoardOperation::SetProperty { changes });
        }
    }

    pub fn deselect_all(&mut self) {
        for element in &mut self.elements {
            element.selected = false;
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
            if element_hit(element, p) {
                return Some(element.id);
            }
        }
        None
    }

    pub fn element(&self, id: u64) -> Option<&Element> {
        self.elements.iter().find(|element| element.id == id)
    }

    pub fn ensure_text(&mut self, id: u64) -> bool {
        let Some(element) = self.element(id) else {
            return false;
        };
        if !element.can_host_text() || element.text.is_some() {
            return false;
        }

        self.apply_operation(BoardOperation::SetProperty {
            changes: vec![ElementPropertyChange {
                id,
                patch: ElementPropertyPatch::Text {
                    before: None,
                    after: Some(TextData::default()),
                },
            }],
        });
        true
    }

    pub fn update_text<F>(&mut self, id: u64, mut update: F) -> bool
    where
        F: FnMut(&mut TextData),
    {
        let Some(element) = self.element(id) else {
            return false;
        };
        let Some(mut after) = element.text.clone() else {
            return false;
        };
        let before = Some(after.clone());
        update(&mut after);
        let after = Some(after);
        if before == after {
            return false;
        }

        self.apply_operation(BoardOperation::SetProperty {
            changes: vec![ElementPropertyChange {
                id,
                patch: ElementPropertyPatch::Text { before, after },
            }],
        });
        true
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
        ShapeType::Rect | ShapeType::Text => {
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
            dist_point_segment(p, a, b) <= 8.0
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

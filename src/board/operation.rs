use super::element::{Element, ElementStyleSnapshot};
use super::geometry::rotate_point;
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementTransform {
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}

impl ElementTransform {
    pub fn new(pos: Vec2, size: Vec2, rotation: f32) -> Self {
        Self {
            pos,
            size,
            rotation,
        }
    }
}

pub fn apply_transform(
    element: &mut Element,
    before: Option<ElementTransform>,
    after: ElementTransform,
) {
    let size_changed = before.map_or(element.size != after.size, |b| b.size != after.size);
    element.pos = after.pos;
    element.size = after.size;
    element.rotation = after.rotation;
    if size_changed {
        element.bump_text_generation();
    }
}

pub fn move_element(element: &mut Element, delta: Vec2) {
    element.pos += delta;
}

pub fn rotate_element(element: &mut Element, center: Vec2, angle: f32) {
    if element.shape == crate::board::element::ShapeType::Line {
        let start = rotate_point(element.pos, center, angle);
        let end = rotate_point(element.pos + element.size, center, angle);
        element.pos = start;
        element.size = end - start;
        if let Some(normal) = element.line_start_normal {
            element.line_start_normal = Some(rotate_point(normal, Vec2::ZERO, angle));
        }
        if let Some(normal) = element.line_end_normal {
            element.line_end_normal = Some(rotate_point(normal, Vec2::ZERO, angle));
        }
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
        before: Option<crate::board::element::TextData>,
        after: Option<crate::board::element::TextData>,
    },
    LineCurve {
        before_bend: f32,
        after_bend: f32,
        before_midpoint_shift: f32,
        after_midpoint_shift: f32,
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
    pub before: crate::board::element::LineEndpoints,
    pub after: crate::board::element::LineEndpoints,
}

#[derive(Clone, Debug)]
pub enum BoardOperation {
    AddElement(Element),
    DeleteElement(Element),
    MoveElements {
        ids: Vec<u64>,
        delta: Vec2,
    },
    RotateElements {
        ids: Vec<u64>,
        center: Vec2,
        angle: f32,
    },
    SetElementRotations {
        changes: Vec<ElementRotationChange>,
    },
    SetProperty {
        changes: Vec<ElementPropertyChange>,
        sync_connected_lines: bool,
    },
    SetLineConnections {
        changes: Vec<LineConnectionChange>,
    },
}

#[derive(Clone, Debug)]
pub(super) enum HistoryEntry {
    OperationPair {
        undo: BoardOperation,
        redo: BoardOperation,
    },
    AddDelete {
        element: Element,
        is_add: bool,
    },
}

impl HistoryEntry {
    pub(super) fn from_operation(op: &BoardOperation) -> Self {
        match op {
            BoardOperation::AddElement(element) => Self::AddDelete {
                element: element.clone(),
                is_add: true,
            },
            BoardOperation::DeleteElement(element) => Self::AddDelete {
                element: element.clone(),
                is_add: false,
            },
            _ => Self::OperationPair {
                undo: inverse(op),
                redo: op.clone(),
            },
        }
    }
}

pub fn inverse(op: &BoardOperation) -> BoardOperation {
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
                        ElementPropertyPatch::Text { before, after } => {
                            ElementPropertyPatch::Text {
                                before: after.clone(),
                                after: before.clone(),
                            }
                        }
                        ElementPropertyPatch::LineCurve {
                            before_bend,
                            after_bend,
                            before_midpoint_shift,
                            after_midpoint_shift,
                        } => ElementPropertyPatch::LineCurve {
                            before_bend: *after_bend,
                            after_bend: *before_bend,
                            before_midpoint_shift: *after_midpoint_shift,
                            after_midpoint_shift: *before_midpoint_shift,
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

pub fn log_operation(op: &BoardOperation) {
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
            println!(
                "[ops] DELETE_ELEMENT id={} shape={:?}",
                element.id, element.shape
            );
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
                    change.id, change.before, change.after,
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
                            before
                                .as_ref()
                                .map(|text| text.content.chars().count())
                                .unwrap_or(0),
                            after
                                .as_ref()
                                .map(|text| text.content.chars().count())
                                .unwrap_or(0),
                        );
                    }
                    ElementPropertyPatch::LineCurve {
                        before_bend,
                        after_bend,
                        before_midpoint_shift,
                        after_midpoint_shift,
                    } => {
                        println!(
                            "[ops]   id={} line_curve bend {:.1} -> {:.1} shift {:.1} -> {:.1}",
                            change.id,
                            before_bend,
                            after_bend,
                            before_midpoint_shift,
                            after_midpoint_shift,
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

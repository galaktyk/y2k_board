use glam::Vec2;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeType {
    Rect,
    Ellipse,
    Line,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub id: u64,
    pub shape: ShapeType,
    /// World-space top-left for Rect/Ellipse; start point for Line.
    pub pos: Vec2,
    /// (width, height) for Rect/Ellipse; (dx, dy) end-delta for Line.
    pub size: Vec2,
    pub color: [f32; 4],
    pub selected: bool,
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
            _ => (self.pos, self.pos + self.size),
        }
    }
}

// ── Operations ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum Op {
    AddElement(Element),
    DeleteElement { id: u64 },
    MoveElements { moves: Vec<(u64, Vec2, Vec2)> }, // (id, old_pos, new_pos)
}

// inverse of an op that was already applied to the board
fn inverse(op: &Op, board: &Board) -> Option<Op> {
    match op {
        Op::AddElement(e) => Some(Op::DeleteElement { id: e.id }),
        Op::DeleteElement { id } => {
            board.elements.iter().find(|e| e.id == *id).map(|e| Op::AddElement(e.clone()))
        }
        Op::MoveElements { moves } => Some(Op::MoveElements {
            moves: moves.iter().map(|&(id, old, new)| (id, new, old)).collect(),
        }),
    }
}

// ── Board ────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub struct Board {
    pub elements: Vec<Element>,
    pub undo_stack: Vec<Op>,
    pub redo_stack: Vec<Op>,
    next_id: u64,
}

impl Board {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_id: 1,
        }
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // Apply an op and push its inverse onto the undo stack.
    pub fn apply_op(&mut self, op: Op) {
        let inv = inverse(&op, self);
        self.execute(&op);
        if let Some(inv) = inv {
            self.undo_stack.push(inv);
        }
        // Any new op clears the redo stack.
        self.redo_stack.clear();
    }

    fn execute(&mut self, op: &Op) {
        match op {
            Op::AddElement(e) => self.elements.push(e.clone()),
            Op::DeleteElement { id } => self.elements.retain(|e| e.id != *id),
            Op::MoveElements { moves } => {
                for (id, _old, new) in moves {
                    if let Some(e) = self.elements.iter_mut().find(|e| e.id == *id) {
                        e.pos = *new;
                    }
                }
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(inv) = self.undo_stack.pop() {
            // On undo we want to push the forward-op onto redo, so compute
            // the inverse of the inverse (= the original forward op).
            let fwd = inverse(&inv, self);
            self.execute(&inv);
            if let Some(fwd) = fwd {
                self.redo_stack.push(fwd);
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(fwd) = self.redo_stack.pop() {
            let inv = inverse(&fwd, self);
            self.execute(&fwd);
            if let Some(inv) = inv {
                self.undo_stack.push(inv);
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn delete_selected(&mut self) {
        let ids: Vec<u64> = self.elements.iter().filter(|e| e.selected).map(|e| e.id).collect();
        for id in ids {
            self.apply_op(Op::DeleteElement { id });
        }
    }

    #[allow(dead_code)]
    pub fn move_selected(&mut self, delta: Vec2) {
        let moves: Vec<(u64, Vec2, Vec2)> = self
            .elements
            .iter()
            .filter(|e| e.selected)
            .map(|e| (e.id, e.pos, e.pos + delta))
            .collect();
        if !moves.is_empty() {
            self.apply_op(Op::MoveElements { moves });
        }
    }

    pub fn deselect_all(&mut self) {
        for e in &mut self.elements {
            e.selected = false;
        }
    }

    pub fn select_only(&mut self, id: u64) {
        for e in &mut self.elements {
            e.selected = e.id == id;
        }
    }

    /// Hit-test a world-space point against elements (last-on-top).
    pub fn hit_test(&self, p: Vec2) -> Option<u64> {
        for e in self.elements.iter().rev() {
            if element_hit(e, p) {
                return Some(e.id);
            }
        }
        None
    }
}

fn element_hit(e: &Element, p: Vec2) -> bool {
    match e.shape {
        ShapeType::Rect => {
            p.x >= e.pos.x && p.x <= e.pos.x + e.size.x && p.y >= e.pos.y && p.y <= e.pos.y + e.size.y
        }
        ShapeType::Ellipse => {
            let c = e.pos + e.size * 0.5;
            let r = e.size * 0.5;
            if r.x == 0.0 || r.y == 0.0 {
                return false;
            }
            let d = (p - c) / r;
            d.dot(d) <= 1.0
        }
        ShapeType::Line => {
            // hit within 8 world units of the line segment
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

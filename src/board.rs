use glam::Vec2;
use crate::input::SelectionBounds;

pub mod geometry;
pub mod element;
pub mod operation;

#[cfg(test)]
mod tests;

pub use element::{
    BoxToolStyle, Element, ElementKind, ElementStyleSnapshot, ImageData, LineAnchor,
    LineEndpoints, ShapeType, TextData, ToolStyleDefaults, DEFAULT_BORDER_WIDTH,
    DEFAULT_ELLIPSE_COLOR, DEFAULT_LINE_COLOR, DEFAULT_LINE_STROKE_WIDTH, DEFAULT_RECT_COLOR,
    DEFAULT_STROKE_COLOR, DEFAULT_TEXT_COLOR, default_border_width, default_line_stroke_width,
    default_stroke_color, default_text_box_color,
};
pub use operation::{
    BoardOperation, ElementPropertyChange, ElementPropertyPatch, ElementRotationChange,
    ElementTransform, LineConnectionChange, apply_transform, log_operation,
    move_element, rotate_element,
};
pub use geometry::{rotate_point, world_to_local_norm};
use operation::HistoryEntry;
use geometry::element_hit;

// ── Board ────────────────────────────────────────────────────────────────────

pub struct Board {
    pub elements: Vec<Element>,
    pub line_attachments: std::collections::HashMap<u64, LineEndpoints>,
    connected_lines: std::collections::HashMap<u64, Vec<u64>>,
    index_by_id: std::collections::HashMap<u64, usize>,
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
            index_by_id: std::collections::HashMap::new(),
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
        let entry = HistoryEntry::from_operation(&op);
        self.execute(&op);
        self.undo_stack.push(entry);
        self.redo_stack.clear();
        log_operation(&op);
        self.emitted_ops.push(op);
    }

    pub fn insert_element_untracked(&mut self, element: Element) {
        self.execute(&BoardOperation::AddElement(element));
    }

    fn rebuild_index_by_id(&mut self) {
        self.index_by_id.clear();
        self.index_by_id.reserve(self.elements.len());
        for (index, element) in self.elements.iter().enumerate() {
            self.index_by_id.insert(element.id, index);
        }
    }

    fn cached_index(&self, id: u64) -> Option<usize> {
        self.index_by_id.get(&id).copied().filter(|&index| {
            self.elements
                .get(index)
                .map(|element| element.id == id)
                .unwrap_or(false)
        })
    }

    fn position_of_id(&self, id: u64) -> Option<usize> {
        self.cached_index(id)
            .or_else(|| self.elements.iter().position(|element| element.id == id))
    }

    fn position_of_id_mut(&mut self, id: u64) -> Option<usize> {
        if let Some(index) = self.cached_index(id) {
            return Some(index);
        }

        let index = self.elements.iter().position(|element| element.id == id)?;
        self.rebuild_index_by_id();
        Some(index)
    }

    fn remove_element_by_id(&mut self, id: u64) {
        let Some(index) = self.position_of_id_mut(id) else {
            return;
        };
        self.elements.remove(index);
        self.rebuild_index_by_id();
    }

    fn upsert_element(&mut self, element: Element) {
        self.remove_element_by_id(element.id);
        self.elements.push(element);
        let index = self.elements.len() - 1;
        let id = self.elements[index].id;
        self.index_by_id.insert(id, index);
    }

    pub fn ordered_candidate_indices(
        &self,
        candidate_ids: Option<&std::collections::HashSet<u64>>,
    ) -> Vec<usize> {
        match candidate_ids {
            Some(candidate_ids) => {
                let mut indices: Vec<usize> = candidate_ids
                    .iter()
                    .filter_map(|id| self.position_of_id(*id))
                    .collect();
                indices.sort_unstable();
                indices
            }
            None => (0..self.elements.len()).collect(),
        }
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
        self.rebuild_index_by_id();
        
        self.next_id = data.next_id.max(1);
        self.clear_transient_state(true);
    }

    pub fn clear_transient_state(&mut self, release_memory: bool) {
        if release_memory {
            self.undo_stack = Vec::new();
            self.redo_stack = Vec::new();
            self.emitted_ops = Vec::new();
        } else {
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.emitted_ops.clear();
        }
    }

    #[allow(dead_code)]
    pub fn update_connected_lines(&mut self, target_id: u64) {
        // [HOT] Updating connected lines 
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

    fn anchored_position_from_transform(
        transform: ElementTransform,
        norm_pos: Vec2,
    ) -> Vec2 {
        let origin = transform.pos + transform.size * 0.5;
        let local = (norm_pos - Vec2::splat(0.5)) * transform.size;
        rotate_point(origin + local, origin, transform.rotation)
    }

    fn preview_transform(
        &self,
        target_id: u64,
        preview_transforms: &std::collections::HashMap<u64, ElementTransform>,
    ) -> Option<ElementTransform> {
        preview_transforms.get(&target_id).copied().or_else(|| {
            self.element(target_id)
                .map(|target| ElementTransform::new(target.pos, target.size, target.rotation))
        })
    }

    /// Compute preview positions for lines connected to elements being dragged
    /// by temporary transform previews. Returns `(line_id, new_pos, new_size)` for
    /// each affected line that is **not** itself selected.
    pub fn compute_drag_line_previews(
        &self,
        selected_ids: &std::collections::HashSet<u64>,
        preview_transforms: &std::collections::HashMap<u64, ElementTransform>,
    ) -> Vec<(u64, Vec2, Vec2)> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for &target_id in selected_ids {
            let line_ids = match self.connected_lines.get(&target_id) {
                Some(ids) => ids,
                None => continue,
            };

            for &line_id in line_ids {
                if selected_ids.contains(&line_id) || !seen.insert(line_id) {
                    continue;
                }

                let endpoints = match self.line_attachments.get(&line_id) {
                    Some(ep) => ep,
                    None => continue,
                };

                let line = match self.element(line_id) {
                    Some(el) => el,
                    None => continue,
                };

                let mut start_pos = line.pos;
                let mut end_pos = line.pos + line.size;

                if let Some(start) = &endpoints.start {
                    if let Some(transform) = self.preview_transform(start.target_id, preview_transforms) {
                        start_pos = Self::anchored_position_from_transform(transform, start.norm_pos);
                    }
                }

                if let Some(end) = &endpoints.end {
                    if let Some(transform) = self.preview_transform(end.target_id, preview_transforms) {
                        end_pos = Self::anchored_position_from_transform(transform, end.norm_pos);
                    }
                }

                result.push((line_id, start_pos, end_pos - start_pos));
            }
        }

        result
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
                self.upsert_element(element.clone());
            }
            BoardOperation::DeleteElement(element) => {
                self.remove_element_by_id(element.id);
            }
            BoardOperation::MoveElements { ids, delta } => {
                let mut affected_targets = Vec::with_capacity(ids.len());
                for id in ids {
                    if let Some(index) = self.position_of_id_mut(*id) {
                        let element = &mut self.elements[index];
                        move_element(element, *delta);
                        affected_targets.push(*id);
                    }
                }
                self.update_connected_lines_for_targets(affected_targets);
            }
            BoardOperation::RotateElements { ids, center, angle } => {
                let mut affected_targets = Vec::with_capacity(ids.len());
                for id in ids {
                    if let Some(index) = self.position_of_id_mut(*id) {
                        let element = &mut self.elements[index];
                        rotate_element(element, *center, *angle);
                        affected_targets.push(*id);
                    }
                }
                self.update_connected_lines_for_targets(affected_targets);
            }
            BoardOperation::SetElementRotations { changes } => {
                let mut affected_targets = Vec::with_capacity(changes.len());
                for change in changes {
                    if let Some(index) = self.position_of_id_mut(change.id) {
                        let element = &mut self.elements[index];
                        element.rotation = change.after;
                        affected_targets.push(change.id);
                    }
                }
                self.update_connected_lines_for_targets(affected_targets);
            }
            BoardOperation::SetProperty {
                changes,
                sync_connected_lines,
            } => {
                let mut connected_line_targets = Vec::new();
                for change in changes {
                    let (before, after) = match &change.patch {
                        ElementPropertyPatch::Transform { before, after } => (Some(*before), *after),
                        ElementPropertyPatch::Style { .. } | ElementPropertyPatch::Text { .. } => continue,
                    };
                    if let Some(index) = self.position_of_id_mut(change.id) {
                        let element = &mut self.elements[index];
                        apply_transform(element, before, after);
                        if *sync_connected_lines {
                            connected_line_targets.push(change.id);
                        }
                    }
                }
                if *sync_connected_lines {
                    self.update_connected_lines_for_targets(connected_line_targets);
                }
                for change in changes {
                    let after = match &change.patch {
                        ElementPropertyPatch::Style { after, .. } => after,
                        ElementPropertyPatch::Transform { .. } | ElementPropertyPatch::Text { .. } => continue,
                    };
                    if let Some(index) = self.position_of_id_mut(change.id) {
                        let element = &mut self.elements[index];
                        element.apply_style_snapshot(*after);
                    }
                }
                for change in changes {
                    let after = match &change.patch {
                        ElementPropertyPatch::Text { after, .. } => after,
                        ElementPropertyPatch::Transform { .. } | ElementPropertyPatch::Style { .. } => continue,
                    };
                    if let Some(index) = self.position_of_id_mut(change.id) {
                        let element = &mut self.elements[index];
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

    fn replay_add_delete_history(&mut self, element: &Element, add: bool) {
        if add {
            self.next_id = self.next_id.max(element.id.saturating_add(1));
            self.upsert_element(element.clone());
        } else {
            self.remove_element_by_id(element.id);
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            match &entry {
                HistoryEntry::OperationPair { undo, .. } => self.execute(undo),
                HistoryEntry::AddDelete { element, is_add } => {
                    self.replay_add_delete_history(element, !is_add);
                }
            }
            self.redo_stack.push(entry);
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            match &entry {
                HistoryEntry::OperationPair { redo, .. } => self.execute(redo),
                HistoryEntry::AddDelete { element, is_add } => {
                    self.replay_add_delete_history(element, *is_add);
                }
            }
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
        self.element(id)
            .map(|element| element.selected)
            .unwrap_or(false)
    }

    pub fn toggle_selected(&mut self, id: u64) {
        if let Some(element) = self.element_mut(id) {
            element.selected = !element.selected;
        }
    }

    pub fn bring_to_front(&mut self, id: u64) -> bool {
        if let Some(index) = self.position_of_id_mut(id) {
            if index < self.elements.len() - 1 {
                let element = self.elements.remove(index);
                self.elements.push(element);
                self.rebuild_index_by_id();
                return true;
            }
        }
        false
    }

    pub fn send_to_back(&mut self, id: u64) -> bool {
        if let Some(index) = self.position_of_id_mut(id) {
            if index > 0 {
                let element = self.elements.remove(index);
                self.elements.insert(0, element);
                self.rebuild_index_by_id();
                return true;
            }
        }
        false
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

    #[allow(dead_code)]
    pub fn select_intersecting_bounds(&mut self, bounds: SelectionBounds, additive: bool) {
        self.select_intersecting_bounds_filtered(bounds, additive, None);
    }

    pub fn select_intersecting_bounds_filtered(
        &mut self,
        bounds: SelectionBounds,
        additive: bool,
        candidate_ids: Option<&std::collections::HashSet<u64>>,
    ) {
        let min = bounds.min();
        let max = bounds.max();

        if !additive {
            self.deselect_all();
        }

        match candidate_ids {
            Some(candidate_ids) => {
                let ids: Vec<u64> = candidate_ids.iter().copied().collect();
                for id in ids {
                    let Some(index) = self.position_of_id_mut(id) else {
                        continue;
                    };
                    let element = &mut self.elements[index];
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
            None => {
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

    #[allow(dead_code)]
    pub fn hit_test(&self, p: Vec2) -> Option<u64> {
        self.hit_test_filtered(p, None)
    }

    #[allow(dead_code)]
    pub fn hit_test_all(&self, p: Vec2) -> Vec<u64> {
        self.hit_test_all_filtered(p, None)
    }

    pub fn hit_test_filtered(
        &self,
        p: Vec2,
        candidate_ids: Option<&std::collections::HashSet<u64>>,
    ) -> Option<u64> {
        let ordered_indices = self.ordered_candidate_indices(candidate_ids);

        for &index in ordered_indices.iter().rev() {
            let element = &self.elements[index];
            if element.shape != ShapeType::Image && element_hit(element, p) {
                return Some(element.id);
            }
        }

        for &index in ordered_indices.iter().rev() {
            let element = &self.elements[index];
            if element.shape == ShapeType::Image && element_hit(element, p) {
                return Some(element.id);
            }
        }

        None
    }

    pub fn hit_test_all_filtered(
        &self,
        p: Vec2,
        candidate_ids: Option<&std::collections::HashSet<u64>>,
    ) -> Vec<u64> {
        let mut hits = Vec::new();

        let ordered_indices = self.ordered_candidate_indices(candidate_ids);

        for &index in ordered_indices.iter().rev() {
            let element = &self.elements[index];
            if element.shape != ShapeType::Image && element_hit(element, p) {
                hits.push(element.id);
            }
        }

        for &index in ordered_indices.iter().rev() {
            let element = &self.elements[index];
            if element.shape == ShapeType::Image && element_hit(element, p) {
                hits.push(element.id);
            }
        }

        hits
    }

    pub fn element(&self, id: u64) -> Option<&Element> {
        self.position_of_id(id)
            .and_then(|index| self.elements.get(index))
    }

    pub fn element_mut(&mut self, id: u64) -> Option<&mut Element> {
        let index = self.position_of_id_mut(id)?;
        self.elements.get_mut(index)
    }
}



#[cfg(test)]
mod temp_test;

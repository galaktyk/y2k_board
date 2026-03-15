use std::collections::{HashMap, HashSet};

use glam::Vec2;

use crate::board::{Board, Element};
use crate::camera::Camera;
use crate::renderer::InstanceData;
use crate::spatial::SpatialGrid;
use crate::ui::overlay;

const BOARD_VISIBILITY_MARGIN: f32 = 64.0;

#[derive(Clone, Copy)]
struct VisibleRange {
    min: Vec2,
    max: Vec2,
}

#[derive(Default)]
pub struct BoardRenderCache {
    all_instances: Vec<InstanceData>,
    id_by_index: Vec<u64>,
    index_by_id: HashMap<u64, usize>,
    visible_instances: Vec<InstanceData>,
    visible_board_indices: Vec<usize>,
    visible_index_by_id: HashMap<u64, usize>,
    visible_range: Option<VisibleRange>,
}

impl BoardRenderCache {
    pub fn rebuild_all(&mut self, board: &Board) {
        self.all_instances.clear();
        self.id_by_index.clear();
        self.index_by_id.clear();
        self.all_instances.reserve(board.elements.len());
        self.id_by_index.reserve(board.elements.len());

        for (index, element) in board.elements.iter().enumerate() {
            self.index_by_id.insert(element.id, index);
            self.id_by_index.push(element.id);
            self.all_instances.push(overlay::element_instance(element, 1.0));
        }
    }

    pub fn rebuild_visible(
        &mut self,
        board: &Board,
        spatial: &SpatialGrid,
        camera: &Camera,
        screen_size: Vec2,
    ) -> bool {
        let (vis_min, vis_max) = camera.visible_rect(screen_size);
        let min = vis_min - Vec2::splat(BOARD_VISIBILITY_MARGIN);
        let max = vis_max + Vec2::splat(BOARD_VISIBILITY_MARGIN);
        let visible_ids = spatial.query(min, max);

        let previous_indices = self.visible_board_indices.clone();

        self.visible_instances.clear();
        self.visible_board_indices.clear();
        self.visible_index_by_id.clear();
        self.visible_range = Some(VisibleRange { min, max });

        for (board_index, element) in board.elements.iter().enumerate() {
            if visible_ids.contains(&element.id) {
                self.push_visible(board_index, element.id);
            }
        }

        self.visible_board_indices != previous_indices
    }

    pub fn update_elements(&mut self, board: &Board, dirty_ids: &HashSet<u64>) {
        if dirty_ids.is_empty() {
            return;
        }

        let visible_range = self.visible_range;
        for &id in dirty_ids {
            let Some(&board_index) = self.index_by_id.get(&id) else {
                continue;
            };
            let element = &board.elements[board_index];
            self.all_instances[board_index] = overlay::element_instance(element, 1.0);

            let should_be_visible = visible_range
                .map(|range| element_in_range(element, range))
                .unwrap_or(false);

            match (self.visible_index_by_id.get(&id).copied(), should_be_visible) {
                (Some(visible_index), true) => {
                    self.visible_instances[visible_index] = self.all_instances[board_index];
                }
                (Some(visible_index), false) => {
                    self.remove_visible(visible_index);
                }
                (None, true) => {
                    self.insert_visible(board_index, id);
                }
                (None, false) => {}
            }
        }
    }

    pub fn all_instances(&self) -> &[InstanceData] {
        &self.all_instances
    }

    fn push_visible(&mut self, board_index: usize, id: u64) {
        let visible_index = self.visible_instances.len();
        self.visible_instances.push(self.all_instances[board_index]);
        self.visible_board_indices.push(board_index);
        self.visible_index_by_id.insert(id, visible_index);
    }

    fn insert_visible(&mut self, board_index: usize, id: u64) {
        let insert_at = self
            .visible_board_indices
            .iter()
            .position(|&existing| existing > board_index)
            .unwrap_or(self.visible_board_indices.len());

        self.visible_instances
            .insert(insert_at, self.all_instances[board_index]);
        self.visible_board_indices.insert(insert_at, board_index);
        self.visible_index_by_id.insert(id, insert_at);
        self.reindex_visible_from(insert_at + 1);
    }

    fn remove_visible(&mut self, visible_index: usize) {
        let board_index = self.visible_board_indices.remove(visible_index);
        self.visible_instances.remove(visible_index);
        let id = self.id_by_index[board_index];
        self.visible_index_by_id.remove(&id);
        self.reindex_visible_from(visible_index);
    }

    fn reindex_visible_from(&mut self, start: usize) {
        for visible_index in start..self.visible_board_indices.len() {
            let board_index = self.visible_board_indices[visible_index];
            let id = self.id_by_index[board_index];
            self.visible_index_by_id.insert(id, visible_index);
        }
    }
}

pub fn element_in_expanded_view(camera: &Camera, screen_size: Vec2, element: &Element) -> bool {
    let (vis_min, vis_max) = camera.visible_rect(screen_size);
    let range = VisibleRange {
        min: vis_min - Vec2::splat(BOARD_VISIBILITY_MARGIN),
        max: vis_max + Vec2::splat(BOARD_VISIBILITY_MARGIN),
    };
    element_in_range(element, range)
}

fn element_in_range(element: &Element, range: VisibleRange) -> bool {
    let (min, max) = element.aabb();
    min.x <= range.max.x
        && max.x >= range.min.x
        && min.y <= range.max.y
        && max.y >= range.min.y
}

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use glam::Vec2;

use crate::board::{Board, Element, LinePreviewPatch};
use crate::camera::Camera;
use crate::rendering::renderer::InstanceData;
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
    element_ranges: Vec<Range<usize>>,
    id_by_index: Vec<u64>,
    index_by_id: HashMap<u64, usize>,
    visible_range: Option<VisibleRange>,
}

impl BoardRenderCache {
    pub fn hard_reset(&mut self) {
        self.all_instances.clear();
        self.all_instances.shrink_to_fit();
        self.element_ranges.clear();
        self.element_ranges.shrink_to_fit();
        self.id_by_index.clear();
        self.id_by_index.shrink_to_fit();
        self.index_by_id.clear();
        self.index_by_id.shrink_to_fit();
        self.visible_range = None;
    }

    pub fn rebuild_all(&mut self, board: &Board) {
        self.all_instances.clear();
        self.element_ranges.clear();
        self.id_by_index.clear();
        self.index_by_id.clear();
        self.all_instances.reserve(board.elements.len() * 2);
        self.element_ranges.reserve(board.elements.len());
        self.id_by_index.reserve(board.elements.len());

        for (index, element) in board.elements.iter().enumerate() {
            self.index_by_id.insert(element.id, index);
            self.id_by_index.push(element.id);
            let start = self.all_instances.len();
            self.all_instances
                .extend(overlay::element_to_instances(element, 1.0));
            let end = self.all_instances.len();
            self.element_ranges.push(start..end);
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
        let _ = spatial.query(min, max);
        let _ = board;
        let _ = screen_size;
        self.visible_range = Some(VisibleRange { min, max });
        false
    }

    pub fn update_elements(&mut self, board: &Board, dirty_ids: &HashSet<u64>) {
        if dirty_ids.is_empty() {
            return;
        }

        let mut dirty_indices: Vec<usize> = dirty_ids
            .iter()
            .filter_map(|id| self.index_by_id.get(id).copied())
            .collect();
        dirty_indices.sort_unstable();

        for board_index in dirty_indices {
            let element = &board.elements[board_index];
            let new_instances = overlay::element_to_instances(element, 1.0);
            self.replace_element_instances(board_index, new_instances);
        }
    }

    /// Replace cached line instances with preview versions while dragging connected targets.
    pub fn patch_line_previews(&mut self, board: &Board, patches: &[LinePreviewPatch]) {
        let mut indexed_patches: Vec<(usize, LinePreviewPatch)> = patches
            .iter()
            .filter_map(|patch| self.index_by_id.get(&patch.id).copied().map(|index| (index, *patch)))
            .collect();
        indexed_patches.sort_unstable_by_key(|(index, _)| *index);

        for (index, patch) in indexed_patches {
            let Some(element) = board.element(patch.id) else {
                continue;
            };

            let mut preview_element = element.clone();
            preview_element.pos = patch.pos;
            preview_element.size = patch.size;
            preview_element.line_start_normal = patch.start_normal;
            preview_element.line_end_normal = patch.end_normal;

            let new_instances = overlay::element_to_instances(&preview_element, 1.0);
            self.replace_element_instances(index, new_instances);
        }
    }

    pub fn all_instances(&self) -> &[InstanceData] {
        &self.all_instances
    }

    pub fn element_count(&self) -> usize {
        self.id_by_index.len()
    }

    fn replace_element_instances(&mut self, board_index: usize, new_instances: Vec<InstanceData>) {
        let old_range = self.element_ranges[board_index].clone();
        let old_len = old_range.end - old_range.start;
        let new_len = new_instances.len();

        self.all_instances.splice(old_range.clone(), new_instances);
        self.element_ranges[board_index] = old_range.start..(old_range.start + new_len);

        let delta = new_len as isize - old_len as isize;
        if delta != 0 {
            for range in self.element_ranges.iter_mut().skip(board_index + 1) {
                range.start = ((range.start as isize) + delta) as usize;
                range.end = ((range.end as isize) + delta) as usize;
            }
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
    min.x <= range.max.x && max.x >= range.min.x && min.y <= range.max.y && max.y >= range.min.y
}

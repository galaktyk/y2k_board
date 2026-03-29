use std::collections::{HashMap, HashSet};
use std::ops::Range;

use glam::Vec2;

use crate::board::geometry::line_curve_from_state;
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

#[derive(Clone, Default)]
struct ElementInstanceRanges {
    shapes: Range<usize>,
    lines: Range<usize>,
}

#[derive(Default)]
pub struct BoardRenderCache {
    all_shape_instances: Vec<InstanceData>,
    all_line_instances: Vec<crate::rendering::renderer::LineInstanceData>,
    element_ranges: Vec<ElementInstanceRanges>,
    id_by_index: Vec<u64>,
    index_by_id: HashMap<u64, usize>,
    visible_range: Option<VisibleRange>,
}

impl BoardRenderCache {
    pub fn hard_reset(&mut self) {
        self.all_shape_instances.clear();
        self.all_shape_instances.shrink_to_fit();
        self.all_line_instances.clear();
        self.all_line_instances.shrink_to_fit();
        self.element_ranges.clear();
        self.element_ranges.shrink_to_fit();
        self.id_by_index.clear();
        self.id_by_index.shrink_to_fit();
        self.index_by_id.clear();
        self.index_by_id.shrink_to_fit();
        self.visible_range = None;
    }

    pub fn rebuild_all(&mut self, board: &Board) {
        self.all_shape_instances.clear();
        self.all_line_instances.clear();
        self.element_ranges.clear();
        self.id_by_index.clear();
        self.index_by_id.clear();
        self.all_shape_instances.reserve(board.elements.len() * 2);
        self.all_line_instances.reserve(board.elements.len());
        self.element_ranges.reserve(board.elements.len());
        self.id_by_index.reserve(board.elements.len());

        for (index, element) in board.elements.iter().enumerate() {
            self.index_by_id.insert(element.id, index);
            self.id_by_index.push(element.id);
            let instances = overlay::element_to_instances(element, 1.0).with_layer(index as f32);
            let shape_start = self.all_shape_instances.len();
            let line_start = self.all_line_instances.len();
            self.all_shape_instances.extend(instances.shapes);
            self.all_line_instances.extend(instances.lines);
            self.element_ranges.push(ElementInstanceRanges {
                shapes: shape_start..self.all_shape_instances.len(),
                lines: line_start..self.all_line_instances.len(),
            });
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
            let new_instances = overlay::element_to_instances(element, 1.0).with_layer(board_index as f32);
            self.replace_element_instances(board_index, new_instances);
        }
    }

    /// Replace cached line instances with preview versions while dragging connected targets.
    pub fn patch_line_previews(&mut self, board: &Board, patches: &[LinePreviewPatch]) {
        for patch in patches {
            let Some(index) = self.index_by_id.get(&patch.id).copied() else {
                continue;
            };
            let Some(element) = board.element(patch.id) else {
                continue;
            };

            let range = self.element_ranges[index].lines.clone();
            for instance in &mut self.all_line_instances[range] {
                instance.pos = patch.pos.to_array();
                instance.size = patch.size.to_array();

                let (c1, c2) = line_curve_from_state(
                    patch.pos,
                    patch.size,
                    element.line_bend,
                    element.line_midpoint_shift,
                    patch.start_normal,
                    patch.end_normal,
                )
                .map(|curve| (curve.c1.to_array(), curve.c2.to_array()))
                .unwrap_or_else(|| straight_line_controls(patch.pos, patch.pos + patch.size));

                instance.line_c1 = c1;
                instance.line_c2 = c2;
            }
        }
    }

    pub fn all_shape_instances(&self) -> &[InstanceData] {
        &self.all_shape_instances
    }

    pub fn all_line_instances(&self) -> &[crate::rendering::renderer::LineInstanceData] {
        &self.all_line_instances
    }

    pub fn element_count(&self) -> usize {
        self.id_by_index.len()
    }

    fn replace_element_instances(
        &mut self,
        board_index: usize,
        new_instances: overlay::OverlayInstances,
    ) {
        let old_ranges = self.element_ranges[board_index].clone();
        let old_shape_len = old_ranges.shapes.end - old_ranges.shapes.start;
        let old_line_len = old_ranges.lines.end - old_ranges.lines.start;
        let new_shape_len = new_instances.shapes.len();
        let new_line_len = new_instances.lines.len();

        self.all_shape_instances
            .splice(old_ranges.shapes.clone(), new_instances.shapes);
        self.all_line_instances
            .splice(old_ranges.lines.clone(), new_instances.lines);
        self.element_ranges[board_index] = ElementInstanceRanges {
            shapes: old_ranges.shapes.start..(old_ranges.shapes.start + new_shape_len),
            lines: old_ranges.lines.start..(old_ranges.lines.start + new_line_len),
        };

        let shape_delta = new_shape_len as isize - old_shape_len as isize;
        let line_delta = new_line_len as isize - old_line_len as isize;
        if shape_delta != 0 || line_delta != 0 {
            for ranges in self.element_ranges.iter_mut().skip(board_index + 1) {
                ranges.shapes.start = ((ranges.shapes.start as isize) + shape_delta) as usize;
                ranges.shapes.end = ((ranges.shapes.end as isize) + shape_delta) as usize;
                ranges.lines.start = ((ranges.lines.start as isize) + line_delta) as usize;
                ranges.lines.end = ((ranges.lines.end as isize) + line_delta) as usize;
            }
        }
    }
}

fn straight_line_controls(start: Vec2, end: Vec2) -> ([f32; 2], [f32; 2]) {
    let delta = end - start;
    (
        (start + delta / 3.0).to_array(),
        (start + delta * (2.0 / 3.0)).to_array(),
    )
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

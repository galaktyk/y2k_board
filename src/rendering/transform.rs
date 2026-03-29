use glam::Vec2;

use crate::rendering::renderer::{InstanceData, LineInstanceData};

pub fn offset_instance(mut instance: InstanceData, delta: Vec2) -> InstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
    instance
}

pub fn offset_line_instance(mut instance: LineInstanceData, delta: Vec2) -> LineInstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
    instance.line_c1[0] += delta.x;
    instance.line_c1[1] += delta.y;
    instance.line_c2[0] += delta.x;
    instance.line_c2[1] += delta.y;
    instance
}

pub fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center
        + Vec2::new(
            offset.x * cos_a - offset.y * sin_a,
            offset.x * sin_a + offset.y * cos_a,
        )
}

pub fn rotate_instance(mut instance: InstanceData, center: Vec2, angle: f32) -> InstanceData {
    let original_center = Vec2::new(instance.pos[0], instance.pos[1])
        + Vec2::new(instance.size[0], instance.size[1]) * 0.5;
    let rotated_center = rotate_point(original_center, center, angle);
    let size = Vec2::new(instance.size[0], instance.size[1]);
    instance.pos = (rotated_center - size * 0.5).to_array();
    instance.rotation += angle;
    instance
}

pub fn rotate_line_instance(
    mut instance: LineInstanceData,
    center: Vec2,
    angle: f32,
) -> LineInstanceData {
    let start = Vec2::new(instance.pos[0], instance.pos[1]);
    let end = start + Vec2::new(instance.size[0], instance.size[1]);
    let c1 = Vec2::new(instance.line_c1[0], instance.line_c1[1]);
    let c2 = Vec2::new(instance.line_c2[0], instance.line_c2[1]);
    let rotated_start = rotate_point(start, center, angle);
    let rotated_end = rotate_point(end, center, angle);
    let rotated_c1 = rotate_point(c1, center, angle);
    let rotated_c2 = rotate_point(c2, center, angle);
    instance.pos = rotated_start.to_array();
    instance.size = (rotated_end - rotated_start).to_array();
    instance.line_c1 = rotated_c1.to_array();
    instance.line_c2 = rotated_c2.to_array();
    instance
}

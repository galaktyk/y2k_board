use glam::Vec2;

use crate::board::Element;
use crate::renderer::{InstanceData, TextInstanceData};

pub fn offset_instance(mut instance: InstanceData, delta: Vec2) -> InstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
    instance
}

pub fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center + Vec2::new(
        offset.x * cos_a - offset.y * sin_a,
        offset.x * sin_a + offset.y * cos_a,
    )
}

pub fn rotate_instance(mut instance: InstanceData, center: Vec2, angle: f32) -> InstanceData {
    if instance.shape_type == 2 || instance.shape_type == 7 {
        let start = Vec2::new(instance.pos[0], instance.pos[1]);
        let end = start + Vec2::new(instance.size[0], instance.size[1]);
        let rotated_start = rotate_point(start, center, angle);
        let rotated_end = rotate_point(end, center, angle);
        instance.pos = rotated_start.to_array();
        instance.size = (rotated_end - rotated_start).to_array();
        return instance;
    }

    let original_center = Vec2::new(instance.pos[0], instance.pos[1])
        + Vec2::new(instance.size[0], instance.size[1]) * 0.5;
    let rotated_center = rotate_point(original_center, center, angle);
    let size = Vec2::new(instance.size[0], instance.size[1]);
    instance.pos = (rotated_center - size * 0.5).to_array();
    instance.rotation += angle;
    instance
}

pub fn offset_text_instance(mut instance: TextInstanceData, delta: Vec2) -> TextInstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
    instance.origin[0] = instance.origin[0].saturating_add(delta.x.round() as i16);
    instance.origin[1] = instance.origin[1].saturating_add(delta.y.round() as i16);
    instance
}

pub fn rotate_text_instance(
    mut instance: TextInstanceData,
    center: Vec2,
    angle: f32,
) -> TextInstanceData {
    let original_origin = Vec2::new(instance.origin[0] as f32, instance.origin[1] as f32);
    let origin = rotate_point(original_origin, center, angle);
    let pos = Vec2::new(instance.pos[0], instance.pos[1]) + (origin - original_origin);

    instance.pos = pos.to_array();
    instance.origin = [
        origin.x.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
        origin.y.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
    ];
    instance.rotation += angle;
    instance
}

pub fn preview_rect_transform(
    element: &Element,
    move_drag_offset: Option<Vec2>,
    rotate_drag_preview: Option<(f32, Vec2)>,
) -> (Vec2, Vec2, f32) {
    if let Some((angle, center)) = rotate_drag_preview.filter(|_| element.selected) {
        let original_center = element.pos + element.size * 0.5;
        let rotated_center = rotate_point(original_center, center, angle);
        (
            rotated_center - element.size * 0.5,
            element.size,
            element.rotation + angle,
        )
    } else if let Some(offset) = move_drag_offset.filter(|_| element.selected) {
        (element.pos + offset, element.size, element.rotation)
    } else {
        (element.pos, element.size, element.rotation)
    }
}
use glam::Vec2;

use crate::rendering::renderer::InstanceData;

pub fn offset_instance(mut instance: InstanceData, delta: Vec2) -> InstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
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
    if instance.shape_type == 2 || instance.shape_type == 6 {
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

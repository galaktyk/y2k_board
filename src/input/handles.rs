use glam::Vec2;

use crate::board::{Element, ShapeType};
use crate::renderer::InstanceData;

pub fn get_element_handles(e: &Element) -> Option<Vec<Vec2>> {
    if e.shape == ShapeType::Line {
        return Some(vec![e.pos, e.pos + e.size]);
    }
    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 {
        center + Vec2::new(rx * c - ry * s, rx * s + ry * c)
    };

    let hw = e.size.x * 0.5;
    let hh = e.size.y * 0.5;
    let th = -hh - 30.0;

    Some(vec![rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh), rot(0.0, th)])
}

pub fn handles_to_instances(e: &Element) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let handles = match get_element_handles(e) {
        Some(handles) => handles,
        None => return out,
    };

    let handle_size = 10.0;
    if e.shape == ShapeType::Line {
        for pt in handles {
            out.push(InstanceData {
                pos: [pt.x - handle_size * 0.5, pt.y - handle_size * 0.5],
                size: [handle_size, handle_size],
                rotation: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                shape_type: 0.0,
                alpha: 1.0,
            });
        }
        return out;
    }

    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s_rot = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 {
        center + Vec2::new(rx * c - ry * s_rot, rx * s_rot + ry * c)
    };
    let stick_center = rot(0.0, -e.size.y * 0.5 - 15.0);

    out.push(InstanceData {
        pos: [stick_center.x - 1.0, stick_center.y],
        size: [2.0, 15.0],
        rotation: e.rotation,
        color: [1.0, 1.0, 1.0, 0.9],
        shape_type: 0.0,
        alpha: 1.0,
    });

    for pt in handles {
        out.push(InstanceData {
            pos: [pt.x - handle_size * 0.5, pt.y - handle_size * 0.5],
            size: [handle_size, handle_size],
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            shape_type: 0.0,
            alpha: 1.0,
        });
    }

    out
}
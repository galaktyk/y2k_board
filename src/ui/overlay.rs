use glam::Vec2;

use crate::board::{Element, ShapeType};
use crate::input::SelectionBounds;
use crate::palette;
use crate::renderer::InstanceData;

const MARQUEE_COLOR: [f32; 4] = palette::BLUE;
const CREATION_OUTLINE_COLOR: [f32; 4] = palette::BLUE;
const MULTI_SELECTION_BOUNDS_COLOR: [f32; 4] = palette::BLUE;

const FIXED_SCREEN_OUTLINE_SHAPE_TYPE: f32 = 5.0;
const FIXED_SCREEN_ELLIPSE_OUTLINE_SHAPE_TYPE: f32 = 6.0;
const FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE: f32 = 7.0;

pub fn element_instance(element: &Element, alpha: f32) -> InstanceData {
    let shape_type = match element.shape {
        ShapeType::Rect => 0.0,
        ShapeType::Ellipse => 1.0,
        ShapeType::Line => 2.0,
        ShapeType::Text => 3.0,
        ShapeType::Image => 255.0,
    };

    InstanceData::new(
        element.pos.to_array(),
        element.size.to_array(),
        element.rotation,
        element.color,
        shape_type,
        alpha,
    )
}

pub fn selection_instance(element: &Element, zoom: f32, alpha: f32) -> Option<InstanceData> {
    if !element.selected {
        return None;
    }

    Some(selection_outline_instance(element, zoom, alpha))
}

pub fn selection_bounds_instance(bounds: SelectionBounds, zoom: f32, alpha: f32) -> InstanceData {
    bounds_outline_instance(bounds, zoom, MULTI_SELECTION_BOUNDS_COLOR, alpha)
}

pub fn marquee_instance(bounds: SelectionBounds, zoom: f32, alpha: f32) -> InstanceData {
    bounds_outline_instance(bounds, zoom, MARQUEE_COLOR, alpha)
}

pub fn preview_instances(element: &Element, zoom: f32, alpha: f32) -> Vec<InstanceData> {
    let mut instances = element_to_instances(element, alpha);

    if element.shape != ShapeType::Line {
        instances.push(selection_outline_instance(element, zoom, 1.0));
    }

    instances
}

pub fn element_to_instances(element: &Element, alpha: f32) -> Vec<InstanceData> {
    vec![element_instance(element, alpha)]
}

fn selection_outline_instance(element: &Element, zoom: f32, alpha: f32) -> InstanceData {
    let expand = 1.0 / zoom.max(0.0001);
    let shape_type = match element.shape {
        ShapeType::Rect | ShapeType::Text | ShapeType::Image => FIXED_SCREEN_OUTLINE_SHAPE_TYPE,
        ShapeType::Ellipse => FIXED_SCREEN_ELLIPSE_OUTLINE_SHAPE_TYPE,
        ShapeType::Line => FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE,
    };

    InstanceData::new(
        (element.pos - Vec2::splat(expand)).to_array(),
        (element.size + Vec2::splat(expand * 2.0)).to_array(),
        element.rotation,
        CREATION_OUTLINE_COLOR,
        shape_type,
        alpha,
    )
}

fn bounds_outline_instance(
    bounds: SelectionBounds,
    zoom: f32,
    color: [f32; 4],
    alpha: f32,
) -> InstanceData {
    let expand = 1.0 / zoom.max(0.0001);
    InstanceData::new(
        (bounds.pos - Vec2::splat(expand)).to_array(),
        (bounds.size + Vec2::splat(expand * 2.0)).to_array(),
        bounds.rotation,
        color,
        FIXED_SCREEN_OUTLINE_SHAPE_TYPE,
        alpha,
    )
}

use glam::Vec2;

use crate::board::{Element, ShapeType};
use crate::input::SelectionBounds;
use crate::palette;
use crate::rendering::renderer::InstanceData;

const MARQUEE_COLOR: [f32; 4] = palette::BLUE;
const CREATION_OUTLINE_COLOR: [f32; 4] = palette::BLUE;
const MULTI_SELECTION_BOUNDS_COLOR: [f32; 4] = palette::BLUE;

const FIXED_SCREEN_OUTLINE_SHAPE_TYPE: f32 = 5.0;
const FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE: f32 = 6.0;

#[allow(dead_code)]
pub fn element_instance(element: &Element, alpha: f32) -> InstanceData {
    element_to_instances(element, alpha)
        .into_iter()
        .next()
        .unwrap_or_default()
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
    let mut out = Vec::new();

    match element.shape {
        ShapeType::Rect => {
            push_fill_instance(&mut out, element, 0.0, element.color, alpha);
            push_border_instance(&mut out, element, 3.0, element.effective_stroke_color(), alpha);
        }
        ShapeType::Ellipse => {
            push_fill_instance(&mut out, element, 1.0, element.color, alpha);
            push_border_instance(&mut out, element, 4.0, element.effective_stroke_color(), alpha);
        }
        ShapeType::Line => {
            let color = element.effective_stroke_color();
            if color[3] > 0.0 {
                out.push(
                    InstanceData::new(
                        element.pos.to_array(),
                        element.size.to_array(),
                        element.rotation,
                        color,
                        2.0,
                        alpha,
                        element.selected,
                    )
                    .with_stroke_width(element.stroke_width),
                );
            }
        }

        ShapeType::Image => {
            out.push(InstanceData::new(
                element.pos.to_array(),
                element.size.to_array(),
                element.rotation,
                element.color,
                255.0,
                alpha,
                element.selected,
            ));
        }
    }

    out
}

fn push_fill_instance(
    out: &mut Vec<InstanceData>,
    element: &Element,
    shape_type: f32,
    color: [f32; 4],
    alpha: f32,
) {
    if color[3] <= 0.0 {
        return;
    }

    out.push(InstanceData::new(
        element.pos.to_array(),
        element.size.to_array(),
        element.rotation,
        color,
        shape_type,
        alpha,
        element.selected,
    ));
}

fn push_border_instance(
    out: &mut Vec<InstanceData>,
    element: &Element,
    shape_type: f32,
    color: [f32; 4],
    alpha: f32,
) {
    if color[3] <= 0.0 || element.border_width == 0 {
        return;
    }

    out.push(
        InstanceData::new(
            element.pos.to_array(),
            element.size.to_array(),
            element.rotation,
            color,
            shape_type,
            alpha,
            element.selected,
        )
        .with_stroke_width(element.border_width),
    );
}

fn selection_outline_instance(
    element: &Element,
    zoom: f32,
    alpha: f32,
) -> InstanceData {
    let expand = 1.0 / zoom.max(0.0001);
    let shape_type = match element.shape {
        ShapeType::Rect | ShapeType::Image => FIXED_SCREEN_OUTLINE_SHAPE_TYPE,
        ShapeType::Ellipse => FIXED_SCREEN_OUTLINE_SHAPE_TYPE,
        ShapeType::Line => FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE,
    };

    InstanceData::new(
        (element.pos - Vec2::splat(expand)).to_array(),
        (element.size + Vec2::splat(expand * 2.0)).to_array(),
        element.rotation,
        CREATION_OUTLINE_COLOR,
        shape_type,
        alpha,
        false,
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
        false,
    )
}

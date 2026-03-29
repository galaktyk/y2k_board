use glam::Vec2;

use crate::board::geometry::{line_curve, CubicBezier};
use crate::board::{Element, ShapeType};
use crate::input::SelectionBounds;
use crate::palette;
use crate::rendering::renderer::{InstanceData, LineInstanceData};

const MARQUEE_COLOR: [f32; 4] = palette::DARK_BLUE;
const CREATION_OUTLINE_COLOR: [f32; 4] = palette::BLUE;
const MULTI_SELECTION_BOUNDS_COLOR: [f32; 4] = palette::BLUE;

const FIXED_SCREEN_OUTLINE_SHAPE_TYPE: f32 = 5.0;
const FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE: f32 = 6.0;
const STICKY_NOTE_SHADOW_SHAPE_TYPE: f32 = 7.0;
const STICKY_NOTE_SHADOW_LAYER_OFFSET: f32 = 0.25;
const STICKY_NOTE_SHADOW_OFFSET_Y: f32 = 6.0;
const STICKY_NOTE_SHADOW_EXPAND_X: f32 = 8.0;
const STICKY_NOTE_SHADOW_EXPAND_Y: f32 = 10.0;
const STICKY_NOTE_SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.18];

#[derive(Default)]
pub struct OverlayInstances {
    pub shadows: Vec<InstanceData>,
    pub shapes: Vec<InstanceData>,
    pub lines: Vec<LineInstanceData>,
}

impl OverlayInstances {
    pub fn with_layer(mut self, layer: f32) -> Self {
        let shadow_layer = (layer - STICKY_NOTE_SHADOW_LAYER_OFFSET).max(0.0);
        for instance in &mut self.shadows {
            *instance = instance.with_layer(shadow_layer);
        }
        for instance in &mut self.shapes {
            *instance = instance.with_layer(layer);
        }
        for instance in &mut self.lines {
            *instance = instance.with_layer(layer);
        }
        self
    }
}

#[allow(dead_code)]
pub fn element_instance(element: &Element, alpha: f32) -> OverlayInstances {
    element_to_instances(element, alpha)
}

pub fn selection_instance(element: &Element, zoom: f32, alpha: f32) -> Option<InstanceData> {
    if !element.selected || element.shape == ShapeType::Line {
        return None;
    }

    Some(selection_outline_instance(element, zoom, alpha))
}

pub fn selection_instances(element: &Element, zoom: f32, alpha: f32) -> OverlayInstances {
    if !element.selected {
        return OverlayInstances::default();
    }

    if element.shape == ShapeType::Line {
        return OverlayInstances {
            shadows: Vec::new(),
            shapes: Vec::new(),
            lines: line_instances(
                element,
                CREATION_OUTLINE_COLOR,
                FIXED_SCREEN_LINE_OUTLINE_SHAPE_TYPE,
                alpha,
                1,
                false,
                false,
                false,
            ),
        };
    }

    OverlayInstances {
        shadows: Vec::new(),
        shapes: selection_instance(element, zoom, alpha).into_iter().collect(),
        lines: Vec::new(),
    }
}

pub fn selection_bounds_instance(bounds: SelectionBounds, zoom: f32, alpha: f32) -> InstanceData {
    bounds_outline_instance(bounds, zoom, MULTI_SELECTION_BOUNDS_COLOR, alpha)
}

pub fn marquee_instance(bounds: SelectionBounds, zoom: f32, alpha: f32) -> InstanceData {
    bounds_outline_instance(bounds, zoom, MARQUEE_COLOR, alpha)
}

pub fn preview_instances(element: &Element, zoom: f32, alpha: f32) -> OverlayInstances {
    let mut instances = element_to_instances(element, alpha);

    if element.shape != ShapeType::Line {
        instances
            .shapes
            .push(selection_outline_instance(element, zoom, 1.0));
    }

    instances
}

pub fn connection_preview_line_instance(
    start: Vec2,
    end: Vec2,
    color: [f32; 4],
    stroke_width: u8,
    alpha: f32,
) -> LineInstanceData {
    let (c1, c2) = straight_line_controls(start, end);
    LineInstanceData::new(
        start.to_array(),
        (end - start).to_array(),
        0.0,
        color,
        2.0,
        alpha,
        false,
    )
    .with_stroke_width(stroke_width.max(1))
    .with_line_curve_controls(c1.to_array(), c2.to_array())
    .with_line_arrowheads(false, true)
}

pub fn element_to_instances(element: &Element, alpha: f32) -> OverlayInstances {
    let mut out = OverlayInstances::default();

    match element.shape {
        ShapeType::Rect => {
            push_sticky_note_shadow_instance(&mut out.shadows, element, alpha);
            push_fill_instance(&mut out.shapes, element, 0.0, element.color, alpha);
            push_border_instance(
                &mut out.shapes,
                element,
                3.0,
                element.effective_stroke_color(),
                alpha,
            );
        }
        ShapeType::Ellipse => {
            push_fill_instance(&mut out.shapes, element, 1.0, element.color, alpha);
            push_border_instance(
                &mut out.shapes,
                element,
                4.0,
                element.effective_stroke_color(),
                alpha,
            );
        }
        ShapeType::Line => {
            let color = element.effective_stroke_color();
            if color[3] > 0.0 {
                out.lines.extend(line_instances(
                    element,
                    color,
                    2.0,
                    alpha,
                    element.stroke_width,
                    element.selected,
                    element.line_arrow_start,
                    element.line_arrow_end,
                ));
            }
        }

        ShapeType::Image => {
            out.shapes.push(InstanceData::new(
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

fn push_sticky_note_shadow_instance(out: &mut Vec<InstanceData>, element: &Element, alpha: f32) {
    if !element.is_sticky_note() || element.color[3] <= 0.0 {
        return;
    }

    out.push(InstanceData::new(
        [
            element.pos.x - STICKY_NOTE_SHADOW_EXPAND_X,
            element.pos.y + STICKY_NOTE_SHADOW_OFFSET_Y - STICKY_NOTE_SHADOW_EXPAND_Y,
        ],
        [
            element.size.x + STICKY_NOTE_SHADOW_EXPAND_X * 2.0,
            element.size.y + STICKY_NOTE_SHADOW_EXPAND_Y * 2.0,
        ],
        element.rotation,
        STICKY_NOTE_SHADOW_COLOR,
        STICKY_NOTE_SHADOW_SHAPE_TYPE,
        alpha,
        element.selected,
    ));
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

fn selection_outline_instance(element: &Element, zoom: f32, alpha: f32) -> InstanceData {
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

fn line_instances(
    element: &Element,
    color: [f32; 4],
    shape_type: f32,
    alpha: f32,
    stroke_width: u8,
    selected: bool,
    arrow_start: bool,
    arrow_end: bool,
) -> Vec<LineInstanceData> {
    let Some(curve) = line_curve_instance(element) else {
        return Vec::new();
    };

    let mut instance = LineInstanceData::new(
        element.pos.to_array(),
        element.size.to_array(),
        0.0,
        color,
        shape_type,
        alpha,
        selected,
    )
    .with_stroke_width(stroke_width.max(1))
    .with_line_curve_controls(curve.c1.to_array(), curve.c2.to_array());

    if shape_type > 1.5 && shape_type < 2.5 {
        instance = instance.with_line_arrowheads(arrow_start, arrow_end);
    }

    vec![instance]
}

fn line_curve_instance(element: &Element) -> Option<CubicBezier> {
    if element.shape != ShapeType::Line || element.size.length_squared() <= 0.0001 {
        return None;
    }

    line_curve(element).or_else(|| {
        let start = element.pos;
        let end = element.pos + element.size;
        let (c1, c2) = straight_line_controls(start, end);
        Some(CubicBezier {
            p0: start,
            c1,
            c2,
            p3: end,
        })
    })
}

fn straight_line_controls(start: Vec2, end: Vec2) -> (Vec2, Vec2) {
    let delta = end - start;
    (start + delta / 3.0, start + delta * (2.0 / 3.0))
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

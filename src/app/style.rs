use crate::board::{Element, ShapeType, ElementStyleSnapshot};
use crate::ui::property_panel::{ColorTarget, LineArrowTarget, WidthTarget};

pub fn tabs(
    show_text: bool,
    show_fill: bool,
    show_stroke: bool,
) -> [Option<ColorTarget>; 3] {
    [
        show_text.then_some(ColorTarget::Text),
        show_fill.then_some(ColorTarget::Fill),
        show_stroke.then_some(ColorTarget::Stroke),
    ]
}

pub fn title_for_selection(selected: &[&Element]) -> &'static str {
    let first_shape = selected.first().map(|element| element.shape);
    if selected
        .iter()
        .all(|element| Some(element.shape) == first_shape)
    {
        match first_shape {
            Some(ShapeType::Rect) => "RECT",
            Some(ShapeType::Ellipse) => "ELPS",
            Some(ShapeType::Line) => "LINE",
            Some(ShapeType::Image) | None => "MIX",
        }
    } else {
        "MIX"
    }
}

pub fn color_for_selection(selected: &[&Element], target: ColorTarget) -> [f32; 4] {
    let Some(first) = selected.first() else {
        return crate::palette::BLACK;
    };

    match target {
        ColorTarget::Text => first.current_text_color().unwrap_or(crate::board::DEFAULT_TEXT_COLOR),
        ColorTarget::Fill => first.color,
        ColorTarget::Stroke => first.effective_stroke_color(),
    }
}

pub fn color_for_box_defaults(style: crate::board::BoxToolStyle, target: ColorTarget) -> [f32; 4] {
    match target {
        ColorTarget::Text => style.text_color,
        ColorTarget::Fill => style.fill_color,
        ColorTarget::Stroke => style.stroke_color,
    }
}

pub fn apply_box_color(style: &mut crate::board::BoxToolStyle, target: ColorTarget, color: [f32; 4]) {
    match target {
        ColorTarget::Text => style.text_color = color,
        ColorTarget::Fill => style.fill_color = color,
        ColorTarget::Stroke => style.stroke_color = color,
    }
}

pub fn updated_style_with_color(
    element: &Element,
    target: ColorTarget,
    color: [f32; 4],
) -> Option<ElementStyleSnapshot> {
    let mut after = element.style_snapshot();
    match target {
        ColorTarget::Text if element.can_host_text() => after.text_color = Some(color),
        ColorTarget::Fill if matches!(element.shape, ShapeType::Rect | ShapeType::Ellipse) => {
            after.fill_color = color;
        }
        ColorTarget::Stroke
            if matches!(element.shape, ShapeType::Rect | ShapeType::Ellipse | ShapeType::Line) =>
        {
            after.stroke_color = color;
            if element.shape == ShapeType::Line {
                after.fill_color = color;
            }
        }
        _ => return None,
    }
    Some(after)
}

pub fn updated_style_with_width(
    element: &Element,
    target: WidthTarget,
    width: u8,
) -> Option<ElementStyleSnapshot> {
    let mut after = element.style_snapshot();
    match target {
        WidthTarget::Border if element.uses_border_width() => {
            after.border_width = Some(width.clamp(0, 16));
        }
        WidthTarget::Stroke if element.uses_stroke_width() => {
            after.stroke_width = Some(width.clamp(1, 16));
        }
        _ => return None,
    }
    Some(after)
}

pub fn updated_style_with_arrow(
    element: &Element,
    target: LineArrowTarget,
    enabled: bool,
) -> Option<ElementStyleSnapshot> {
    if element.shape != ShapeType::Line {
        return None;
    }

    let mut after = element.style_snapshot();
    match target {
        LineArrowTarget::Start => after.line_arrow_start = Some(enabled),
        LineArrowTarget::End => after.line_arrow_end = Some(enabled),
    }
    Some(after)
}

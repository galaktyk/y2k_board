use glam::Vec2;

use super::geometry::{line_curve, sample_cubic};
use super::*;

#[test]
fn text_bounds_keep_inner_box_centered() {
    let element = Element {
        id: 1,
        shape: ShapeType::Rect,
        kind: ElementKind::Generic,
        pos: Vec2::new(100.0, 50.0),
        size: Vec2::new(200.0, 120.0),
        rotation: 0.4,
        color: [0.0, 0.0, 0.0, 0.0],
        stroke_color: default_stroke_color(),
        border_width: default_border_width(),
        stroke_width: default_line_stroke_width(),
        line_arrow_start: false,
        line_arrow_end: false,
        line_bend: 0.0,
        line_midpoint_shift: 0.0,
        line_start_normal: None,
        line_end_normal: None,
        selected: false,
        text: Some(TextData::default()),
        image: None,
        text_layout_generation: 0,
    };

    let (min, max) = element.text_bounds().unwrap();
    let inner_center = (min + max) * 0.5;

    assert_eq!(min, Vec2::new(112.0, 62.0));
    assert_eq!(max, Vec2::new(288.0, 158.0));
    assert_eq!(inner_center, element.pos + element.size * 0.5);
}

#[test]
fn bring_to_front_works() {
    let mut board = Board::new();
    board.elements = vec![
        Element {
            id: 1,
            shape: ShapeType::Rect,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(10.0),
            rotation: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
        Element {
            id: 2,
            shape: ShapeType::Image,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(10.0),
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: Some(ImageData {
                asset_path: "img.webp".to_string(),
                hires_asset_path: None,
                original_width: 10,
                original_height: 10,
                base_width: 10,
                base_height: 10,
            }),
            text_layout_generation: 0,
        },
        Element {
            id: 3,
            shape: ShapeType::Ellipse,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(10.0),
            rotation: 0.0,
            color: [0.0, 1.0, 0.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
    ];

    assert!(board.bring_to_front(1));
    assert_eq!(
        board
            .elements
            .iter()
            .map(|element| element.id)
            .collect::<Vec<_>>(),
        vec![2, 3, 1]
    );
}

#[test]
fn hit_test_prioritizes_shape_layer_over_images() {
    let mut board = Board::new();
    board.elements = vec![
        Element {
            id: 1,
            shape: ShapeType::Image,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: Some(ImageData {
                asset_path: "img.webp".to_string(),
                hires_asset_path: None,
                original_width: 20,
                original_height: 20,
                base_width: 20,
                base_height: 20,
            }),
            text_layout_generation: 0,
        },
        Element {
            id: 2,
            shape: ShapeType::Rect,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
    ];

    assert_eq!(board.hit_test(Vec2::new(10.0, 10.0)), Some(2));
}

#[test]
fn hit_test_uses_board_order_within_shape_layer() {
    let mut board = Board::new();
    board.elements = vec![
        Element {
            id: 1,
            shape: ShapeType::Rect,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
        Element {
            id: 2,
            shape: ShapeType::Ellipse,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [0.0, 1.0, 0.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
    ];

    assert_eq!(board.hit_test(Vec2::new(10.0, 10.0)), Some(2));
}

#[test]
fn hit_test_prioritizes_text_elements_over_images() {
    let mut board = Board::new();
    board.elements = vec![
        Element {
            id: 1,
            shape: ShapeType::Image,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: Some(ImageData {
                asset_path: "img.webp".to_string(),
                hires_asset_path: None,
                original_width: 20,
                original_height: 20,
                base_width: 20,
                base_height: 20,
            }),
            text_layout_generation: 0,
        },
        Element {
            id: 2,
            shape: ShapeType::Rect,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: Some(TextData {
                content: "hello".to_string(),
                font_size: 24.0,
                color: DEFAULT_TEXT_COLOR,
            }),
            image: None,
            text_layout_generation: 0,
        },
    ];

    assert_eq!(board.hit_test(Vec2::new(10.0, 10.0)), Some(2));
}

#[test]
fn set_property_can_skip_connected_line_sync() {
    let mut board = Board::new();
    board.elements = vec![
        Element {
            id: 1,
            shape: ShapeType::Rect,
            kind: ElementKind::Generic,
            pos: Vec2::ZERO,
            size: Vec2::splat(20.0),
            rotation: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
        Element {
            id: 2,
            shape: ShapeType::Line,
            kind: ElementKind::Generic,
            pos: Vec2::new(20.0, 10.0),
            size: Vec2::new(40.0, 0.0),
            rotation: 0.0,
            color: DEFAULT_LINE_COLOR,
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        },
    ];
    board.line_attachments.insert(
        2,
        LineEndpoints {
            start: Some(LineAnchor {
                target_id: 1,
                norm_pos: Vec2::new(1.0, 0.5),
            }),
            end: None,
        },
    );
    board.connected_lines.insert(1, vec![2]);

    board.execute(&BoardOperation::SetProperty {
        changes: vec![ElementPropertyChange {
            id: 1,
            patch: ElementPropertyPatch::Transform {
                before: ElementTransform::new(Vec2::ZERO, Vec2::splat(20.0), 0.0),
                after: ElementTransform::new(Vec2::new(100.0, 0.0), Vec2::splat(20.0), 0.0),
            },
        }],
        sync_connected_lines: false,
    });

    let line = board.element(2).unwrap();
    assert_eq!(line.pos, Vec2::new(20.0, 10.0));
    assert_eq!(line.size, Vec2::new(40.0, 0.0));
}

#[test]
fn transform_related_ids_can_filter_connected_lines_by_visibility() {
    let mut board = Board::new();
    board.connected_lines.insert(10, vec![20, 21]);

    let visible_ids = std::collections::HashSet::from([21_u64]);

    assert_eq!(board.transform_related_ids([10]), vec![10, 20, 21]);
    assert_eq!(
        board.transform_related_ids_filtered([10], Some(&visible_ids)),
        vec![10, 21]
    );
}

#[test]
fn compute_drag_line_previews_tracks_resized_targets() {
    let mut board = Board::new();
    board.elements = vec![
        rect_element(1, Vec2::ZERO, Vec2::new(20.0, 20.0)),
        line_element(2, Vec2::new(20.0, 10.0), Vec2::new(40.0, 0.0)),
    ];
    board.line_attachments.insert(
        2,
        LineEndpoints {
            start: Some(LineAnchor {
                target_id: 1,
                norm_pos: Vec2::new(1.0, 0.5),
            }),
            end: None,
        },
    );
    board.connected_lines.insert(1, vec![2]);

    let selected_ids = std::collections::HashSet::from([1_u64]);
    let preview_transforms = std::collections::HashMap::from([(
        1_u64,
        ElementTransform::new(Vec2::ZERO, Vec2::new(40.0, 20.0), 0.0),
    )]);

    let previews = board.compute_drag_line_previews(&selected_ids, &preview_transforms);

    assert_eq!(
        previews,
        vec![(2, Vec2::new(40.0, 10.0), Vec2::new(20.0, 0.0))]
    );
}

#[test]
fn compute_drag_line_previews_tracks_rotated_targets() {
    let mut board = Board::new();
    board.elements = vec![
        rect_element(1, Vec2::ZERO, Vec2::new(20.0, 20.0)),
        line_element(2, Vec2::new(20.0, 10.0), Vec2::new(40.0, 0.0)),
    ];
    board.line_attachments.insert(
        2,
        LineEndpoints {
            start: Some(LineAnchor {
                target_id: 1,
                norm_pos: Vec2::new(1.0, 0.5),
            }),
            end: None,
        },
    );
    board.connected_lines.insert(1, vec![2]);

    let selected_ids = std::collections::HashSet::from([1_u64]);
    let preview_transforms = std::collections::HashMap::from([(
        1_u64,
        ElementTransform::new(
            Vec2::ZERO,
            Vec2::new(20.0, 20.0),
            std::f32::consts::FRAC_PI_2,
        ),
    )]);

    let previews = board.compute_drag_line_previews(&selected_ids, &preview_transforms);

    assert_eq!(
        previews,
        vec![(2, Vec2::new(10.0, 20.0), Vec2::new(50.0, -10.0))]
    );
}

#[test]
fn line_style_snapshot_round_trip_preserves_arrowheads() {
    let mut line = line_element(1, Vec2::ZERO, Vec2::new(120.0, 0.0));
    line.line_arrow_start = true;
    line.line_arrow_end = false;
    line.stroke_width = 5;

    let snapshot = line.style_snapshot();

    line.line_arrow_start = false;
    line.line_arrow_end = true;
    line.stroke_width = 1;
    line.apply_style_snapshot(snapshot);

    assert!(line.line_arrow_start);
    assert!(!line.line_arrow_end);
    assert_eq!(line.stroke_width, 5);
}

#[test]
fn line_curve_handle_round_trips_bend() {
    let mut line = line_element(1, Vec2::ZERO, Vec2::new(120.0, 0.0));
    line.line_bend = 36.0;

    let handle = line_bend_handle_position(&line);
    let offset = line_curve_handle_offset_from_handle(&line, handle);

    assert!(offset.x.abs() < 0.001);
    assert!((offset.y - 36.0).abs() < 0.001);
}

#[test]
fn line_curve_handle_stays_on_rendered_curve() {
    let mut line = line_element(1, Vec2::ZERO, Vec2::new(120.0, 20.0));
    line.line_start_normal = Some(Vec2::new(1.0, 0.0));
    line.line_end_normal = Some(Vec2::new(0.0, -1.0));
    line.line_bend = 28.0;

    let curve = line_curve(&line).unwrap();
    let handle = line_bend_handle_position(&line);

    assert!((handle - sample_cubic(curve, 0.5)).length() < 0.001);
}

#[test]
fn line_curve_handle_can_shift_along_chord() {
    let mut line = line_element(1, Vec2::ZERO, Vec2::new(120.0, 0.0));
    let target_handle = Vec2::new(78.0, 24.0);
    let offset = line_curve_handle_offset_from_handle(&line, target_handle);
    line.line_midpoint_shift = offset.x;
    line.line_bend = offset.y;

    let curve = line_curve(&line).unwrap();
    let handle = line_bend_handle_position(&line);

    assert!(line.line_midpoint_shift > 0.0);
    assert!((handle - target_handle).length() < 0.001);
    assert!((handle - sample_cubic(curve, 0.5)).length() < 0.001);
}

#[test]
fn curved_line_aabb_expands_beyond_chord() {
    let mut line = line_element(1, Vec2::ZERO, Vec2::new(120.0, 0.0));
    line.line_bend = 48.0;

    let (min, max) = line.aabb();

    assert!(min.y < 0.0);
    assert!(max.y > 0.0);
    assert!(max.x >= 120.0);
}

#[test]
fn connected_line_updates_store_face_normals() {
    let mut board = Board::new();
    board.elements = vec![
        rect_element(1, Vec2::ZERO, Vec2::new(20.0, 20.0)),
        line_element(2, Vec2::new(20.0, 10.0), Vec2::new(40.0, 0.0)),
    ];
    board.line_attachments.insert(
        2,
        LineEndpoints {
            start: Some(LineAnchor {
                target_id: 1,
                norm_pos: Vec2::new(1.0, 0.5),
            }),
            end: None,
        },
    );
    board.connected_lines.insert(1, vec![2]);

    board.update_connected_lines(1);

    let line = board.element(2).unwrap();
    assert_eq!(line.line_start_normal, Some(Vec2::new(1.0, 0.0)));
    assert_eq!(line.line_end_normal, None);
}

#[test]
fn short_connected_line_controls_do_not_overshoot_deeply() {
    let mut line = line_element(1, Vec2::ZERO, Vec2::new(40.0, 0.0));
    line.line_start_normal = Some(Vec2::new(1.0, 0.0));
    line.line_end_normal = Some(Vec2::new(0.0, -1.0));

    let curve = line_curve(&line).unwrap();

    assert!((curve.c1 - curve.p0).length() < 24.0);
    assert!((curve.c2 - curve.p3).length() < 24.0);
}

#[test]
fn connected_line_end_tangent_stays_outside_target() {
    let mut line = line_element(1, Vec2::new(-100.0, 0.0), Vec2::new(100.0, 0.0));
    line.line_end_normal = Some(Vec2::new(-1.0, 0.0));

    let curve = line_curve(&line).unwrap();
    let pre_end = sample_cubic(curve, 0.99);

    assert!(pre_end.x < curve.p3.x);
}

#[test]
fn opposite_facing_edge_controls_both_point_outward() {
    let mut line = line_element(1, Vec2::new(100.0, 40.0), Vec2::new(200.0, 0.0));
    line.line_start_normal = Some(Vec2::new(-1.0, 0.0));
    line.line_end_normal = Some(Vec2::new(1.0, 0.0));

    let curve = line_curve(&line).unwrap();

    assert!(curve.c1.x < curve.p0.x);
    assert!(curve.c2.x > curve.p3.x);
}

fn rect_element(id: u64, pos: Vec2, size: Vec2) -> Element {
    Element {
        id,
        shape: ShapeType::Rect,
        kind: ElementKind::Generic,
        pos,
        size,
        rotation: 0.0,
        color: [1.0, 0.0, 0.0, 1.0],
        stroke_color: default_stroke_color(),
        border_width: default_border_width(),
        stroke_width: default_line_stroke_width(),
        line_arrow_start: false,
        line_arrow_end: false,
        line_bend: 0.0,
        line_midpoint_shift: 0.0,
        line_start_normal: None,
        line_end_normal: None,
        selected: false,
        text: None,
        image: None,
        text_layout_generation: 0,
    }
}

fn line_element(id: u64, pos: Vec2, size: Vec2) -> Element {
    Element {
        id,
        shape: ShapeType::Line,
        kind: ElementKind::Generic,
        pos,
        size,
        rotation: 0.0,
        color: DEFAULT_LINE_COLOR,
        stroke_color: default_stroke_color(),
        border_width: default_border_width(),
        stroke_width: default_line_stroke_width(),
        line_arrow_start: false,
        line_arrow_end: false,
        line_bend: 0.0,
        line_midpoint_shift: 0.0,
        line_start_normal: None,
        line_end_normal: None,
        selected: false,
        text: None,
        image: None,
        text_layout_generation: 0,
    }
}

use glam::Vec2;

use crate::palette;
use crate::stats::emit_text;
use crate::renderer::InstanceData;

const PANEL_BG_COLOR: [f32; 4] = palette::GRAY_0;
const PANEL_HOVER_COLOR: [f32; 4] = palette::GRAY_1;
const PANEL_ACTIVE_COLOR: [f32; 4] = palette::GRAY_2;
const PANEL_BORDER_HIGHLIGHT: [f32; 4] = [239.0 / 255.0, 239.0 / 255.0, 239.0 / 255.0, 1.0];
const PANEL_BORDER_SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const PANEL_TEXT_COLOR: [f32; 4] = palette::BLACK;
const PANEL_SLIDER_FILL_COLOR: [f32; 4] = palette::BLUE_GRAY;
const PANEL_OUTER_MARGIN: f32 = 16.0;
const PANEL_PADDING: f32 = 10.0;
const PANEL_WIDTH: f32 = 126.0;
const TAB_HEIGHT: f32 = 24.0;
const TAB_GAP: f32 = 4.0;
const SWATCH_SIZE: f32 = 18.0;
const SWATCH_GAP: f32 = 4.0;
const SWATCH_COLUMNS: usize = 3;
const SLIDER_HEIGHT: f32 = 26.0;
const SLIDER_TRACK_HEIGHT: f32 = 6.0;
const SLIDER_KNOB_SIZE: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorTarget {
    Text,
    Fill,
    Stroke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthTarget {
    Border,
    Stroke,
}

impl WidthTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Border => "BORDER",
            Self::Stroke => "LINE",
        }
    }

    pub fn min_width(self) -> u8 {
        match self {
            Self::Border => 0,
            Self::Stroke => 1,
        }
    }
}

impl ColorTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "TXT",
            Self::Fill => "FIL",
            Self::Stroke => "BRD",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyPanelView {
    pub title: &'static str,
    pub tabs: [Option<ColorTarget>; 3],
    pub active_target: ColorTarget,
    pub active_color: [f32; 4],
    pub border_width: Option<u8>,
    pub stroke_width: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropertyPanelHit {
    Tab(ColorTarget),
    Swatch(usize),
    Width(WidthTarget, u8),
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x < self.x + self.w
            && point.y >= self.y
            && point.y < self.y + self.h
    }
}

#[derive(Clone)]
struct PropertyPanelLayout {
    origin: Vec2,
    size: Vec2,
    title_rect: Rect,
    tab_rects: Vec<(ColorTarget, Rect)>,
    swatch_rects: Vec<Rect>,
    width_tracks: Vec<(WidthTarget, Rect)>,
}

pub fn first_available_target(tabs: [Option<ColorTarget>; 3]) -> Option<ColorTarget> {
    tabs.into_iter().flatten().next()
}

pub fn contains_point(screen_size: Vec2, view: &PropertyPanelView, x: f32, y: f32) -> bool {
    let layout = layout(screen_size, view);
    x >= layout.origin.x
        && x < layout.origin.x + layout.size.x
        && y >= layout.origin.y
        && y < layout.origin.y + layout.size.y
}

pub fn hit_test(screen_size: Vec2, view: &PropertyPanelView, x: f32, y: f32) -> Option<PropertyPanelHit> {
    let point = Vec2::new(x, y);
    let layout = layout(screen_size, view);

    for (target, rect) in &layout.tab_rects {
        if rect.contains(point) {
            return Some(PropertyPanelHit::Tab(*target));
        }
    }

    for (index, rect) in layout.swatch_rects.iter().enumerate() {
        if rect.contains(point) {
            return Some(PropertyPanelHit::Swatch(index));
        }
    }

    for (target, rect) in &layout.width_tracks {
        if rect.contains(point) {
            return Some(PropertyPanelHit::Width(*target, width_from_track(*rect, x, *target)));
        }
    }

    None
}

pub fn build_instances(screen_size: Vec2, view: &PropertyPanelView, mouse_pos: Vec2) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let layout = layout(screen_size, view);
    let hovered = hit_test(screen_size, view, mouse_pos.x, mouse_pos.y);

    out.push(InstanceData::new(
        layout.origin.to_array(),
        layout.size.to_array(),
        0.0,
        PANEL_BG_COLOR,
        0.0,
        1.0,
    ));
    out.push(InstanceData::new(
        layout.origin.to_array(),
        [layout.size.x, 1.0],
        0.0,
        PANEL_BORDER_HIGHLIGHT,
        0.0,
        1.0,
    ));
    out.push(InstanceData::new(
        layout.origin.to_array(),
        [1.0, layout.size.y],
        0.0,
        PANEL_BORDER_HIGHLIGHT,
        0.0,
        1.0,
    ));
    out.push(InstanceData::new(
        [layout.origin.x, layout.origin.y + layout.size.y - 1.0],
        [layout.size.x, 1.0],
        0.0,
        PANEL_BORDER_SHADOW,
        0.0,
        1.0,
    ));
    out.push(InstanceData::new(
        [layout.origin.x + layout.size.x - 1.0, layout.origin.y],
        [1.0, layout.size.y],
        0.0,
        PANEL_BORDER_SHADOW,
        0.0,
        1.0,
    ));

    emit_text(
        view.title,
        layout.title_rect.x,
        layout.title_rect.y,
        2.0,
        PANEL_TEXT_COLOR,
        &mut out,
    );

    for (target, rect) in &layout.tab_rects {
        let is_active = *target == view.active_target;
        let is_hovered = hovered == Some(PropertyPanelHit::Tab(*target));
        let background = if is_active {
            PANEL_ACTIVE_COLOR
        } else if is_hovered {
            PANEL_HOVER_COLOR
        } else {
            PANEL_BG_COLOR
        };

        out.push(InstanceData::new(
            [rect.x, rect.y],
            [rect.w, rect.h],
            0.0,
            background,
            0.0,
            1.0,
        ));

        let border = if is_active { view.active_color } else { PANEL_BORDER_SHADOW };
        out.push(InstanceData::new(
            [rect.x, rect.y + rect.h - 2.0],
            [rect.w, 2.0],
            0.0,
            border,
            0.0,
            1.0,
        ));

        let label = target.label();
        let label_w = label.len() as f32 * 8.0 - 2.0;
        emit_text(
            label,
            rect.x + (rect.w - label_w) * 0.5,
            rect.y + 7.0,
            2.0,
            PANEL_TEXT_COLOR,
            &mut out,
        );
    }

    let selected_color_index = palette::PALETTE
        .iter()
        .position(|color| *color == view.active_color);

    for (index, rect) in layout.swatch_rects.iter().enumerate() {
        let color = palette::PALETTE[index];
        let is_hovered = hovered == Some(PropertyPanelHit::Swatch(index));
        out.push(InstanceData::new(
            [rect.x, rect.y],
            [rect.w, rect.h],
            0.0,
            color,
            0.0,
            1.0,
        ));

        let border = if Some(index) == selected_color_index {
            PANEL_BORDER_SHADOW
        } else if is_hovered {
            PANEL_HOVER_COLOR
        } else {
            PANEL_BG_COLOR
        };
        out.push(InstanceData::new(
            [rect.x - 1.0, rect.y - 1.0],
            [rect.w + 2.0, 1.0],
            0.0,
            border,
            0.0,
            1.0,
        ));
        out.push(InstanceData::new(
            [rect.x - 1.0, rect.y + rect.h],
            [rect.w + 2.0, 1.0],
            0.0,
            border,
            0.0,
            1.0,
        ));
        out.push(InstanceData::new(
            [rect.x - 1.0, rect.y - 1.0],
            [1.0, rect.h + 2.0],
            0.0,
            border,
            0.0,
            1.0,
        ));
        out.push(InstanceData::new(
            [rect.x + rect.w, rect.y - 1.0],
            [1.0, rect.h + 2.0],
            0.0,
            border,
            0.0,
            1.0,
        ));
    }

    for (target, track) in &layout.width_tracks {
        let Some(width) = width_for(view, *target) else {
            continue;
        };
        let is_hovered = matches!(hovered, Some(PropertyPanelHit::Width(hover_target, _)) if hover_target == *target);
            out.push(InstanceData::new(
                [track.x, track.y],
                [track.w, track.h],
                0.0,
                PANEL_ACTIVE_COLOR,
                0.0,
                1.0,
            ));

            let min_width = f32::from(target.min_width());
            let fill = (f32::from(width.clamp(target.min_width(), 16)) - min_width)
                / (16.0 - min_width).max(0.0001);
            out.push(InstanceData::new(
                [track.x, track.y],
                [track.w * fill, track.h],
                0.0,
                PANEL_SLIDER_FILL_COLOR,
                0.0,
                1.0,
            ));

            let knob_x = track.x + track.w * fill - SLIDER_KNOB_SIZE * 0.5;
            let knob_color = if is_hovered { PANEL_HOVER_COLOR } else { PANEL_BORDER_HIGHLIGHT };
            out.push(InstanceData::new(
                [knob_x, track.y - (SLIDER_KNOB_SIZE - track.h) * 0.5],
                [SLIDER_KNOB_SIZE, SLIDER_KNOB_SIZE],
                0.0,
                knob_color,
                0.0,
                1.0,
            ));

            let width_label = format!("{} {}PX", target.label(), width);
            emit_text(
                &width_label,
                track.x,
                track.y - 16.0,
                2.0,
                PANEL_TEXT_COLOR,
                &mut out,
            );
    }

    out
}

fn layout(screen_size: Vec2, view: &PropertyPanelView) -> PropertyPanelLayout {
    let mut y = PANEL_PADDING;
    let title_rect = Rect {
        x: PANEL_PADDING,
        y,
        w: PANEL_WIDTH - PANEL_PADDING * 2.0,
        h: 10.0,
    };
    y += 18.0;

    let tabs: Vec<ColorTarget> = view.tabs.into_iter().flatten().collect();
    let tab_width = if tabs.is_empty() {
        0.0
    } else {
        (PANEL_WIDTH - PANEL_PADDING * 2.0 - TAB_GAP * (tabs.len().saturating_sub(1)) as f32)
            / tabs.len() as f32
    };
    let mut tab_rects = Vec::with_capacity(tabs.len());
    for (index, target) in tabs.iter().enumerate() {
        tab_rects.push((
            *target,
            Rect {
                x: PANEL_PADDING + index as f32 * (tab_width + TAB_GAP),
                y,
                w: tab_width,
                h: TAB_HEIGHT,
            },
        ));
    }
    y += TAB_HEIGHT + 10.0;

    let rows = palette::PALETTE.len().div_ceil(SWATCH_COLUMNS);
    let grid_width = SWATCH_COLUMNS as f32 * SWATCH_SIZE + (SWATCH_COLUMNS as f32 - 1.0) * SWATCH_GAP;
    let grid_x = PANEL_PADDING + (PANEL_WIDTH - PANEL_PADDING * 2.0 - grid_width) * 0.5;
    let mut swatch_rects = Vec::with_capacity(palette::PALETTE.len());
    for index in 0..palette::PALETTE.len() {
        let row = index / SWATCH_COLUMNS;
        let col = index % SWATCH_COLUMNS;
        swatch_rects.push(Rect {
            x: grid_x + col as f32 * (SWATCH_SIZE + SWATCH_GAP),
            y: y + row as f32 * (SWATCH_SIZE + SWATCH_GAP),
            w: SWATCH_SIZE,
            h: SWATCH_SIZE,
        });
    }
    y += rows as f32 * (SWATCH_SIZE + SWATCH_GAP) - SWATCH_GAP;

    let mut width_tracks = Vec::new();
    for target in [WidthTarget::Border, WidthTarget::Stroke] {
        if width_for(view, target).is_none() {
            continue;
        }

        y += 22.0;
        width_tracks.push((
            target,
            Rect {
                x: PANEL_PADDING,
                y,
                w: PANEL_WIDTH - PANEL_PADDING * 2.0,
                h: SLIDER_TRACK_HEIGHT,
            },
        ));
        y += SLIDER_HEIGHT;
    }

    let size = Vec2::new(PANEL_WIDTH, y + PANEL_PADDING);
    let origin = Vec2::new(
        (screen_size.x - PANEL_WIDTH - PANEL_OUTER_MARGIN).max(0.0),
        ((screen_size.y - size.y) * 0.5).max(PANEL_OUTER_MARGIN),
    );

    for (_, rect) in &mut tab_rects {
        rect.x += origin.x;
        rect.y += origin.y;
    }
    for rect in &mut swatch_rects {
        rect.x += origin.x;
        rect.y += origin.y;
    }

    for (_, rect) in &mut width_tracks {
        rect.x += origin.x;
        rect.y += origin.y;
    }

    PropertyPanelLayout {
        origin,
        size,
        title_rect: Rect {
            x: origin.x + title_rect.x,
            y: origin.y + title_rect.y,
            w: title_rect.w,
            h: title_rect.h,
        },
        tab_rects,
        swatch_rects,
        width_tracks,
    }
}

fn width_for(view: &PropertyPanelView, target: WidthTarget) -> Option<u8> {
    match target {
        WidthTarget::Border => view.border_width,
        WidthTarget::Stroke => view.stroke_width,
    }
}

fn width_from_track(track: Rect, x: f32, target: WidthTarget) -> u8 {
    let t = ((x - track.x) / track.w).clamp(0.0, 1.0);
    let min_width = f32::from(target.min_width());
    (min_width + (16.0 - min_width) * t).round().clamp(min_width, 16.0) as u8
}

pub fn width_at_x(screen_size: Vec2, view: &PropertyPanelView, target: WidthTarget, x: f32) -> Option<u8> {
    layout(screen_size, view)
        .width_tracks
        .into_iter()
        .find_map(|(track_target, track)| (track_target == target).then(|| width_from_track(track, x, target)))
}
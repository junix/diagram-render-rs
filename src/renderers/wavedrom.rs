use std::collections::BTreeMap;

use diagram_ast_parser::ast::wavedrom::{
    WaveDromDocument, WaveLane, WaveRegisterField, WaveSignalItem, WaveTimingDiagram,
};

use super::RenderPlan;
use super::cards::{push_text, truncate};
use crate::Theme;
use crate::scene::{Point, Primitive, Rect, Scene, Stroke, TextAnchor, TextWeight};

const LABEL_WIDTH: f32 = 178.0;
const CELL_WIDTH: f32 = 46.0;
const LANE_HEIGHT: f32 = 58.0;

struct LaneView<'a> {
    group: Option<String>,
    lane: &'a WaveLane,
}

pub(crate) fn render(document: &WaveDromDocument, theme: &Theme) -> RenderPlan {
    let mut lanes = Vec::new();
    if let Some(timing) = &document.timing {
        flatten_lanes(&timing.signal, None, &mut lanes);
    }
    let max_cells = lanes
        .iter()
        .filter_map(|view| view.lane.wave.as_ref().map(String::len))
        .max()
        .unwrap_or(8)
        .max(4);
    let waveform_width = max_cells as f32 * CELL_WIDTH;
    let width = (LABEL_WIDTH + waveform_width + 64.0).max(660.0);
    let timing_height = if lanes.is_empty() {
        0.0
    } else {
        82.0 + lanes.len() as f32 * LANE_HEIGHT + 38.0
    };
    let register_height = document.register.as_ref().map_or(0.0, |register| {
        128.0 + register.fields.len().min(3) as f32 * 8.0
    });
    let height = (timing_height + register_height + 30.0).max(220.0);
    let title = document
        .timing
        .as_ref()
        .and_then(|timing| timing.head.as_ref())
        .and_then(|head| head.text.clone())
        .unwrap_or_else(|| "WaveDrom timing and register diagram".to_owned());
    let mut scene = Scene::new(width, height, &title);
    push_text(
        &mut scene,
        Point::new(34.0, 34.0),
        &title,
        23.0,
        TextAnchor::Start,
        &theme.foreground,
        TextWeight::Bold,
    );
    let mut warnings = Vec::new();
    let mut nodes = BTreeMap::new();
    if let Some(timing) = &document.timing {
        draw_timing(&mut scene, timing, &lanes, max_cells, theme, &mut nodes);
        draw_edges(&mut scene, &timing.edges, &nodes, theme, &mut warnings);
    }
    if let Some(register) = &document.register {
        draw_register(
            &mut scene,
            &register.fields,
            timing_height.max(74.0),
            width,
            theme,
        );
    }
    RenderPlan { scene, warnings }
}

fn flatten_lanes<'a>(
    items: &'a [WaveSignalItem],
    group: Option<&str>,
    output: &mut Vec<LaneView<'a>>,
) {
    for item in items {
        match item {
            WaveSignalItem::Lane(lane) => output.push(LaneView {
                group: group.map(str::to_owned),
                lane,
            }),
            WaveSignalItem::Group(nested) => {
                flatten_lanes(&nested.items, Some(&nested.label), output);
            }
        }
    }
}

fn draw_timing(
    scene: &mut Scene,
    timing: &WaveTimingDiagram,
    lanes: &[LaneView<'_>],
    max_cells: usize,
    theme: &Theme,
    nodes: &mut BTreeMap<char, Point>,
) {
    let x0 = LABEL_WIDTH;
    for cell in 0..=max_cells {
        let x = x0 + cell as f32 * CELL_WIDTH;
        scene.push(Primitive::Line {
            from: Point::new(x, 68.0),
            to: Point::new(x, 78.0 + lanes.len() as f32 * LANE_HEIGHT),
            stroke: Stroke::solid(theme.grid.clone(), 0.8),
        });
        if cell < max_cells {
            push_text(
                scene,
                Point::new(x + CELL_WIDTH / 2.0, 69.0),
                &cell.to_string(),
                10.0,
                TextAnchor::Middle,
                &theme.muted,
                TextWeight::Normal,
            );
        }
    }
    for (index, view) in lanes.iter().enumerate() {
        let y = 96.0 + index as f32 * LANE_HEIGHT;
        if let Some(group) = &view.group {
            push_text(
                scene,
                Point::new(18.0, y - 10.0),
                &truncate(group, 13),
                9.5,
                TextAnchor::Start,
                &theme.accent,
                TextWeight::Bold,
            );
        }
        push_text(
            scene,
            Point::new(18.0, y + 7.0),
            &truncate(view.lane.name.as_deref().unwrap_or("signal"), 19),
            13.0,
            TextAnchor::Start,
            &theme.foreground,
            TextWeight::Bold,
        );
        scene.push(Primitive::Line {
            from: Point::new(x0, y),
            to: Point::new(x0 + max_cells as f32 * CELL_WIDTH, y),
            stroke: Stroke::solid(theme.grid.clone(), 1.0),
        });
        draw_lane(scene, view.lane, x0, y, max_cells, theme, nodes);
    }
    if let Some(foot) = &timing.foot
        && let Some(text) = &foot.text
    {
        push_text(
            scene,
            Point::new(x0, 96.0 + lanes.len() as f32 * LANE_HEIGHT),
            text,
            11.0,
            TextAnchor::Start,
            &theme.muted,
            TextWeight::Normal,
        );
    }
}

fn draw_lane(
    scene: &mut Scene,
    lane: &WaveLane,
    x0: f32,
    y: f32,
    max_cells: usize,
    theme: &Theme,
    nodes: &mut BTreeMap<char, Point>,
) {
    let wave = lane.wave.as_deref().unwrap_or("");
    let mut level = y + 13.0;
    let mut clock = false;
    let mut data_index = 0usize;
    for (index, symbol) in wave.chars().take(max_cells).enumerate() {
        let left = x0 + index as f32 * CELL_WIDTH;
        let right = left + CELL_WIDTH;
        match symbol {
            '0' | 'l' | 'L' => {
                clock = false;
                draw_level(scene, left, right, &mut level, y + 13.0, theme);
            }
            '1' | 'h' | 'H' => {
                clock = false;
                draw_level(scene, left, right, &mut level, y - 13.0, theme);
            }
            'p' | 'P' | 'n' | 'N' => {
                clock = true;
                draw_clock(scene, left, right, y, symbol == 'n' || symbol == 'N', theme);
                level = y;
            }
            '.' if clock => draw_clock(scene, left, right, y, false, theme),
            '.' => {
                let current = level;
                draw_level(scene, left, right, &mut level, current, theme);
            }
            'x' | 'X' => {
                clock = false;
                scene.push(Primitive::Rect {
                    rect: Rect::new(left + 1.0, y - 14.0, CELL_WIDTH - 2.0, 28.0),
                    radius: 4.0,
                    fill: Some(theme.danger.clone()),
                    fill_opacity: 0.14,
                    stroke: Some(Stroke::solid(theme.danger.clone(), 1.2)),
                });
                scene.push(Primitive::Line {
                    from: Point::new(left + 5.0, y - 10.0),
                    to: Point::new(right - 5.0, y + 10.0),
                    stroke: Stroke::solid(theme.danger.clone(), 1.0),
                });
                scene.push(Primitive::Line {
                    from: Point::new(left + 5.0, y + 10.0),
                    to: Point::new(right - 5.0, y - 10.0),
                    stroke: Stroke::solid(theme.danger.clone(), 1.0),
                });
                level = y;
            }
            'z' | 'Z' => {
                clock = false;
                scene.push(Primitive::Line {
                    from: Point::new(left, y),
                    to: Point::new(right, y),
                    stroke: Stroke::dashed(theme.muted.clone(), 1.4),
                });
                level = y;
            }
            '=' | '2'..='9' => {
                clock = false;
                scene.push(Primitive::Rect {
                    rect: Rect::new(left + 1.0, y - 14.0, CELL_WIDTH - 2.0, 28.0),
                    radius: 6.0,
                    fill: Some(theme.accent_soft.clone()),
                    fill_opacity: 1.0,
                    stroke: Some(Stroke::solid(theme.accent.clone(), 1.2)),
                });
                if let Some(data) = lane.data.get(data_index) {
                    push_text(
                        scene,
                        Point::new(left + CELL_WIDTH / 2.0, y),
                        &truncate(data, 8),
                        10.0,
                        TextAnchor::Middle,
                        &theme.foreground,
                        TextWeight::Bold,
                    );
                    data_index += 1;
                }
                level = y;
            }
            _ => draw_level(scene, left, right, &mut level, y, theme),
        }
    }
    if let Some(node_text) = &lane.node {
        for (index, node) in node_text.chars().take(max_cells).enumerate() {
            if node == '.' || node.is_whitespace() {
                continue;
            }
            let point = Point::new(x0 + (index as f32 + 0.5) * CELL_WIDTH, y - 20.0);
            nodes.insert(node, point);
            scene.push(Primitive::Ellipse {
                center: point,
                radius_x: 7.0,
                radius_y: 7.0,
                fill: Some(theme.surface.clone()),
                fill_opacity: 1.0,
                stroke: Some(Stroke::solid(theme.accent.clone(), 1.2)),
            });
            push_text(
                scene,
                point,
                &node.to_string(),
                9.0,
                TextAnchor::Middle,
                &theme.accent,
                TextWeight::Bold,
            );
        }
    }
}

fn draw_level(scene: &mut Scene, left: f32, right: f32, level: &mut f32, next: f32, theme: &Theme) {
    scene.push(Primitive::Polyline {
        points: vec![
            Point::new(left, *level),
            Point::new(left, next),
            Point::new(right, next),
        ],
        stroke: Stroke::solid(theme.line.clone(), 2.0),
        fill: None,
    });
    *level = next;
}

fn draw_clock(scene: &mut Scene, left: f32, right: f32, y: f32, inverted: bool, theme: &Theme) {
    let high = if inverted { y + 13.0 } else { y - 13.0 };
    let low = if inverted { y - 13.0 } else { y + 13.0 };
    let middle = (left + right) / 2.0;
    scene.push(Primitive::Polyline {
        points: vec![
            Point::new(left, low),
            Point::new(left, high),
            Point::new(middle, high),
            Point::new(middle, low),
            Point::new(right, low),
        ],
        stroke: Stroke::solid(theme.line.clone(), 2.0),
        fill: None,
    });
}

fn draw_edges(
    scene: &mut Scene,
    edges: &[String],
    nodes: &BTreeMap<char, Point>,
    theme: &Theme,
    warnings: &mut Vec<String>,
) {
    for edge in edges {
        let labels: Vec<char> = edge
            .chars()
            .filter(|character| character.is_alphanumeric())
            .collect();
        let (Some(from_name), Some(to_name)) = (labels.first(), labels.get(1)) else {
            warnings.push(format!("WaveDrom edge `{edge}` has no two node labels"));
            continue;
        };
        let (Some(from), Some(to)) = (nodes.get(from_name).copied(), nodes.get(to_name).copied())
        else {
            warnings.push(format!("WaveDrom edge `{edge}` references an unknown node"));
            continue;
        };
        let control_y = from.y.min(to.y) - 22.0;
        scene.push(Primitive::Polyline {
            points: vec![
                from,
                Point::new(from.x, control_y),
                Point::new(to.x, control_y),
                to,
            ],
            stroke: Stroke::solid(theme.accent.clone(), 1.4),
            fill: None,
        });
        draw_arrowhead(scene, to, Point::new(to.x, control_y), &theme.accent);
    }
}

fn draw_register(
    scene: &mut Scene,
    fields: &[WaveRegisterField],
    top: f32,
    width: f32,
    theme: &Theme,
) {
    push_text(
        scene,
        Point::new(34.0, top + 28.0),
        "Register fields",
        17.0,
        TextAnchor::Start,
        &theme.foreground,
        TextWeight::Bold,
    );
    let total_bits = fields
        .iter()
        .map(|field| field.bits.unwrap_or(1))
        .sum::<u64>()
        .max(1);
    let available = width - 68.0;
    let mut x = 34.0;
    let y = top + 52.0;
    let mut high_bit = total_bits;
    for (index, field) in fields.iter().enumerate() {
        let bits = field.bits.unwrap_or(1).max(1);
        let field_width = if index + 1 == fields.len() {
            34.0 + available - x
        } else {
            available * bits as f32 / total_bits as f32
        };
        scene.push(Primitive::Rect {
            rect: Rect::new(x, y, field_width, 54.0),
            radius: 4.0,
            fill: Some(theme.series(index).to_owned()),
            fill_opacity: 0.15,
            stroke: Some(Stroke::solid(theme.series(index).to_owned(), 1.3)),
        });
        push_text(
            scene,
            Point::new(x + field_width / 2.0, y + 21.0),
            &truncate(field.name.as_deref().unwrap_or("reserved"), 16),
            11.0,
            TextAnchor::Middle,
            &theme.foreground,
            TextWeight::Bold,
        );
        let low_bit = high_bit.saturating_sub(bits);
        let range = if bits == 1 {
            low_bit.to_string()
        } else {
            format!("{}:{}", high_bit - 1, low_bit)
        };
        push_text(
            scene,
            Point::new(x + field_width / 2.0, y + 40.0),
            &range,
            9.5,
            TextAnchor::Middle,
            &theme.muted,
            TextWeight::Normal,
        );
        high_bit = low_bit;
        x += field_width;
    }
}

fn draw_arrowhead(scene: &mut Scene, tip: Point, previous: Point, color: &str) {
    let dx = tip.x - previous.x;
    let dy = tip.y - previous.y;
    let length = (dx * dx + dy * dy).sqrt().max(0.001);
    let ux = dx / length;
    let uy = dy / length;
    let base = Point::new(tip.x - ux * 9.0, tip.y - uy * 9.0);
    let perpendicular = Point::new(-uy * 4.0, ux * 4.0);
    scene.push(Primitive::Polygon {
        points: vec![
            tip,
            Point::new(base.x + perpendicular.x, base.y + perpendicular.y),
            Point::new(base.x - perpendicular.x, base.y - perpendicular.y),
        ],
        fill: Some(color.to_owned()),
        fill_opacity: 1.0,
        stroke: None,
    });
}

//! Shared visual layout for graph-like domain families.
//!
//! `CardDiagram` is deliberately a view/layout structure, not a semantic graph
//! model. Format modules resolve their own names, hierarchy, and relationship
//! meaning before producing these cards and connectors.

use std::collections::{BTreeMap, BTreeSet};

use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};

use crate::Theme;
use crate::scene::{Point, Primitive, Rect, Scene, Stroke, TextAnchor, TextWeight};

const CARD_WIDTH: f32 = 258.0;
const X_GAP: f32 = 92.0;
const Y_GAP: f32 = 82.0;
const MARGIN: f32 = 42.0;
const TITLE_HEIGHT: f32 = 72.0;

#[derive(Clone, Debug)]
pub(crate) struct Card {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub lines: Vec<String>,
    pub color_slot: usize,
}

impl Card {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        kind: impl Into<String>,
        lines: Vec<String>,
        color_slot: usize,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            kind: kind.into(),
            lines,
            color_slot,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ConnectorKind {
    #[default]
    Directed,
    Reverse,
    Bidirectional,
    Undirected,
}

#[derive(Clone, Debug)]
pub(crate) struct Connector {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub kind: ConnectorKind,
    pub dashed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CardDiagram {
    pub title: String,
    pub cards: Vec<Card>,
    pub connectors: Vec<Connector>,
}

impl CardDiagram {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            cards: Vec::new(),
            connectors: Vec::new(),
        }
    }

    pub fn push_card(&mut self, card: Card) {
        if let Some(existing) = self
            .cards
            .iter_mut()
            .find(|existing| existing.id == card.id)
        {
            if existing.lines.is_empty() && !card.lines.is_empty() {
                *existing = card;
            }
            return;
        }
        self.cards.push(card);
    }

    pub fn ensure_endpoint(&mut self, id: &str) {
        if self.cards.iter().any(|card| card.id == id) {
            return;
        }
        self.cards.push(Card::new(
            id,
            display_id(id),
            "REFERENCE",
            Vec::new(),
            self.cards.len(),
        ));
    }
}

pub(crate) fn render(diagram: &mut CardDiagram, theme: &Theme) -> Scene {
    deduplicate_cards(diagram);
    if diagram.cards.is_empty() {
        diagram.cards.push(Card::new(
            "empty",
            "Empty diagram",
            "NO ELEMENTS",
            vec!["The AST contains no renderable elements.".to_owned()],
            0,
        ));
    }

    let wrapped: Vec<Vec<String>> = diagram
        .cards
        .iter()
        .map(|card| card.lines.iter().flat_map(|line| wrap(line, 34)).collect())
        .collect();
    let heights: Vec<f32> = wrapped
        .iter()
        .map(|lines| 78.0 + lines.len().max(1) as f32 * 19.0)
        .collect();
    let columns = ((diagram.cards.len() as f32).sqrt().ceil() as usize).clamp(1, 4);
    let rows = diagram.cards.len().div_ceil(columns);
    let mut row_heights = vec![0.0_f32; rows];
    for (index, height) in heights.iter().copied().enumerate() {
        row_heights[index / columns] = row_heights[index / columns].max(height);
    }

    let width = MARGIN * 2.0 + columns as f32 * CARD_WIDTH + (columns - 1) as f32 * X_GAP;
    let height = TITLE_HEIGHT
        + MARGIN
        + row_heights.iter().sum::<f32>()
        + rows.saturating_sub(1) as f32 * Y_GAP
        + MARGIN;
    let mut scene = Scene::new(width, height, &diagram.title);
    push_text(
        &mut scene,
        Point::new(MARGIN, 34.0),
        &diagram.title,
        24.0,
        TextAnchor::Start,
        &theme.foreground,
        TextWeight::Bold,
    );
    push_text(
        &mut scene,
        Point::new(MARGIN, 56.0),
        "typed AST view",
        11.0,
        TextAnchor::Start,
        &theme.muted,
        TextWeight::Normal,
    );

    let mut positions = BTreeMap::new();
    let mut row_y = TITLE_HEIGHT;
    for (row, row_height) in row_heights.iter().copied().enumerate() {
        for column in 0..columns {
            let index = row * columns + column;
            if index >= diagram.cards.len() {
                break;
            }
            let rect = Rect::new(
                MARGIN + column as f32 * (CARD_WIDTH + X_GAP),
                row_y,
                CARD_WIDTH,
                heights[index],
            );
            positions.insert(diagram.cards[index].id.clone(), rect);
        }
        row_y += row_height + Y_GAP;
    }

    draw_connectors(&mut scene, &diagram.connectors, &positions, theme);
    for (index, card) in diagram.cards.iter().enumerate() {
        if let Some(rect) = positions.get(&card.id).copied() {
            draw_card(&mut scene, card, rect, &wrapped[index], theme);
        }
    }
    scene
}

fn deduplicate_cards(diagram: &mut CardDiagram) {
    let mut seen = BTreeSet::new();
    diagram.cards.retain(|card| seen.insert(card.id.clone()));
}

fn draw_connectors(
    scene: &mut Scene,
    connectors: &[Connector],
    positions: &BTreeMap<String, Rect>,
    theme: &Theme,
) {
    for connector in connectors {
        let (Some(from_rect), Some(to_rect)) = (
            positions.get(&connector.from).copied(),
            positions.get(&connector.to).copied(),
        ) else {
            continue;
        };
        let stroke = if connector.dashed {
            Stroke::dashed(theme.line.clone(), 1.6)
        } else {
            Stroke::solid(theme.line.clone(), 1.6)
        };
        let points = connector_points(from_rect, to_rect);
        scene.push(Primitive::Polyline {
            points: points.clone(),
            stroke,
            fill: None,
        });
        let start = points[0];
        let end = *points.last().unwrap_or(&start);
        let start_neighbor = points.get(1).copied().unwrap_or(end);
        let end_neighbor = points
            .get(points.len().saturating_sub(2))
            .copied()
            .unwrap_or(start);
        if matches!(
            connector.kind,
            ConnectorKind::Directed | ConnectorKind::Bidirectional
        ) {
            draw_arrowhead(scene, end, end_neighbor, &theme.line);
        }
        if matches!(
            connector.kind,
            ConnectorKind::Reverse | ConnectorKind::Bidirectional
        ) {
            draw_arrowhead(scene, start, start_neighbor, &theme.line);
        }
        if let Some(label) = connector.label.as_deref().filter(|label| !label.is_empty()) {
            let middle = polyline_middle(&points);
            let label = truncate(label, 34);
            let label_width = (label.width() as f32 * 7.1 + 18.0).clamp(46.0, 250.0);
            scene.push(Primitive::Rect {
                rect: Rect::new(
                    middle.x - label_width / 2.0,
                    middle.y - 12.0,
                    label_width,
                    24.0,
                ),
                radius: 8.0,
                fill: Some(theme.surface_alt.clone()),
                fill_opacity: 0.98,
                stroke: Some(Stroke::solid(theme.grid.clone(), 1.0)),
            });
            push_text(
                scene,
                middle,
                &label,
                11.5,
                TextAnchor::Middle,
                &theme.foreground,
                TextWeight::Normal,
            );
        }
    }
}

fn draw_card(scene: &mut Scene, card: &Card, rect: Rect, lines: &[String], theme: &Theme) {
    scene.push(Primitive::Rect {
        rect: Rect::new(rect.x + 4.0, rect.y + 5.0, rect.width, rect.height),
        radius: 12.0,
        fill: Some(theme.line.clone()),
        fill_opacity: 0.12,
        stroke: None,
    });
    scene.push(Primitive::Rect {
        rect,
        radius: 12.0,
        fill: Some(theme.surface.clone()),
        fill_opacity: 1.0,
        stroke: Some(Stroke::solid(theme.grid.clone(), 1.2)),
    });
    scene.push(Primitive::Rect {
        rect: Rect::new(rect.x, rect.y, rect.width, 56.0),
        radius: 12.0,
        fill: Some(theme.accent_soft.clone()),
        fill_opacity: 1.0,
        stroke: None,
    });
    scene.push(Primitive::Rect {
        rect: Rect::new(rect.x, rect.y, 6.0, rect.height),
        radius: 3.0,
        fill: Some(theme.series(card.color_slot).to_owned()),
        fill_opacity: 1.0,
        stroke: None,
    });
    push_text(
        scene,
        Point::new(rect.x + 18.0, rect.y + 19.0),
        &truncate(&card.kind.to_ascii_uppercase(), 28),
        10.5,
        TextAnchor::Start,
        &theme.muted,
        TextWeight::Bold,
    );
    push_text(
        scene,
        Point::new(rect.x + 18.0, rect.y + 40.0),
        &truncate(&card.title, 31),
        16.0,
        TextAnchor::Start,
        &theme.foreground,
        TextWeight::Bold,
    );
    for (index, line) in lines.iter().enumerate() {
        push_text(
            scene,
            Point::new(rect.x + 18.0, rect.y + 72.0 + index as f32 * 19.0),
            line,
            12.5,
            TextAnchor::Start,
            &theme.foreground,
            TextWeight::Normal,
        );
    }
}

fn connector_points(from: Rect, to: Rect) -> Vec<Point> {
    if from == to {
        let top = Point::new(from.x + from.width * 0.65, from.y);
        return vec![
            top,
            Point::new(top.x, top.y - 28.0),
            Point::new(from.x + from.width + 28.0, top.y - 28.0),
            Point::new(from.x + from.width + 28.0, from.y + from.height * 0.35),
            Point::new(from.x + from.width, from.y + from.height * 0.35),
        ];
    }
    let from_center = from.center();
    let to_center = to.center();
    let start = rect_boundary(from, to_center);
    let end = rect_boundary(to, from_center);
    if (start.x - end.x).abs() < 1.0 || (start.y - end.y).abs() < 1.0 {
        return vec![start, end];
    }
    let middle_x = (start.x + end.x) / 2.0;
    vec![
        start,
        Point::new(middle_x, start.y),
        Point::new(middle_x, end.y),
        end,
    ]
}

fn rect_boundary(rect: Rect, toward: Point) -> Point {
    let center = rect.center();
    let dx = toward.x - center.x;
    let dy = toward.y - center.y;
    if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        return center;
    }
    let scale_x = if dx.abs() < f32::EPSILON {
        f32::INFINITY
    } else {
        rect.width / 2.0 / dx.abs()
    };
    let scale_y = if dy.abs() < f32::EPSILON {
        f32::INFINITY
    } else {
        rect.height / 2.0 / dy.abs()
    };
    let scale = scale_x.min(scale_y);
    Point::new(center.x + dx * scale, center.y + dy * scale)
}

fn draw_arrowhead(scene: &mut Scene, tip: Point, previous: Point, color: &str) {
    let dx = tip.x - previous.x;
    let dy = tip.y - previous.y;
    let length = (dx * dx + dy * dy).sqrt().max(0.001);
    let ux = dx / length;
    let uy = dy / length;
    let base = Point::new(tip.x - ux * 11.0, tip.y - uy * 11.0);
    let perpendicular = Point::new(-uy * 5.0, ux * 5.0);
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

fn polyline_middle(points: &[Point]) -> Point {
    points.get(points.len() / 2).copied().unwrap_or_default()
}

pub(crate) fn push_text(
    scene: &mut Scene,
    at: Point,
    text: &str,
    size: f32,
    anchor: TextAnchor,
    color: &str,
    weight: TextWeight,
) {
    scene.push(Primitive::Text {
        at,
        text: text.to_owned(),
        size,
        anchor,
        color: color.to_owned(),
        weight,
        italic: false,
    });
}

pub(crate) fn truncate(value: &str, max_width: usize) -> String {
    if value.width() <= max_width {
        return value.to_owned();
    }
    let mut result = String::new();
    for character in value.chars() {
        if result.width() + character.width().unwrap_or(0) + 1 > max_width {
            break;
        }
        result.push(character);
    }
    result.push('…');
    result
}

pub(crate) fn wrap(value: &str, max_width: usize) -> Vec<String> {
    if value.trim().is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        let candidate_width = current.width() + usize::from(!current.is_empty()) + word.width();
        if candidate_width > max_width && !current.is_empty() {
            lines.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(truncate(&current, max_width));
    }
    lines
}

fn display_id(id: &str) -> String {
    id.rsplit(['.', '/']).next().unwrap_or(id).to_owned()
}

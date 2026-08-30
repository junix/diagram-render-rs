use diagram_ast_parser::ast::ScalarKind;
use diagram_ast_parser::ast::pikchr::{
    PikchrDirection, PikchrDocument, PikchrObject, PikchrStatement,
};

use super::RenderPlan;
use super::cards::{push_text, truncate};
use crate::Theme;
use crate::scene::{Point, Primitive, Rect, Scene, Stroke, TextAnchor, TextWeight};

pub(crate) fn render(document: &PikchrDocument, theme: &Theme) -> RenderPlan {
    let shape_count = document
        .statements
        .iter()
        .filter(|statement| match &statement.node {
            PikchrStatement::Object(object) => !matches!(
                object.object_type.to_ascii_lowercase().as_str(),
                "arrow" | "line" | "spline" | "arc" | "move"
            ),
            _ => false,
        })
        .count();
    let width = (shape_count as f32 * 190.0 + 160.0).clamp(700.0, 1600.0);
    let height = 300.0;
    let mut scene = Scene::new(width, height, "Pikchr geometric scene");
    push_text(
        &mut scene,
        Point::new(34.0, 34.0),
        "Pikchr geometric scene",
        23.0,
        TextAnchor::Start,
        &theme.foreground,
        TextWeight::Bold,
    );
    let mut cursor = Point::new(125.0, 165.0);
    let mut direction = PikchrDirection::Right;
    let mut previous_was_shape = false;
    let mut warnings = Vec::new();
    let mut warned_attributes = false;

    for statement in &document.statements {
        match &statement.node {
            PikchrStatement::Direction(next) => direction = *next,
            PikchrStatement::Object(object) => {
                let object_type = object.object_type.to_ascii_lowercase();
                if !object.attributes.is_empty() && !warned_attributes {
                    warnings.push(
                        "Pikchr attributes are retained by the AST; this renderer currently applies labels and common flow geometry, not expression-level sizing/placement"
                            .to_owned(),
                    );
                    warned_attributes = true;
                }
                if matches!(
                    object_type.as_str(),
                    "arrow" | "line" | "spline" | "arc" | "move"
                ) {
                    draw_flow_object(&mut scene, &object_type, &mut cursor, direction, theme);
                    previous_was_shape = false;
                } else {
                    if previous_was_shape {
                        cursor = advance(cursor, direction, 190.0);
                    }
                    draw_shape(&mut scene, object, cursor, theme);
                    previous_was_shape = true;
                }
            }
            PikchrStatement::Place(place) => warnings.push(format!(
                "Pikchr place `{}` is retained but its expression is not geometrically evaluated",
                place.label
            )),
            PikchrStatement::Assignment(assignment) => warnings.push(format!(
                "Pikchr assignment `{}` is retained but not expression-evaluated",
                assignment.variable
            )),
            PikchrStatement::Define(definition) => warnings.push(format!(
                "Pikchr macro `{}` is retained but not expanded",
                definition.name
            )),
            PikchrStatement::Print(_) | PikchrStatement::Assert(_) => {}
        }
    }
    RenderPlan { scene, warnings }
}

fn draw_shape(scene: &mut Scene, object: &PikchrObject, center: Point, theme: &Theme) {
    let object_type = object.object_type.to_ascii_lowercase();
    let label = object_label(object);
    let stroke = Some(Stroke::solid(theme.line.clone(), 1.6));
    match object_type.as_str() {
        "circle" => scene.push(Primitive::Ellipse {
            center,
            radius_x: 44.0,
            radius_y: 44.0,
            fill: Some(theme.surface.clone()),
            fill_opacity: 1.0,
            stroke,
        }),
        "ellipse" | "oval" => scene.push(Primitive::Ellipse {
            center,
            radius_x: 76.0,
            radius_y: 40.0,
            fill: Some(theme.surface.clone()),
            fill_opacity: 1.0,
            stroke,
        }),
        "diamond" => scene.push(Primitive::Polygon {
            points: vec![
                Point::new(center.x, center.y - 48.0),
                Point::new(center.x + 76.0, center.y),
                Point::new(center.x, center.y + 48.0),
                Point::new(center.x - 76.0, center.y),
            ],
            fill: Some(theme.surface.clone()),
            fill_opacity: 1.0,
            stroke,
        }),
        "cylinder" => {
            scene.push(Primitive::Rect {
                rect: Rect::new(center.x - 72.0, center.y - 34.0, 144.0, 68.0),
                radius: 5.0,
                fill: Some(theme.surface.clone()),
                fill_opacity: 1.0,
                stroke: stroke.clone(),
            });
            scene.push(Primitive::Ellipse {
                center: Point::new(center.x, center.y - 34.0),
                radius_x: 72.0,
                radius_y: 13.0,
                fill: Some(theme.accent_soft.clone()),
                fill_opacity: 1.0,
                stroke,
            });
        }
        "dot" => scene.push(Primitive::Ellipse {
            center,
            radius_x: 6.0,
            radius_y: 6.0,
            fill: Some(theme.accent.clone()),
            fill_opacity: 1.0,
            stroke: None,
        }),
        "text" => {}
        _ => scene.push(Primitive::Rect {
            rect: Rect::new(center.x - 75.0, center.y - 38.0, 150.0, 76.0),
            radius: 10.0,
            fill: Some(theme.surface.clone()),
            fill_opacity: 1.0,
            stroke,
        }),
    }
    if !label.is_empty() {
        for (index, line) in label.lines().take(3).enumerate() {
            push_text(
                scene,
                Point::new(center.x, center.y + (index as f32 - 0.5) * 19.0),
                &truncate(line, 20),
                if index == 0 { 14.0 } else { 11.0 },
                TextAnchor::Middle,
                &theme.foreground,
                if index == 0 {
                    TextWeight::Bold
                } else {
                    TextWeight::Normal
                },
            );
        }
    }
}

fn draw_flow_object(
    scene: &mut Scene,
    object_type: &str,
    cursor: &mut Point,
    direction: PikchrDirection,
    theme: &Theme,
) {
    let end = advance(*cursor, direction, 170.0);
    if object_type != "move" {
        let visible_start = advance(*cursor, direction, 75.0);
        let visible_end = advance(end, direction, -75.0);
        let stroke = if matches!(object_type, "spline" | "arc") {
            Stroke::dashed(theme.line.clone(), 1.8)
        } else {
            Stroke::solid(theme.line.clone(), 1.8)
        };
        scene.push(Primitive::Line {
            from: visible_start,
            to: visible_end,
            stroke,
        });
        if object_type == "arrow" {
            draw_arrowhead(scene, visible_end, visible_start, &theme.line);
        }
    }
    *cursor = end;
}

fn object_label(object: &PikchrObject) -> String {
    let mut values = Vec::new();
    if let Some(label) = &object.label {
        values.push(label.clone());
    }
    values.extend(
        object
            .attributes
            .iter()
            .filter(|attribute| attribute.kind == ScalarKind::String)
            .map(|attribute| attribute.value.clone()),
    );
    values.dedup();
    values.join("\n")
}

fn advance(point: Point, direction: PikchrDirection, amount: f32) -> Point {
    match direction {
        PikchrDirection::Right => Point::new(point.x + amount, point.y),
        PikchrDirection::Down => Point::new(point.x, point.y + amount),
        PikchrDirection::Left => Point::new(point.x - amount, point.y),
        PikchrDirection::Up => Point::new(point.x, point.y - amount),
    }
}

fn draw_arrowhead(scene: &mut Scene, tip: Point, previous: Point, color: &str) {
    let dx = tip.x - previous.x;
    let dy = tip.y - previous.y;
    let length = (dx * dx + dy * dy).sqrt().max(0.001);
    let ux = dx / length;
    let uy = dy / length;
    let base = Point::new(tip.x - ux * 12.0, tip.y - uy * 12.0);
    let perpendicular = Point::new(-uy * 5.5, ux * 5.5);
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

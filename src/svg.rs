use std::fmt::Write as _;

use crate::scene::{Primitive, Scene, Stroke, TextAnchor, TextWeight};

pub(crate) fn scene_to_svg(scene: &Scene, background: Option<&str>, font_family: &str) -> String {
    let mut svg = String::with_capacity(scene.primitives.len() * 140 + 512);
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{:.2}" height="{:.2}" viewBox="0 0 {:.2} {:.2}" role="img" aria-labelledby="diagram-title">"#,
        scene.width, scene.height, scene.width, scene.height
    );
    let _ = write!(
        svg,
        "<title id=\"diagram-title\">{}</title>",
        escape_text(&scene.title)
    );
    let _ = write!(
        svg,
        "<g shape-rendering=\"geometricPrecision\" text-rendering=\"optimizeLegibility\" font-family=\"{}\">",
        escape_attr(font_family)
    );
    if let Some(color) = background {
        let _ = write!(
            svg,
            r#"<rect data-canvas-background="true" x="0" y="0" width="{:.2}" height="{:.2}" fill="{}"/>"#,
            scene.width,
            scene.height,
            escape_attr(color)
        );
    }
    for primitive in &scene.primitives {
        write_primitive(&mut svg, primitive);
    }
    svg.push_str("</g></svg>");
    svg
}

fn write_primitive(svg: &mut String, primitive: &Primitive) {
    match primitive {
        Primitive::Rect {
            rect,
            radius,
            fill,
            fill_opacity,
            stroke,
        } => {
            let _ = write!(
                svg,
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}""#,
                rect.x, rect.y, rect.width, rect.height, radius
            );
            write_fill(svg, fill.as_deref(), *fill_opacity);
            write_stroke(svg, stroke.as_ref());
            svg.push_str("/>");
        }
        Primitive::Ellipse {
            center,
            radius_x,
            radius_y,
            fill,
            fill_opacity,
            stroke,
        } => {
            let _ = write!(
                svg,
                r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}""#,
                center.x, center.y, radius_x, radius_y
            );
            write_fill(svg, fill.as_deref(), *fill_opacity);
            write_stroke(svg, stroke.as_ref());
            svg.push_str("/>");
        }
        Primitive::Line { from, to, stroke } => {
            let _ = write!(
                svg,
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" fill="none""#,
                from.x, from.y, to.x, to.y
            );
            write_stroke(svg, Some(stroke));
            svg.push_str("/>");
        }
        Primitive::Polyline {
            points,
            stroke,
            fill,
        } => {
            svg.push_str("<polyline points=\"");
            write_points(svg, points);
            svg.push('"');
            write_fill(svg, fill.as_deref(), 1.0);
            write_stroke(svg, Some(stroke));
            svg.push_str("/>");
        }
        Primitive::Polygon {
            points,
            fill,
            fill_opacity,
            stroke,
        } => {
            svg.push_str("<polygon points=\"");
            write_points(svg, points);
            svg.push('"');
            write_fill(svg, fill.as_deref(), *fill_opacity);
            write_stroke(svg, stroke.as_ref());
            svg.push_str("/>");
        }
        Primitive::Text {
            at,
            text,
            size,
            anchor,
            color,
            weight,
            italic,
        } => {
            let anchor = match anchor {
                TextAnchor::Start => "start",
                TextAnchor::Middle => "middle",
                TextAnchor::End => "end",
            };
            let weight = match weight {
                TextWeight::Normal => "400",
                TextWeight::Bold => "650",
            };
            let style = if *italic { "italic" } else { "normal" };
            let _ = write!(
                svg,
                r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="{}" dominant-baseline="middle" fill="{}" font-weight="{}" font-style="{}">{}</text>"#,
                at.x,
                at.y,
                size,
                anchor,
                escape_attr(color),
                weight,
                style,
                escape_text(text)
            );
        }
    }
}

fn write_points(svg: &mut String, points: &[crate::scene::Point]) {
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            svg.push(' ');
        }
        let _ = write!(svg, "{:.2},{:.2}", point.x, point.y);
    }
}

fn write_fill(svg: &mut String, fill: Option<&str>, opacity: f32) {
    match fill {
        Some(fill) => {
            let _ = write!(
                svg,
                " fill=\"{}\" fill-opacity=\"{:.3}\"",
                escape_attr(fill),
                opacity.clamp(0.0, 1.0)
            );
        }
        None => svg.push_str(" fill=\"none\""),
    }
}

fn write_stroke(svg: &mut String, stroke: Option<&Stroke>) {
    let Some(stroke) = stroke else {
        svg.push_str(" stroke=\"none\"");
        return;
    };
    let _ = write!(
        svg,
        " stroke=\"{}\" stroke-width=\"{:.2}\" stroke-opacity=\"{:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
        escape_attr(&stroke.color),
        stroke.width,
        stroke.opacity.clamp(0.0, 1.0)
    );
    if !stroke.dash.is_empty() {
        svg.push_str(" stroke-dasharray=\"");
        for (index, value) in stroke.dash.iter().enumerate() {
            if index > 0 {
                svg.push(',');
            }
            let _ = write!(svg, "{value:.2}");
        }
        svg.push('"');
    }
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

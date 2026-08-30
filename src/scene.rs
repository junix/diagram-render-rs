/// A finite 2D point in CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle in CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Stroke styling shared by line-like primitives.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub color: String,
    pub width: f32,
    pub dash: Vec<f32>,
    pub opacity: f32,
}

impl Stroke {
    #[must_use]
    pub fn solid(color: impl Into<String>, width: f32) -> Self {
        Self {
            color: color.into(),
            width,
            dash: Vec::new(),
            opacity: 1.0,
        }
    }

    #[must_use]
    pub fn dashed(color: impl Into<String>, width: f32) -> Self {
        Self {
            color: color.into(),
            width,
            dash: vec![6.0, 5.0],
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextWeight {
    #[default]
    Normal,
    Bold,
}

/// Backend-neutral visual primitives. They intentionally contain no AST or
/// diagram semantics.
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Rect {
        rect: Rect,
        radius: f32,
        fill: Option<String>,
        fill_opacity: f32,
        stroke: Option<Stroke>,
    },
    Ellipse {
        center: Point,
        radius_x: f32,
        radius_y: f32,
        fill: Option<String>,
        fill_opacity: f32,
        stroke: Option<Stroke>,
    },
    Line {
        from: Point,
        to: Point,
        stroke: Stroke,
    },
    Polyline {
        points: Vec<Point>,
        stroke: Stroke,
        fill: Option<String>,
    },
    Polygon {
        points: Vec<Point>,
        fill: Option<String>,
        fill_opacity: f32,
        stroke: Option<Stroke>,
    },
    Text {
        at: Point,
        text: String,
        size: f32,
        anchor: TextAnchor,
        color: String,
        weight: TextWeight,
        italic: bool,
    },
}

/// One sized vector scene ready for SVG emission.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub title: String,
    pub primitives: Vec<Primitive>,
}

impl Scene {
    #[must_use]
    pub fn new(width: f32, height: f32, title: impl Into<String>) -> Self {
        Self {
            width,
            height,
            title: title.into(),
            primitives: Vec::new(),
        }
    }

    pub fn push(&mut self, primitive: Primitive) {
        self.primitives.push(primitive);
    }

    pub(crate) fn validate(&self) -> std::result::Result<(), String> {
        if !finite_positive(self.width) || !finite_positive(self.height) {
            return Err(format!(
                "canvas dimensions must be finite and positive ({}x{})",
                self.width, self.height
            ));
        }
        for (index, primitive) in self.primitives.iter().enumerate() {
            if !primitive_is_finite(primitive) {
                return Err(format!("primitive {index} contains non-finite geometry"));
            }
        }
        Ok(())
    }
}

fn finite_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn point_is_finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn stroke_is_finite(stroke: &Stroke) -> bool {
    stroke.width.is_finite()
        && stroke.opacity.is_finite()
        && stroke.dash.iter().all(|value| value.is_finite())
}

fn primitive_is_finite(primitive: &Primitive) -> bool {
    match primitive {
        Primitive::Rect {
            rect,
            radius,
            fill_opacity,
            stroke,
            ..
        } => {
            [
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                *radius,
                *fill_opacity,
            ]
            .into_iter()
            .all(f32::is_finite)
                && stroke.as_ref().is_none_or(stroke_is_finite)
        }
        Primitive::Ellipse {
            center,
            radius_x,
            radius_y,
            fill_opacity,
            stroke,
            ..
        } => {
            point_is_finite(*center)
                && [*radius_x, *radius_y, *fill_opacity]
                    .into_iter()
                    .all(f32::is_finite)
                && stroke.as_ref().is_none_or(stroke_is_finite)
        }
        Primitive::Line { from, to, stroke } => {
            point_is_finite(*from) && point_is_finite(*to) && stroke_is_finite(stroke)
        }
        Primitive::Polyline { points, stroke, .. } => {
            points.iter().copied().all(point_is_finite) && stroke_is_finite(stroke)
        }
        Primitive::Polygon {
            points,
            fill_opacity,
            stroke,
            ..
        } => {
            points.iter().copied().all(point_is_finite)
                && fill_opacity.is_finite()
                && stroke.as_ref().is_none_or(stroke_is_finite)
        }
        Primitive::Text { at, size, .. } => point_is_finite(*at) && size.is_finite(),
    }
}

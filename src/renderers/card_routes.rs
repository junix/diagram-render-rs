//! Bounded repair of unsafe card routes. Valid legacy geometry is retained.
//!
//! The visibility-grid A* follows the small native routing approach used by
//! graph-ir-rs and tala-rs, without pulling a second layout engine into this crate.
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use super::{Connector, Point, Rect, polyline_middle};
use crate::{RenderError, Result};

const MAX_CARDS: usize = 256;
const MAX_CONNECTORS: usize = 1024;
const MAX_GRID: usize = 16_384;
const MAX_POINTS: usize = 256;
const EDGE_WORK: usize = 2_000_000;
const TOTAL_WORK: usize = 12_000_000;
const CLEARANCE: f32 = 22.0;
const EPS: f32 = 0.01;

pub(super) struct Router<'a> {
    cards: &'a BTreeMap<String, Rect>,
    frame: Rect,
    routes: Vec<Vec<Point>>,
    labels: Vec<Rect>,
    pairs: BTreeMap<(&'a str, &'a str), usize>,
    work: usize,
}

impl<'a> Router<'a> {
    pub(super) fn new(
        cards: &'a BTreeMap<String, Rect>,
        width: f32,
        height: f32,
        count: usize,
    ) -> Result<Self> {
        if count > MAX_CONNECTORS || (count != 0 && cards.len() > MAX_CARDS) {
            return Err(RenderError::InvalidScene(format!(
                "card routing budget exceeded: {} cards / {count} connectors (limits {MAX_CARDS}/{MAX_CONNECTORS})",
                cards.len()
            )));
        }
        Ok(Self {
            cards,
            // Reserve the title/subtitle and leave room for the stroke.
            frame: Rect::new(8.0, 64.0, width - 16.0, height - 72.0),
            routes: Vec::new(),
            labels: Vec::new(),
            pairs: BTreeMap::new(),
            work: TOTAL_WORK,
        })
    }

    pub(super) fn route(
        &mut self,
        index: usize,
        connector: &Connector,
        from: Rect,
        to: Rect,
        label_width: f32,
        legacy: Vec<Point>,
    ) -> Result<(Vec<Point>, Option<Point>)> {
        let context = || {
            format!(
                "card connector {} ({} -> {})",
                index + 1,
                connector.from,
                connector.to
            )
        };
        let from_id = self
            .cards
            .get_key_value(&connector.from)
            .expect("resolved source")
            .0
            .as_str();
        let to_id = self
            .cards
            .get_key_value(&connector.to)
            .expect("resolved target")
            .0
            .as_str();
        let pair = if from_id <= to_id {
            (from_id, to_id)
        } else {
            (to_id, from_id)
        };
        let ordinal = *self.pairs.get(&pair).unwrap_or(&0);
        self.pairs.insert(pair, ordinal + 1);
        let mut budget = Budget {
            edge: EDGE_WORK,
            total: &mut self.work,
        };
        let occupied = self.cards.values().copied().collect::<Vec<_>>();
        let legacy_label = (label_width > 0.0).then(|| polyline_middle(&legacy));
        let clear = route_clear(
            &legacy,
            from,
            to,
            &occupied,
            &self.labels,
            self.frame,
            &mut budget,
        )
        .map(|clear| clear && !self.routes.iter().any(|route| same_route(&legacy, route)))
        .and_then(|clear| {
            Ok(clear
                && match legacy_label {
                    Some(at) => label_clear(
                        at,
                        label_width,
                        &legacy,
                        &occupied,
                        &self.labels,
                        &self.routes,
                        self.frame,
                        &mut budget,
                    )?,
                    None => true,
                })
        });
        if clear.map_err(|why| failure(&context(), why))? {
            return Ok(self.remember(legacy, legacy_label, label_width));
        }

        // Separate repeated/reversed endpoint pairs at their boundary ports.
        // Refuse excessive multiplicity rather than clamping distinct ports together.
        if ordinal >= 8 {
            return Err(failure(
                &context(),
                "parallel port budget exceeded (8 routes per endpoint pair)",
            ));
        }
        let offset = if ordinal == 0 {
            0.0
        } else {
            let step = ordinal.div_ceil(2) as f32 * 10.0;
            if ordinal % 2 == 1 { step } else { -step }
        };
        let mut reason = "no clear orthogonal route and owned label slot";
        // Increasing clearance gives a bounded alternative when the shortest
        // route has no readable label position (for example stacked labels).
        for (first, second) in port_pairs(from, to, label_width, ordinal) {
            for (extra, label_padding) in [(0.0, 5.0), (0.0, 19.0), (14.0, 19.0)] {
                let clearance = CLEARANCE
                    + extra
                    + if from == to && label_padding == 5.0 {
                        6.0
                    } else {
                        0.0
                    };
                let mut obstacles = occupied
                    .iter()
                    .map(|r| expand(*r, clearance))
                    .collect::<Vec<_>>();
                obstacles.extend(self.labels.iter().map(|r| expand(*r, label_padding)));
                let start = port(from, first, offset, clearance);
                let end = port(to, second, offset, clearance);
                let result = visibility_route(
                    start.escape,
                    end.escape,
                    &obstacles,
                    &self.routes,
                    self.frame,
                    &mut budget,
                );
                let interior = match result {
                    Ok(Some(points)) => points,
                    Ok(None) => continue,
                    Err(why) => return Err(failure(&context(), why)),
                };
                let mut points = vec![start.boundary];
                points.extend(interior);
                points.push(end.boundary);
                let points = simplify(points);
                if !route_clear(
                    &points,
                    from,
                    to,
                    &occupied,
                    &self.labels,
                    self.frame,
                    &mut budget,
                )
                .map_err(|why| failure(&context(), why))?
                {
                    continue;
                }
                if self.routes.iter().any(|route| same_route(&points, route)) {
                    continue;
                }
                let label = if label_width > 0.0 {
                    match place_label(
                        &points,
                        label_width,
                        &occupied,
                        &self.labels,
                        &self.routes,
                        self.frame,
                        &mut budget,
                    )
                    .map_err(|why| failure(&context(), why))?
                    {
                        Some(point) => Some(point),
                        None => {
                            reason = "no clear label slot on its connector route";
                            continue;
                        }
                    }
                } else {
                    None
                };
                return Ok(self.remember(points, label, label_width));
            }
        }
        Err(failure(&context(), reason))
    }

    fn remember(
        &mut self,
        points: Vec<Point>,
        label: Option<Point>,
        width: f32,
    ) -> (Vec<Point>, Option<Point>) {
        self.routes.push(points.clone());
        if let Some(at) = label {
            self.labels.push(label_rect(at, width));
        }
        (points, label)
    }
}

fn failure(context: &str, why: &str) -> RenderError {
    RenderError::InvalidScene(format!("{context}: {why}"))
}

struct Budget<'a> {
    edge: usize,
    total: &'a mut usize,
}
impl Budget<'_> {
    fn spend(&mut self, amount: usize) -> std::result::Result<(), &'static str> {
        if amount > self.edge || amount > *self.total {
            return Err("routing work budget exhausted");
        }
        self.edge -= amount;
        *self.total -= amount;
        Ok(())
    }
}
type SearchResult<T> = std::result::Result<T, &'static str>;

fn expand(r: Rect, pad: f32) -> Rect {
    Rect::new(
        r.x - pad,
        r.y - pad,
        r.width + pad * 2.0,
        r.height + pad * 2.0,
    )
}
fn inside(p: Point, r: Rect) -> bool {
    p.x > r.x + EPS && p.x < r.x + r.width - EPS && p.y > r.y + EPS && p.y < r.y + r.height - EPS
}
fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height
}
fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
fn label_rect(at: Point, width: f32) -> Rect {
    Rect::new(at.x - width / 2.0, at.y - 12.0, width, 24.0)
}

// Exact segment/open-rectangle intersection, including legacy near-axis routes.
fn crosses(a: Point, b: Point, r: Rect) -> bool {
    let mut low = 0.0_f32;
    let mut high = 1.0_f32;
    for (origin, delta, min, max) in [
        (a.x, b.x - a.x, r.x + EPS, r.x + r.width - EPS),
        (a.y, b.y - a.y, r.y + EPS, r.y + r.height - EPS),
    ] {
        if delta.abs() < f32::EPSILON {
            if origin <= min || origin >= max {
                return false;
            }
        } else {
            let first = (min - origin) / delta;
            let second = (max - origin) / delta;
            low = low.max(first.min(second));
            high = high.min(first.max(second));
            if low >= high {
                return false;
            }
        }
    }
    low < high
}

// A port must leave/re-enter normally; tangent routes can hide an arrowhead in
// its own endpoint card even when the centerline follows that card's boundary.
fn outward_port(at: Point, next: Point, card: Rect) -> bool {
    let dx = next.x - at.x;
    let dy = next.y - at.y;
    ((at.x - card.x).abs() < EPS && dy.abs() < EPS && dx < 0.0)
        || ((at.x - card.x - card.width).abs() < EPS && dy.abs() < EPS && dx > 0.0)
        || ((at.y - card.y).abs() < EPS && dx.abs() < EPS && dy < 0.0)
        || ((at.y - card.y - card.height).abs() < EPS && dx.abs() < EPS && dy > 0.0)
}

fn route_clear(
    points: &[Point],
    from: Rect,
    to: Rect,
    cards: &[Rect],
    labels: &[Rect],
    frame: Rect,
    budget: &mut Budget<'_>,
) -> SearchResult<bool> {
    if points.len() < 2 || points.len() > MAX_POINTS || points.iter().any(|p| !contains(frame, *p))
    {
        return Ok(false);
    }
    if !outward_port(points[0], points[1], from)
        || !outward_port(points[points.len() - 1], points[points.len() - 2], to)
    {
        return Ok(false);
    }
    for pair in points.windows(2) {
        if pair[0] == pair[1] {
            return Ok(false);
        }
        for card in cards {
            budget.spend(1)?;
            let obstacle = if *card == from || *card == to {
                *card
            } else {
                expand(*card, 8.0)
            };
            if crosses(pair[0], pair[1], obstacle) {
                return Ok(false);
            }
        }
        for label in labels {
            budget.spend(1)?;
            if crosses(pair[0], pair[1], expand(*label, 5.0)) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)] // Explicit geometry inputs; no hidden mutable scene state.
fn label_clear(
    at: Point,
    width: f32,
    route: &[Point],
    cards: &[Rect],
    labels: &[Rect],
    routes: &[Vec<Point>],
    frame: Rect,
    budget: &mut Budget<'_>,
) -> SearchResult<bool> {
    let rect = expand(label_rect(at, width), 1.0);
    if !contains(frame, Point::new(rect.x, rect.y))
        || !contains(frame, Point::new(rect.x + rect.width, rect.y + rect.height))
    {
        return Ok(false);
    }
    for card in cards {
        budget.spend(1)?;
        if overlaps(rect, expand(*card, 8.0)) {
            return Ok(false);
        }
    }
    for label in labels {
        budget.spend(1)?;
        if overlaps(rect, expand(*label, 5.0)) {
            return Ok(false);
        }
    }
    // Keep both arrowhead footprints and other connectors legible.
    for endpoint in [route[0], route[route.len() - 1]] {
        if overlaps(
            rect,
            Rect::new(endpoint.x - 12.0, endpoint.y - 12.0, 24.0, 24.0),
        ) {
            return Ok(false);
        }
    }
    for other in routes {
        for endpoint in [other[0], other[other.len() - 1]] {
            budget.spend(1)?;
            if overlaps(
                rect,
                Rect::new(endpoint.x - 12.0, endpoint.y - 12.0, 24.0, 24.0),
            ) {
                return Ok(false);
            }
        }
        for pair in other.windows(2) {
            budget.spend(1)?;
            if crosses(pair[0], pair[1], expand(rect, 3.0)) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn place_label(
    route: &[Point],
    width: f32,
    cards: &[Rect],
    labels: &[Rect],
    routes: &[Vec<Point>],
    frame: Rect,
    budget: &mut Budget<'_>,
) -> SearchResult<Option<Point>> {
    let mut candidates = vec![polyline_middle(route)];
    let mut segments = route.windows(2).collect::<Vec<_>>();
    segments.sort_by(|a, b| distance(b[0], b[1]).total_cmp(&distance(a[0], a[1])));
    for pair in segments {
        // A wide label may only fit near one end of a clear segment. Include
        // the canvas-clamped center rather than relying only on fixed samples.
        if pair[0].y == pair[1].y {
            let low = pair[0].x.min(pair[1].x).max(frame.x + width / 2.0 + 1.0);
            let high = pair[0]
                .x
                .max(pair[1].x)
                .min(frame.x + frame.width - width / 2.0 - 1.0);
            if low <= high {
                candidates.push(Point::new(
                    ((pair[0].x + pair[1].x) / 2.0).clamp(low, high),
                    pair[0].y,
                ));
            }
        }
        for t in [0.5, 0.25, 0.75] {
            candidates.push(Point::new(
                pair[0].x + (pair[1].x - pair[0].x) * t,
                pair[0].y + (pair[1].y - pair[0].y) * t,
            ));
        }
    }
    for at in candidates {
        if label_clear(at, width, route, cards, labels, routes, frame, budget)? {
            return Ok(Some(at));
        }
    }
    // Fixed midpoint samples can miss a narrow valid interval between a pill
    // and an arrowhead. Obstacle boundaries produce the remaining candidates
    // on the owner's existing segments, with no off-route label displacement.
    let mut blockers = cards.iter().map(|r| expand(*r, 8.0)).collect::<Vec<_>>();
    blockers.extend(labels.iter().map(|r| expand(*r, 5.0)));
    for path in std::iter::once(route).chain(routes.iter().map(Vec::as_slice)) {
        for endpoint in [path[0], path[path.len() - 1]] {
            blockers.push(Rect::new(endpoint.x - 12.0, endpoint.y - 12.0, 24.0, 24.0));
        }
    }
    for other in routes {
        for pair in other.windows(2) {
            budget.spend(1)?;
            blockers.push(expand(
                Rect::new(
                    pair[0].x.min(pair[1].x),
                    pair[0].y.min(pair[1].y),
                    (pair[0].x - pair[1].x).abs(),
                    (pair[0].y - pair[1].y).abs(),
                ),
                3.0,
            ));
        }
    }
    for pair in route.windows(2) {
        for blocker in &blockers {
            budget.spend(1)?;
            let candidates = if pair[0].y == pair[1].y {
                [
                    Point::new(blocker.x - width / 2.0 - 1.05, pair[0].y),
                    Point::new(blocker.x + blocker.width + width / 2.0 + 1.05, pair[0].y),
                ]
            } else {
                [
                    Point::new(pair[0].x, blocker.y - 13.05),
                    Point::new(pair[0].x, blocker.y + blocker.height + 13.05),
                ]
            };
            for at in candidates {
                if at.x >= pair[0].x.min(pair[1].x)
                    && at.x <= pair[0].x.max(pair[1].x)
                    && at.y >= pair[0].y.min(pair[1].y)
                    && at.y <= pair[0].y.max(pair[1].y)
                    && label_clear(at, width, route, cards, labels, routes, frame, budget)?
                {
                    return Ok(Some(at));
                }
            }
        }
    }
    Ok(None)
}

#[derive(Clone, Copy)]
struct Port {
    boundary: Point,
    escape: Point,
}
fn port(r: Rect, side: usize, offset: f32, pad: f32) -> Port {
    let center = r.center();
    let (boundary, normal) = match side {
        0 => (
            Point::new(r.x + r.width, center.y + offset),
            Point::new(1.0, 0.0),
        ),
        1 => (Point::new(r.x, center.y + offset), Point::new(-1.0, 0.0)),
        2 => (
            Point::new(center.x + offset, r.y + r.height),
            Point::new(0.0, 1.0),
        ),
        _ => (Point::new(center.x + offset, r.y), Point::new(0.0, -1.0)),
    };
    Port {
        boundary,
        escape: Point::new(boundary.x + normal.x * pad, boundary.y + normal.y * pad),
    }
}
fn port_pairs(from: Rect, to: Rect, label_width: f32, ordinal: usize) -> Vec<(usize, usize)> {
    if from == to {
        let pairs = [
            (0, 2),
            (2, 1),
            (1, 0),
            (2, 0),
            (1, 2),
            (0, 1),
            (0, 2),
            (2, 1),
        ];
        let mut choices = vec![pairs[ordinal]];
        for pair in pairs {
            if !choices.contains(&pair) {
                choices.push(pair);
            }
        }
        for first in 0..4 {
            for second in 0..4 {
                if first != second && !choices.contains(&(first, second)) {
                    choices.push((first, second));
                }
            }
        }
        return choices;
    }
    let vertical_overlap = from.y < to.y + to.height && to.y < from.y + from.height;
    let gap = (to.x - from.x - from.width).max(from.x - to.x - to.width);
    let dx = to.center().x - from.center().x;
    let dy = to.center().y - from.center().y;
    let horizontal = if dx >= 0.0 { (0, 1) } else { (1, 0) };
    let vertical = if dy >= 0.0 { (2, 3) } else { (3, 2) };
    let preferred = if vertical_overlap
        && (label_width > gap - 16.0 || (ordinal > 0 && gap < label_width + 64.0))
    {
        (2, 2)
    } else if dx.abs() > dy.abs() {
        horizontal
    } else {
        vertical
    };
    // The nearest ports may trap a wide label in a narrow column gap. Try
    // other cardinal pairs before giving up, still within one shared work cap.
    let mut pairs = vec![preferred];
    for pair in [vertical, horizontal, (2, 2)] {
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    // Mixed sides are needed when an earlier pill blocks only one endpoint's
    // usual escape. There are at most 16 pairs, all sharing the edge work cap.
    for first in 0..4 {
        for second in 0..4 {
            if !pairs.contains(&(first, second)) {
                pairs.push((first, second));
            }
        }
    }
    pairs
}

fn same_route(a: &[Point], b: &[Point]) -> bool {
    a == b || a.iter().eq(b.iter().rev())
}
fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut result: Vec<Point> = Vec::new();
    for p in points {
        if result.last() == Some(&p) {
            continue;
        }
        while result.len() >= 2 {
            let a = result[result.len() - 2];
            let b = result[result.len() - 1];
            if (a.x == b.x && b.x == p.x && (b.y - a.y) * (p.y - b.y) >= 0.0)
                || (a.y == b.y && b.y == p.y && (b.x - a.x) * (p.x - b.x) >= 0.0)
            {
                result.pop();
            } else {
                break;
            }
        }
        result.push(p);
    }
    result
}

#[derive(Clone, Copy)]
struct Entry {
    priority: f32,
    cost: f32,
    state: usize,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then_with(|| other.cost.total_cmp(&self.cost))
            .then_with(|| other.state.cmp(&self.state))
    }
}

fn visibility_route(
    start: Point,
    end: Point,
    obstacles: &[Rect],
    routes: &[Vec<Point>],
    frame: Rect,
    budget: &mut Budget<'_>,
) -> SearchResult<Option<Vec<Point>>> {
    if start == end || !contains(frame, start) || !contains(frame, end) {
        return Ok(None);
    }
    let mut xs = vec![start.x, end.x, frame.x, frame.x + frame.width];
    let mut ys = vec![start.y, end.y, frame.y, frame.y + frame.height];
    for r in obstacles {
        xs.extend([r.x, r.x + r.width]);
        ys.extend([r.y, r.y + r.height]);
    }
    // Alternate lanes allow parallel edges to avoid previous path/label footprints.
    for route in routes {
        for p in route {
            budget.spend(1)?;
            xs.extend([p.x - 28.0, p.x + 28.0]);
            ys.extend([p.y - 28.0, p.y + 28.0]);
        }
    }
    xs.retain(|x| *x >= frame.x && *x <= frame.x + frame.width);
    ys.retain(|y| *y >= frame.y && *y <= frame.y + frame.height);
    xs.sort_by(f32::total_cmp);
    ys.sort_by(f32::total_cmp);
    xs.dedup();
    ys.dedup();
    let count = xs
        .len()
        .checked_mul(ys.len())
        .filter(|n| *n <= MAX_GRID)
        .ok_or("routing visibility-grid budget exceeded")?;
    budget.spend(count)?;
    let width = xs.len();
    let point = |i: usize| Point::new(xs[i % width], ys[i / width]);
    let index = |p: Point| -> usize {
        ys.binary_search_by(|y| y.total_cmp(&p.y)).unwrap() * width
            + xs.binary_search_by(|x| x.total_cmp(&p.x)).unwrap()
    };
    let initial = index(start) * 3;
    let destination = index(end);
    let mut blocked = vec![false; count];
    for (i, value) in blocked.iter_mut().enumerate() {
        for r in obstacles {
            budget.spend(1)?;
            if inside(point(i), *r) {
                *value = true;
                break;
            }
        }
    }
    if blocked[initial / 3] || blocked[destination] {
        return Ok(None);
    }
    let mut costs = vec![f32::INFINITY; count * 3];
    let mut previous = vec![usize::MAX; count * 3];
    let mut queue = BinaryHeap::new();
    costs[initial] = 0.0;
    queue.push(Entry {
        priority: distance(start, end),
        cost: 0.0,
        state: initial,
    });
    while let Some(entry) = queue.pop() {
        budget.spend(1)?;
        if entry.cost > costs[entry.state] {
            continue;
        }
        let current = entry.state / 3;
        if current == destination {
            let mut state = entry.state;
            let mut path = Vec::new();
            loop {
                path.push(point(state / 3));
                if state == initial {
                    break;
                }
                state = previous[state];
                if path.len() > MAX_GRID {
                    return Err("routing path budget exceeded");
                }
            }
            path.reverse();
            let path = simplify(path);
            if path.len() > MAX_POINTS {
                return Err("routing vertex budget exceeded");
            }
            return Ok(Some(path));
        }
        let x = current % width;
        let y = current / width;
        let neighbors = [
            x.checked_sub(1).map(|nx| (y * width + nx, 1)),
            (x + 1 < width).then_some((current + 1, 1)),
            y.checked_sub(1).map(|ny| (ny * width + x, 2)),
            (y + 1 < ys.len()).then_some((current + width, 2)),
        ];
        for (next, direction) in neighbors.into_iter().flatten() {
            if blocked[next] {
                continue;
            }
            let a = point(current);
            let b = point(next);
            let mut clear = true;
            for r in obstacles {
                budget.spend(1)?;
                if crosses(a, b, *r) {
                    clear = false;
                    break;
                }
            }
            if !clear {
                continue;
            }
            let mut penalty = 0.0;
            for route in routes {
                for pair in route.windows(2) {
                    budget.spend(1)?;
                    if a.y == b.y && pair[0].y == pair[1].y && a.y == pair[0].y {
                        penalty += ((a.x.max(b.x).min(pair[0].x.max(pair[1].x))
                            - a.x.min(b.x).max(pair[0].x.min(pair[1].x)))
                        .max(0.0))
                            * 5.0;
                    } else if a.x == b.x && pair[0].x == pair[1].x && a.x == pair[0].x {
                        penalty += ((a.y.max(b.y).min(pair[0].y.max(pair[1].y))
                            - a.y.min(b.y).max(pair[0].y.min(pair[1].y)))
                        .max(0.0))
                            * 5.0;
                    }
                }
            }
            let bend = if entry.state % 3 == 0 || entry.state % 3 == direction {
                0.0
            } else {
                18.0
            };
            let cost = entry.cost + distance(a, b) + bend + penalty;
            let state = next * 3 + direction;
            if cost < costs[state] {
                costs[state] = cost;
                previous[state] = entry.state;
                queue.push(Entry {
                    priority: cost + distance(b, end),
                    cost,
                    state,
                });
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "card_routes_tests.rs"]
mod tests;

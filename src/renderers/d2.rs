use diagram_ast_parser::Located;
use diagram_ast_parser::ast::d2::{D2Document, D2EdgeOperator, D2Statement, D2Value};

use super::RenderPlan;
use super::cards::{Card, CardDiagram, Connector, ConnectorKind};
use crate::Theme;

pub(crate) fn render(document: &D2Document, theme: &Theme) -> RenderPlan {
    let mut diagram = CardDiagram::new("D2 diagram");
    let mut warnings = Vec::new();
    walk(&document.statements, "", None, &mut diagram, &mut warnings);

    let endpoints: Vec<String> = diagram
        .connectors
        .iter()
        .flat_map(|connector| [connector.from.clone(), connector.to.clone()])
        .collect();
    for endpoint in endpoints {
        diagram.ensure_endpoint(&endpoint);
    }
    RenderPlan {
        scene: super::cards::render(&mut diagram, theme),
        warnings,
    }
}

fn walk(
    statements: &[Located<D2Statement>],
    prefix: &str,
    owner: Option<&str>,
    diagram: &mut CardDiagram,
    warnings: &mut Vec<String>,
) {
    for statement in statements {
        match &statement.node {
            D2Statement::Entry(entry) => match &entry.value {
                Some(D2Value::Map { label, statements }) => {
                    let id = qualify(prefix, &entry.key);
                    let mut lines = Vec::new();
                    if let Some(owner) = owner {
                        lines.insert(0, format!("inside {owner}"));
                    }
                    let title = label
                        .as_deref()
                        .or_else(|| scalar_property(statements, "label"))
                        .unwrap_or(&entry.key);
                    let kind = scalar_property(statements, "shape")
                        .map_or_else(|| "D2 CONTAINER".to_owned(), |shape| format!("D2 {shape}"));
                    diagram.push_card(Card::new(&id, title, kind, lines, diagram.cards.len()));
                    walk(statements, &id, Some(&id), diagram, warnings);
                }
                Some(D2Value::Scalar(value)) if is_property(&entry.key) && owner.is_some() => {
                    if entry.key != "label"
                        && let Some(card) = diagram
                            .cards
                            .iter_mut()
                            .find(|card| Some(card.id.as_str()) == owner)
                    {
                        card.lines.push(format!("{}: {value}", entry.key));
                    }
                }
                Some(D2Value::Scalar(value)) => {
                    if entry.key.eq_ignore_ascii_case("direction") {
                        continue;
                    }
                    let id = qualify(prefix, &entry.key);
                    let lines = if value == &entry.key {
                        Vec::new()
                    } else {
                        vec![format!("label: {value}")]
                    };
                    diagram.push_card(Card::new(
                        id,
                        if value.is_empty() { &entry.key } else { value },
                        "D2 NODE",
                        lines,
                        diagram.cards.len(),
                    ));
                }
                None => {
                    let id = qualify(prefix, &entry.key);
                    diagram.push_card(Card::new(
                        id,
                        &entry.key,
                        "D2 NODE",
                        Vec::new(),
                        diagram.cards.len(),
                    ));
                }
            },
            D2Statement::EdgeChain(edge) => {
                for (index, operator) in edge.operators.iter().copied().enumerate() {
                    let Some(from) = edge.endpoints.get(index) else {
                        continue;
                    };
                    let Some(to) = edge.endpoints.get(index + 1) else {
                        continue;
                    };
                    diagram.connectors.push(Connector {
                        from: resolve_edge_endpoint(prefix, from),
                        to: resolve_edge_endpoint(prefix, to),
                        label: edge.label.clone(),
                        kind: match operator {
                            D2EdgeOperator::Directed => ConnectorKind::Directed,
                            D2EdgeOperator::ReverseDirected => ConnectorKind::Reverse,
                            D2EdgeOperator::Undirected => ConnectorKind::Undirected,
                            D2EdgeOperator::Bidirectional => ConnectorKind::Bidirectional,
                        },
                        dashed: false,
                    });
                }
            }
            D2Statement::Import(import) => warnings.push(format!(
                "D2 import `{}` is shown neither loaded nor expanded",
                import.path
            )),
        }
    }
}

fn scalar_property<'a>(statements: &'a [Located<D2Statement>], name: &str) -> Option<&'a str> {
    statements
        .iter()
        .find_map(|statement| match &statement.node {
            D2Statement::Entry(entry) if entry.key == name => match &entry.value {
                Some(D2Value::Scalar(value)) => Some(value.as_str()),
                _ => None,
            },
            _ => None,
        })
}

fn is_property(key: &str) -> bool {
    matches!(
        key,
        "label" | "shape" | "icon" | "tooltip" | "link" | "class" | "width" | "height" | "near"
    ) || key.starts_with("style.")
}

fn qualify(prefix: &str, key: &str) -> String {
    if prefix.is_empty() || key.contains('.') {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

fn resolve_edge_endpoint(prefix: &str, endpoint: &str) -> String {
    if endpoint.contains('.') || prefix.is_empty() {
        endpoint.to_owned()
    } else {
        qualify(prefix, endpoint)
    }
}

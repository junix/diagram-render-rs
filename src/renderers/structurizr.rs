use diagram_ast_parser::Located;
use diagram_ast_parser::ast::structurizr::{StructurizrDocument, StructurizrStatement};

use super::RenderPlan;
use super::cards::{Card, CardDiagram, Connector, ConnectorKind};
use crate::Theme;

pub(crate) fn render(document: &StructurizrDocument, theme: &Theme) -> RenderPlan {
    let mut diagram = CardDiagram::new("Structurizr workspace");
    let mut warnings = Vec::new();
    walk(&document.statements, None, &mut diagram, &mut warnings);
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
    statements: &[Located<StructurizrStatement>],
    parent: Option<&str>,
    diagram: &mut CardDiagram,
    warnings: &mut Vec<String>,
) {
    for statement in statements {
        match &statement.node {
            StructurizrStatement::Workspace(workspace) => {
                if let Some(name) = &workspace.name {
                    diagram.title = format!("Structurizr · {name}");
                }
                walk(&workspace.body, None, diagram, warnings);
            }
            StructurizrStatement::Element(element) => {
                let id = element
                    .id
                    .clone()
                    .or_else(|| element.name.clone())
                    .unwrap_or_else(|| format!("element-{}", diagram.cards.len() + 1));
                let mut lines = Vec::new();
                if let Some(description) = &element.description {
                    lines.push(description.clone());
                }
                if let Some(technology) = &element.technology {
                    lines.push(format!("technology: {technology}"));
                }
                if let Some(parent) = parent {
                    lines.push(format!("inside {parent}"));
                }
                diagram.push_card(Card::new(
                    &id,
                    element.name.as_deref().unwrap_or(&id),
                    &element.element_type,
                    lines,
                    diagram.cards.len(),
                ));
                walk(&element.body, Some(&id), diagram, warnings);
            }
            StructurizrStatement::Relationship(relationship) => {
                let label = match (&relationship.description, &relationship.technology) {
                    (Some(description), Some(technology)) => {
                        Some(format!("{description} · {technology}"))
                    }
                    (Some(description), None) => Some(description.clone()),
                    (None, Some(technology)) => Some(technology.clone()),
                    (None, None) => None,
                };
                diagram.connectors.push(Connector {
                    from: relationship.source.clone(),
                    to: relationship.target.clone(),
                    label,
                    kind: ConnectorKind::Directed,
                    dashed: false,
                });
                walk(&relationship.body, parent, diagram, warnings);
            }
            StructurizrStatement::Block(block) => {
                if block.keyword.to_ascii_lowercase().contains("view") {
                    warnings.push(format!(
                        "Structurizr `{}` view syntax is retained, but this renderer shows the model rather than evaluating a view",
                        block.keyword
                    ));
                }
                walk(&block.body, parent, diagram, warnings);
            }
            StructurizrStatement::Property(property) => {
                if let Some(parent) = parent {
                    if let Some(card) = diagram.cards.iter_mut().find(|card| card.id == parent) {
                        let values = property
                            .values
                            .iter()
                            .map(|value| value.value.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        card.lines.push(format!("{}: {values}", property.name));
                    }
                }
                walk(&property.body, parent, diagram, warnings);
            }
            StructurizrStatement::Directive(_) => {}
        }
    }
}

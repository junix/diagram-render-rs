use diagram_ast_parser::Located;
use diagram_ast_parser::ast::likec4::{LikeC4Document, LikeC4SectionKind, LikeC4Statement};

use super::RenderPlan;
use super::cards::{Card, CardDiagram, Connector, ConnectorKind};
use crate::Theme;

pub(crate) fn render(document: &LikeC4Document, theme: &Theme) -> RenderPlan {
    let mut diagram = CardDiagram::new("LikeC4 model");
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
    statements: &[Located<LikeC4Statement>],
    current_element: Option<&str>,
    diagram: &mut CardDiagram,
    warnings: &mut Vec<String>,
) {
    for statement in statements {
        match &statement.node {
            LikeC4Statement::Section(section) => {
                if section.section == LikeC4SectionKind::Deployment {
                    diagram.title = "LikeC4 deployment model".to_owned();
                }
                walk(&section.body, current_element, diagram, warnings);
            }
            LikeC4Statement::Element(element) => {
                let mut lines = Vec::new();
                if let Some(description) = &element.description {
                    lines.push(description.clone());
                }
                if let Some(reference) = &element.reference {
                    lines.push(format!("instance of {reference}"));
                }
                if let Some(parent) = current_element {
                    lines.push(format!("inside {parent}"));
                }
                diagram.push_card(Card::new(
                    &element.name,
                    element.title.as_deref().unwrap_or(&element.name),
                    &element.element_type,
                    lines,
                    diagram.cards.len(),
                ));
                walk(&element.body, Some(&element.name), diagram, warnings);
            }
            LikeC4Statement::Relationship(relationship) => {
                if let Some(source) = relationship.source.as_deref().or(current_element) {
                    diagram.connectors.push(Connector {
                        from: source.to_owned(),
                        to: relationship.target.clone(),
                        label: relationship
                            .title
                            .clone()
                            .or_else(|| relationship.description.clone())
                            .or_else(|| relationship.relationship_type.clone()),
                        kind: ConnectorKind::Directed,
                        dashed: relationship.relationship_type.is_some(),
                    });
                } else {
                    warnings.push(format!(
                        "LikeC4 relationship targeting `{}` has no resolvable source",
                        relationship.target
                    ));
                }
                walk(&relationship.body, current_element, diagram, warnings);
            }
            LikeC4Statement::View(view) => {
                warnings.push(format!(
                    "LikeC4 {} view is retained, but this renderer shows the model rather than evaluating predicates/includes",
                    view.name.as_deref().unwrap_or(&view.view_type)
                ));
                walk(&view.body, current_element, diagram, warnings);
            }
            LikeC4Statement::Extend(extension) => {
                walk(&extension.body, Some(&extension.target), diagram, warnings);
            }
            LikeC4Statement::Property(property) => {
                if let Some(current_element) = current_element
                    && let Some(card) = diagram
                        .cards
                        .iter_mut()
                        .find(|card| card.id == current_element)
                {
                    let values = property
                        .values
                        .iter()
                        .map(|value| value.value.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    card.lines.push(format!("{}: {values}", property.name));
                }
                walk(&property.body, current_element, diagram, warnings);
            }
            LikeC4Statement::KindDefinition(_) | LikeC4Statement::Tag(_) => {}
        }
    }
}

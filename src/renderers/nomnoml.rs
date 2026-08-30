use diagram_ast_parser::ast::nomnoml::{
    NomnomlClassifier, NomnomlDocument, NomnomlRelation, NomnomlStatement,
};

use super::RenderPlan;
use super::cards::{Card, CardDiagram, Connector, ConnectorKind};
use crate::Theme;

pub(crate) fn render(document: &NomnomlDocument, theme: &Theme) -> RenderPlan {
    let mut diagram = CardDiagram::new("nomnoml classifier diagram");
    for statement in &document.statements {
        match &statement.node {
            NomnomlStatement::Classifier(classifier) => {
                diagram.push_card(classifier_card(classifier, diagram.cards.len()));
            }
            NomnomlStatement::Relation(relation) => {
                let start = classifier_id(&relation.start, "start");
                let end = classifier_id(&relation.end, "end");
                diagram.push_card(classifier_card(&relation.start, diagram.cards.len()));
                diagram.push_card(classifier_card(&relation.end, diagram.cards.len()));
                diagram
                    .connectors
                    .push(relation_connector(relation, start, end));
            }
        }
    }
    RenderPlan {
        scene: super::cards::render(&mut diagram, theme),
        warnings: Vec::new(),
    }
}

fn classifier_card(classifier: &NomnomlClassifier, color_slot: usize) -> Card {
    let id = classifier_id(classifier, "classifier");
    let title = classifier
        .compartments
        .first()
        .and_then(|compartment| compartment.lines.first())
        .cloned()
        .unwrap_or_else(|| id.clone());
    let mut lines = Vec::new();
    for (index, compartment) in classifier.compartments.iter().enumerate() {
        for line in &compartment.lines {
            if index != 0 || line != &title {
                lines.push(line.clone());
            }
        }
    }
    for (name, value) in &classifier.attributes {
        if name != "id" {
            lines.push(format!("{name}: {value}"));
        }
    }
    Card::new(
        id,
        title,
        classifier.classifier_type.as_deref().unwrap_or("CLASS"),
        lines,
        color_slot,
    )
}

fn classifier_id(classifier: &NomnomlClassifier, fallback: &str) -> String {
    classifier
        .attributes
        .get("id")
        .cloned()
        .or_else(|| {
            classifier
                .compartments
                .first()
                .and_then(|compartment| compartment.lines.first())
                .cloned()
        })
        .unwrap_or_else(|| fallback.to_owned())
}

fn relation_connector(relation: &NomnomlRelation, from: String, to: String) -> Connector {
    let label = match (&relation.start_label, &relation.end_label) {
        (Some(start), Some(end)) => Some(format!("{start} · {end}")),
        (Some(start), None) => Some(start.clone()),
        (None, Some(end)) => Some(end.clone()),
        (None, None) => None,
    };
    let kind = if relation.association.contains('<') && relation.association.contains('>') {
        ConnectorKind::Bidirectional
    } else if relation.association.contains('<') {
        ConnectorKind::Reverse
    } else if relation.association.contains('>') {
        ConnectorKind::Directed
    } else {
        ConnectorKind::Undirected
    };
    Connector {
        from,
        to,
        label,
        kind,
        dashed: relation.association.contains("--") || relation.association.contains("__"),
    }
}

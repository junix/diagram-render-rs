use std::collections::BTreeMap;

use diagram_ast_parser::ast::dbml::{
    DbmlCardinality, DbmlDocument, DbmlEndpoint, DbmlItem, DbmlTableItem,
};

use super::RenderPlan;
use super::cards::{Card, CardDiagram, Connector, ConnectorKind};
use crate::Theme;

pub(crate) fn render(document: &DbmlDocument, theme: &Theme) -> RenderPlan {
    let title = document
        .items
        .iter()
        .find_map(|item| match &item.node {
            DbmlItem::Project(project) => Some(format!("DBML · {}", project.name)),
            _ => None,
        })
        .unwrap_or_else(|| "DBML schema".to_owned());
    let mut diagram = CardDiagram::new(title);
    let mut aliases = BTreeMap::new();
    let mut refs = Vec::new();
    let mut note_index = 0usize;

    for item in &document.items {
        match &item.node {
            DbmlItem::Project(_) => {}
            DbmlItem::Table(table) => {
                let id = qualified(table.schema.as_deref(), &table.name);
                aliases.insert(table.name.clone(), id.clone());
                if let Some(alias) = &table.alias {
                    aliases.insert(alias.clone(), id.clone());
                }
                let mut lines = Vec::new();
                for table_item in &table.items {
                    match &table_item.node {
                        DbmlTableItem::Column(column) => {
                            let mut suffixes = Vec::new();
                            for setting in &column.settings {
                                if setting.name.eq_ignore_ascii_case("pk") {
                                    suffixes.push("PK");
                                } else if setting.name.eq_ignore_ascii_case("unique") {
                                    suffixes.push("UNIQUE");
                                } else if setting.name.eq_ignore_ascii_case("not null") {
                                    suffixes.push("NOT NULL");
                                }
                            }
                            let suffix = if suffixes.is_empty() {
                                String::new()
                            } else {
                                format!(" · {}", suffixes.join(" · "))
                            };
                            lines.push(format!("{}  {}{}", column.name, column.data_type, suffix));
                        }
                        DbmlTableItem::Indexes(indexes) => {
                            lines.push(format!("{} index(es)", indexes.len()));
                        }
                        DbmlTableItem::Check(check) => {
                            lines.push(format!("check {}", check.expression));
                        }
                        DbmlTableItem::Checks(checks) => {
                            lines.push(format!("{} checks", checks.len()));
                        }
                        DbmlTableItem::Note(note) => lines.push(format!("note: {note}")),
                        DbmlTableItem::Partial(name) => lines.push(format!("uses ~{name}")),
                    }
                }
                diagram.push_card(Card::new(id, table.name.clone(), "TABLE", lines, 0));
            }
            DbmlItem::TablePartial(partial) => {
                let lines = partial
                    .items
                    .iter()
                    .filter_map(|item| match &item.node {
                        DbmlTableItem::Column(column) => {
                            Some(format!("{}  {}", column.name, column.data_type))
                        }
                        _ => None,
                    })
                    .collect();
                diagram.push_card(Card::new(
                    format!("partial.{}", partial.name),
                    &partial.name,
                    "TABLE PARTIAL",
                    lines,
                    4,
                ));
            }
            DbmlItem::Enum(enumeration) => {
                let id = qualified(enumeration.schema.as_deref(), &enumeration.name);
                aliases.insert(enumeration.name.clone(), id.clone());
                let lines = enumeration
                    .values
                    .iter()
                    .map(|value| value.node.name.clone())
                    .collect();
                diagram.push_card(Card::new(id, &enumeration.name, "ENUM", lines, 1));
            }
            DbmlItem::Ref(reference) => refs.push(reference),
            DbmlItem::TableGroup(group) => {
                diagram.push_card(Card::new(
                    format!("group.{}", group.name),
                    &group.name,
                    "TABLE GROUP",
                    group.tables.clone(),
                    2,
                ));
            }
            DbmlItem::Note(note) => {
                note_index += 1;
                diagram.push_card(Card::new(
                    format!("note.{note_index}"),
                    format!("Note {note_index}"),
                    "SCHEMA NOTE",
                    vec![note.clone()],
                    4,
                ));
            }
        }
    }

    for reference in refs {
        let from = resolve_endpoint(&reference.from, &aliases, &diagram);
        let to = resolve_endpoint(&reference.to, &aliases, &diagram);
        diagram.ensure_endpoint(&from);
        diagram.ensure_endpoint(&to);
        let columns = format!(
            "{} {} {}",
            reference.from.columns.join(","),
            cardinality(reference.cardinality),
            reference.to.columns.join(",")
        );
        let label = reference
            .name
            .as_ref()
            .map_or_else(|| columns.clone(), |name| format!("{name} · {columns}"));
        diagram.connectors.push(Connector {
            from,
            to,
            label: Some(label),
            kind: ConnectorKind::Directed,
            dashed: false,
        });
    }

    RenderPlan {
        scene: super::cards::render(&mut diagram, theme),
        warnings: Vec::new(),
    }
}

fn qualified(schema: Option<&str>, name: &str) -> String {
    schema.map_or_else(|| name.to_owned(), |schema| format!("{schema}.{name}"))
}

fn resolve_endpoint(
    endpoint: &DbmlEndpoint,
    aliases: &BTreeMap<String, String>,
    diagram: &CardDiagram,
) -> String {
    let full = qualified(endpoint.schema.as_deref(), &endpoint.table);
    if diagram.cards.iter().any(|card| card.id == full) {
        return full;
    }
    aliases.get(&endpoint.table).cloned().unwrap_or(full)
}

fn cardinality(cardinality: DbmlCardinality) -> &'static str {
    match cardinality {
        DbmlCardinality::ManyToOne => "N:1",
        DbmlCardinality::OneToMany => "1:N",
        DbmlCardinality::OneToOne => "1:1",
        DbmlCardinality::ManyToMany => "N:N",
    }
}

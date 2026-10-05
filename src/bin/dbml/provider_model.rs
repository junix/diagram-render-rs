//! Adapter-owned authored schema, not an upstream AST wire-format promise.
use anyhow::{Result, ensure};
use diagram_ast_parser::{
    Located, Span,
    ast::{Document, dbml::*},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Authored {
    schema_version: String,
    tables: Vec<Table>,
    refs: Vec<Reference>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    name: String,
    columns: Vec<Column>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Column {
    name: String,
    data_type: String,
    flags: Vec<Flag>,
}
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
enum Flag {
    #[serde(rename = "pk")]
    Pk,
    #[serde(rename = "unique")]
    Unique,
    #[serde(rename = "not null")]
    NotNull,
}
impl Flag {
    fn setting(self) -> &'static str {
        match self {
            Self::Pk => "pk",
            Self::Unique => "unique",
            Self::NotNull => "not null",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Pk => "PK",
            Self::Unique => "UNIQUE",
            Self::NotNull => "NOT NULL",
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    from: Endpoint,
    to: Endpoint,
    cardinality: Cardinality,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    table: String,
    column: String,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Cardinality {
    ManyToOne,
    OneToMany,
    OneToOne,
    ManyToMany,
}
impl Cardinality {
    fn native(self) -> DbmlCardinality {
        match self {
            Self::ManyToOne => DbmlCardinality::ManyToOne,
            Self::OneToMany => DbmlCardinality::OneToMany,
            Self::OneToOne => DbmlCardinality::OneToOne,
            Self::ManyToMany => DbmlCardinality::ManyToMany,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Counts {
    pub tables: usize,
    pub columns: usize,
    pub refs: usize,
}
fn identifier(value: &str, max: usize, underscore: bool) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .as_bytes()
            .first()
            .is_some_and(|c| c.is_ascii_alphabetic() || (underscore && *c == b'_'))
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
impl Authored {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= super::provider_io::MAX_INPUT_BYTES,
            "authored input exceeds 64 KiB"
        );
        super::provider_json::validate(bytes)?;
        let value: Self = serde_json::from_slice(bytes)?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == "dbml.authored/v1",
            "require schema_version dbml.authored/v1"
        );
        ensure!((1..=2).contains(&self.tables.len()), "require 1..2 tables");
        ensure!(
            self.refs.len() <= 1,
            "at most one explicit reference is supported"
        );
        let mut tables = BTreeSet::new();
        for table in &self.tables {
            ensure!(
                identifier(&table.name, 12, true),
                "table name must be an ASCII identifier of 1..12 bytes"
            );
            ensure!(tables.insert(&table.name), "duplicate table identity");
            ensure!(
                (1..=16).contains(&table.columns.len()),
                "require 1..16 columns per table"
            );
            let mut columns = BTreeSet::new();
            for column in &table.columns {
                ensure!(
                    identifier(&column.name, 12, true),
                    "column name must be an ASCII identifier of 1..12 bytes"
                );
                ensure!(
                    identifier(&column.data_type, 12, false),
                    "data_type must be an ASCII type token of 1..12 bytes"
                );
                ensure!(columns.insert(&column.name), "duplicate column identity");
                let flags: BTreeSet<_> = column.flags.iter().collect();
                ensure!(flags.len() == column.flags.len(), "duplicate column flag");
                let suffix = if column.flags.is_empty() {
                    String::new()
                } else {
                    format!(
                        " · {}",
                        column
                            .flags
                            .iter()
                            .map(|f| f.label())
                            .collect::<Vec<_>>()
                            .join(" · ")
                    )
                };
                // ASCII identifiers plus middot (one display cell): exact native composition.
                let line = format!("{}  {}{}", column.name, column.data_type, suffix);
                ensure!(
                    unicode_width::UnicodeWidthStr::width(line.as_str()) <= 18,
                    "column label exceeds 18 display cells"
                );
            }
        }
        for reference in &self.refs {
            ensure!(
                reference.from.table != reference.to.table,
                "self references are outside this profile"
            );
            for endpoint in [&reference.from, &reference.to] {
                ensure!(
                    identifier(&endpoint.table, 12, true) && identifier(&endpoint.column, 12, true),
                    "invalid reference identity"
                );
                let table = self
                    .tables
                    .iter()
                    .find(|t| t.name == endpoint.table)
                    .ok_or_else(|| anyhow::anyhow!("unresolved reference table"))?;
                ensure!(
                    table.columns.iter().any(|c| c.name == endpoint.column),
                    "unresolved reference column"
                );
            }
            // The native label is "from_column N:1 to_column" for all cardinalities.
            ensure!(
                reference.from.column.len() + 5 + reference.to.column.len() <= 11,
                "reference label exceeds 11 display cells"
            );
        }
        Ok(())
    }
    pub(crate) fn counts(&self) -> Counts {
        Counts {
            tables: self.tables.len(),
            columns: self.tables.iter().map(|t| t.columns.len()).sum(),
            refs: self.refs.len(),
        }
    }
    pub(crate) fn native(&self) -> Document {
        // This authored profile has no source positions; zero spans are synthetic.
        let span = Span::new(0, 0);
        let mut items: Vec<_> = self
            .tables
            .iter()
            .map(|t| {
                Located::new(
                    span,
                    DbmlItem::Table(DbmlTable {
                        schema: None,
                        name: t.name.clone(),
                        alias: None,
                        settings: vec![],
                        items: t
                            .columns
                            .iter()
                            .map(|c| {
                                Located::new(
                                    span,
                                    DbmlTableItem::Column(DbmlColumn {
                                        name: c.name.clone(),
                                        data_type: c.data_type.clone(),
                                        settings: c
                                            .flags
                                            .iter()
                                            .map(|f| DbmlSetting {
                                                name: f.setting().into(),
                                                value: None,
                                            })
                                            .collect(),
                                    }),
                                )
                            })
                            .collect(),
                    }),
                )
            })
            .collect();
        for r in &self.refs {
            let endpoint = |e: &Endpoint| DbmlEndpoint {
                schema: None,
                table: e.table.clone(),
                columns: vec![e.column.clone()],
            };
            items.push(Located::new(
                span,
                DbmlItem::Ref(DbmlRef {
                    name: None,
                    from: endpoint(&r.from),
                    to: endpoint(&r.to),
                    cardinality: r.cardinality.native(),
                    settings: vec![],
                }),
            ));
        }
        Document::Dbml(DbmlDocument { span, items })
    }
}

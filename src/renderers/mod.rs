mod cards;
mod d2;
mod dbml;
mod likec4;
mod nomnoml;
mod pikchr;
mod structurizr;
mod wavedrom;

use diagram_ast_parser::ast::Document;

use crate::{Scene, Theme};

pub(crate) struct RenderPlan {
    pub scene: Scene,
    pub warnings: Vec<String>,
}

pub(crate) fn render(document: &Document, theme: &Theme) -> RenderPlan {
    match document {
        Document::Dbml(document) => dbml::render(document, theme),
        Document::WaveDrom(document) => wavedrom::render(document, theme),
        Document::D2(document) => d2::render(document, theme),
        Document::Structurizr(document) => structurizr::render(document, theme),
        Document::LikeC4(document) => likec4::render(document, theme),
        Document::Nomnoml(document) => nomnoml::render(document, theme),
        Document::Pikchr(document) => pikchr::render(document, theme),
    }
}

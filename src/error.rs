use thiserror::Error;

pub type Result<T> = std::result::Result<T, RenderError>;

/// Failure at one of the explicit parser, scene, SVG, or PNG boundaries.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error(transparent)]
    Parse(#[from] diagram_ast_parser::ParseError),

    #[error("AST JSON decode failed: {0}")]
    AstJson(#[from] serde_json::Error),

    #[error("invalid render option: {0}")]
    InvalidOption(String),

    #[error("invalid scene: {0}")]
    InvalidScene(String),

    #[error("SVG parse failed: {0}")]
    Svg(String),

    #[error("PNG render failed: {0}")]
    Png(String),
}

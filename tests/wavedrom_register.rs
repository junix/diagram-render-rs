use diagram_ast_parser::Span;
use diagram_ast_parser::ast::wavedrom::{WaveDromDocument, WaveRegisterDiagram, WaveRegisterField};
use diagram_render_rs::{
    DiagramFormat, Document, OutputFormat, RenderError, RenderOptions, render_document,
    render_source,
};

fn register_document(widths: &[Option<u64>]) -> Document {
    Document::WaveDrom(WaveDromDocument {
        span: Span::default(),
        timing: None,
        register: Some(WaveRegisterDiagram {
            fields: widths
                .iter()
                .map(|&bits| WaveRegisterField {
                    bits,
                    name: None,
                    attr: None,
                    field_type: None,
                    extra: serde_json::Map::new(),
                })
                .collect(),
            config: None,
        }),
        extra: serde_json::Map::new(),
    })
}

#[test]
fn public_ast_rejects_register_overflow_for_svg_and_png() {
    // Construct the public AST directly: JSON5 number parsing need not preserve
    // every u64 value, and a huge bit count must never allocate per-bit storage.
    for tail in [Some(1), Some(0), None, Some(u64::MAX)] {
        let document = register_document(&[Some(u64::MAX), tail]);
        for output in [OutputFormat::Svg, OutputFormat::Png] {
            let error = render_document(&document, output, &RenderOptions::default())
                .expect_err("aggregate register width must be checked before rendering");
            match error {
                RenderError::InvalidScene(message) => {
                    assert_eq!(
                        message,
                        "WaveDrom register total bit width exceeds u64::MAX"
                    );
                }
                other => panic!("expected invalid scene, got {other}"),
            }
        }
    }
}

#[test]
fn public_ast_accepts_the_maximum_register_width_without_expanding_bits() {
    let document = register_document(&[Some(u64::MAX)]);
    let rendered = render_document(&document, OutputFormat::Svg, &RenderOptions::default())
        .expect("one u64::MAX field has a representable aggregate");
    assert!(rendered.svg.contains("18446744073709551614:0"));
    assert_eq!(rendered.svg.matches("<rect ").count(), 1);
    assert!(rendered.warnings.is_empty());
}

#[test]
fn parsed_registers_normalize_missing_and_zero_widths_for_svg_and_png() {
    for source in [
        "{reg:[{bits:0},{bits:0},{bits:0}]}",
        "{reg:[{bits:0},{},{bits:2}]}",
        "{reg:[{bits:2},{bits:6}]}",
    ] {
        let rendered = render_source(
            DiagramFormat::WaveDrom,
            source,
            OutputFormat::Png,
            &RenderOptions::default(),
        )
        .expect("register renders with consistent normalized widths");
        assert!(rendered.warnings.is_empty());
        assert!(!rendered.svg.contains("NaN"));
        assert!(!rendered.svg.contains("width=\"-"));
        let png = rendered.png.expect("PNG requested");
        let decoded = resvg::tiny_skia::Pixmap::decode_png(&png).expect("valid PNG");
        assert_eq!((decoded.width(), decoded.height()), (660, 220));
    }
}

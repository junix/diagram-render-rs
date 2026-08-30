use crate::{RenderError, Result};

const MAX_DIMENSION: u32 = 32_768;
const MAX_PIXELS: u64 = 100_000_000;

pub(crate) struct Raster {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub(crate) fn svg_to_png(svg: &str, scale: f32, width: Option<u32>) -> Result<Raster> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|error| RenderError::Svg(error.to_string()))?;
    let source_size = tree.size();
    let effective_scale = width.map_or(scale, |requested| {
        requested as f32 / source_size.width().max(1.0)
    });

    if !effective_scale.is_finite() || !(0.05..=16.0).contains(&effective_scale) {
        return Err(RenderError::InvalidOption(
            "effective PNG scale must be between 0.05 and 16.0".to_owned(),
        ));
    }

    let pixel_width = (source_size.width() * effective_scale).ceil() as u32;
    let pixel_height = (source_size.height() * effective_scale).ceil() as u32;
    if pixel_width == 0 || pixel_height == 0 {
        return Err(RenderError::Png("rendered image has zero size".to_owned()));
    }
    if pixel_width > MAX_DIMENSION || pixel_height > MAX_DIMENSION {
        return Err(RenderError::Png(format!(
            "rendered image exceeds {MAX_DIMENSION}px dimension limit ({pixel_width}x{pixel_height})"
        )));
    }
    if u64::from(pixel_width) * u64::from(pixel_height) > MAX_PIXELS {
        return Err(RenderError::Png(format!(
            "rendered image exceeds {MAX_PIXELS} pixel limit ({pixel_width}x{pixel_height})"
        )));
    }

    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixel_width, pixel_height)
        .ok_or_else(|| RenderError::Png("could not allocate PNG pixel buffer".to_owned()))?;
    let transform = resvg::tiny_skia::Transform::from_scale(effective_scale, effective_scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let bytes = pixmap
        .encode_png()
        .map_err(|error| RenderError::Png(error.to_string()))?;
    Ok(Raster {
        bytes,
        width: pixel_width,
        height: pixel_height,
    })
}

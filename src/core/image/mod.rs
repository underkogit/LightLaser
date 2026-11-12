pub(crate) mod engraver;
mod loader;
mod palette;
mod raster;
pub(crate) mod svg_raster;
pub(crate) mod svg_trace;

pub use engraver::{EngraveError, EngraveSegment, engrave_layer, engrave_segments};
pub use loader::{LoadImageError, LoadedImage, load_image};
pub use palette::{PaletteColor, extract_palette};
pub use raster::{RasterImage, RasterizeOptions, rasterize};
pub use svg_raster::{RasterizedSvg, rasterize_svg};
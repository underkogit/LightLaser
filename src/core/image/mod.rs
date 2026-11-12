pub(crate) mod engraver;
mod loader;
mod palette;
mod raster;
mod svg_raster;

pub use engraver::{engrave_layer, engrave_segments, EngraveError, EngraveSegment};
pub use loader::{load_image, LoadImageError, LoadedImage};
pub use palette::{extract_palette, PaletteColor};
pub use raster::{rasterize, RasterImage, RasterizeOptions};
pub use svg_raster::{rasterize_svg, RasterizedSvg};
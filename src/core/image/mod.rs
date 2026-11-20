pub(crate) mod engraver;
pub(crate) mod gerber;
mod loader;
mod palette;
mod raster;
pub(crate) mod svg_raster;
pub(crate) mod svg_size;
pub(crate) mod svg_trace;

pub use engraver::{EngraveError, EngraveSegment, engrave_layer, engrave_segments};
pub use gerber::parse_gerber_segments;
pub use loader::{LoadImageError, LoadedImage, load_image};
pub use palette::{PaletteColor, extract_palette};
pub use raster::{RasterImage, RasterizeOptions, rasterize};
pub use svg_raster::{rasterize_svg_into_box, svg_raster_box};
pub use svg_size::{SvgImportProfile, svg_size_mm};

pub mod primitives;
pub mod transform;
pub mod line;
pub mod polygon;
pub mod region;
pub mod math3d;

pub use primitives::*;
pub use transform::*;
pub use line::*;
pub use polygon::*;
pub use region::*;
pub use math3d::*;

// --- Qt Canonical Aliases ---
pub type QLine = line::Line;
pub type QLineF = line::LineF;
pub type QPolygon = polygon::Polygon;
pub type QPolygonF = polygon::PolygonF;
pub type QRegion = region::Region;

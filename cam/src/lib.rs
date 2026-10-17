//! Machining geometry as Fidget expressions: cutters, the paths they follow,
//! and the stock they cut. Nothing here knows the editor or the document. It
//! builds optimized like the other dependencies, so its tests' dense reference
//! checks don't run at the editor's debug level.

pub mod cutter;
pub mod field;
pub mod paths;
pub mod stock;

//! Progred's built-in libraries. Most modules are one library's complete
//! contribution to the editor, through the interface in [`crate::library`];
//! `completion`, `line_edit` and `representation` are helpers several share.

/// A built-in library's contribution, over a world that hosts the toolkit.
pub type Library<W> = crate::library::Library<W, <W as crate::display::Host>::Hover>;

pub mod absent;
pub mod blob;
pub mod color;
pub mod completion;
#[cfg(test)]
mod conformance;
pub mod control;
pub mod controls;
pub mod conversion;
pub mod f32;
pub mod f64;
pub mod fidget;
pub mod geometry;
pub mod grap;
pub mod layout;
pub mod line_edit;
pub mod list;
pub mod logic;
pub mod name;
pub mod number;
pub mod path;
pub mod png;
pub mod presentation;
pub mod random;
pub mod representation;
pub mod selection;
pub mod sequence;
pub mod site;
pub mod text;
pub mod toolpath;
pub mod tree;
pub mod u64;
pub mod workspace;

#[cfg(test)]
pub(crate) mod test_widgets;

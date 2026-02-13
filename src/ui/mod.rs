pub mod d;
mod identicon;
pub mod graph_view;
pub mod placeholder;
pub mod projection;
pub mod render;
pub mod split_view;
pub mod tree_view;

pub use identicon::identicon;
pub use projection::layout;
pub use projection::{insertion_point, project, InteractionMode};

use eframe::egui::Color32;

pub mod colors {
    use super::Color32;

    pub const SELECTION: Color32 = Color32::from_rgb(59, 130, 246);
    pub const ASSIGN: Color32 = Color32::from_rgb(234, 179, 8);
    pub const SELECT_UNDER: Color32 = Color32::from_rgb(34, 197, 94);
}

use eframe::egui::{Color32, Pos2, Response, Rect, Rounding, Ui, Vec2};
use std::collections::hash_map::DefaultHasher;
use std::hash::{BuildHasher, BuildHasherDefault};
use uuid::Uuid;

const GRID_SIZE: usize = 5;

pub fn identicon(ui: &mut Ui, size: f32, uuid: &Uuid) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), eframe::egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let hash = BuildHasherDefault::<DefaultHasher>::default().hash_one(uuid);
        let (pattern, color) = extract_pattern_and_color(hash);
        draw_identicon(ui.painter(), rect, &pattern, color);
    }
    response
}

fn extract_pattern_and_color(hash: u64) -> ([[bool; GRID_SIZE]; GRID_SIZE], Color32) {
    let pattern_bits = hash & 0x7FFF;
    let hue = ((hash >> 15) & 0xFF) as u8;

    let mut pattern = [[false; GRID_SIZE]; GRID_SIZE];
    for row in 0..GRID_SIZE {
        for col in 0..3 {
            let bit_index = row * 3 + col;
            let is_on = (pattern_bits >> bit_index) & 1 == 1;
            pattern[row][col] = is_on;
            pattern[row][GRID_SIZE - 1 - col] = is_on;
        }
    }

    (pattern, hsl_to_rgb(hue, 0.65, 0.50))
}

fn draw_identicon(
    painter: &eframe::egui::Painter,
    rect: Rect,
    pattern: &[[bool; GRID_SIZE]; GRID_SIZE],
    foreground: Color32,
) {
    let background = Color32::from_gray(250);
    let border = Color32::from_gray(180);
    let rounding = Rounding::same(2.0);
    
    // Draw background with border
    painter.rect_filled(rect, rounding, background);
    painter.rect_stroke(rect, rounding, eframe::epaint::Stroke::new(1.0, border));

    // Inset slightly for the pattern
    let inset = 1.0;
    let inner_rect = rect.shrink(inset);
    
    let cell_size = Vec2::new(
        inner_rect.width() / GRID_SIZE as f32,
        inner_rect.height() / GRID_SIZE as f32,
    );

    for row in 0..GRID_SIZE {
        for col in 0..GRID_SIZE {
            if pattern[row][col] {
                let cell_rect = Rect::from_min_size(
                    Pos2::new(
                        inner_rect.min.x + col as f32 * cell_size.x,
                        inner_rect.min.y + row as f32 * cell_size.y,
                    ),
                    cell_size,
                );
                painter.rect_filled(cell_rect, Rounding::ZERO, foreground);
            }
        }
    }
}

fn hsl_to_rgb(hue: u8, saturation: f32, lightness: f32) -> Color32 {
    let h = hue as f32 / 255.0 * 360.0;
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = lightness - c / 2.0;

    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(uuid: &Uuid) -> u64 {
        BuildHasherDefault::<DefaultHasher>::default().hash_one(uuid)
    }

    #[test]
    fn same_uuid_same_pattern() {
        let uuid = Uuid::parse_str("a1b2c3d4-e5f6-a1b2-c3d4-e5f6a1b2c3d4").unwrap();
        let hash1 = hash(&uuid);
        let hash2 = hash(&uuid);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_uuids_different_patterns() {
        let uuid1 = Uuid::parse_str("a1b2c3d4-e5f6-a1b2-c3d4-e5f6a1b2c3d4").unwrap();
        let uuid2 = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        assert_ne!(hash(&uuid1), hash(&uuid2));
    }

    #[test]
    fn pattern_is_symmetric() {
        let hash = 0x12345678u64;
        let (pattern, _) = extract_pattern_and_color(hash);
        for row in 0..GRID_SIZE {
            assert_eq!(pattern[row][0], pattern[row][4]);
            assert_eq!(pattern[row][1], pattern[row][3]);
        }
    }
}

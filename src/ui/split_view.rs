use eframe::egui::{self, Color32, Context, Rect, Sense, Stroke, Ui};

pub fn horizontal_split(
    ui: &mut Ui,
    ctx: &Context,
    split: &mut f32,
    contents: impl FnOnce(&mut Ui, &mut Ui),
) {
    let full_rect = ui.max_rect();
    let (separator_width, separator_hit_width) = (1.0, 8.0);
    let left_width = (full_rect.width() - separator_width) * (1.0 - *split);
    let separator_x = full_rect.min.x + left_width;

    let left_rect = Rect::from_min_max(
        full_rect.min,
        egui::pos2(separator_x, full_rect.max.y),
    );
    let right_rect = Rect::from_min_max(
        egui::pos2(separator_x + separator_width, full_rect.min.y),
        full_rect.max,
    );

    let separator_response = ui.allocate_rect(
        Rect::from_center_size(
            egui::pos2(separator_x, full_rect.center().y),
            egui::vec2(separator_hit_width, full_rect.height()),
        ),
        Sense::drag(),
    );
    if separator_response.dragged() {
        let new_left = left_width + separator_response.drag_delta().x;
        *split = (1.0 - new_left / (full_rect.width() - separator_width)).clamp(0.1, 0.9);
    }
    if separator_response.hovered() || separator_response.dragged() {
        ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    ui.painter().vline(
        separator_x,
        full_rect.y_range(),
        Stroke::new(separator_width, Color32::from_gray(180)),
    );

    let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect));
    left_ui.set_clip_rect(left_rect);
    let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right_rect));
    right_ui.set_clip_rect(right_rect);
    contents(&mut left_ui, &mut right_ui);
}

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Progred"),
        ..Default::default()
    };

    eframe::run_native(
        "Progred",
        options,
        Box::new(|_cc| Ok(Box::new(ProgredApp::default()))),
    )
}

struct ProgredApp {
    name: String,
}

impl Default for ProgredApp {
    fn default() -> Self {
        Self {
            name: String::from(""),
        }
    }
}

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Progred - Graph Editor");

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.name);
            });

            if ui.button("Click me").clicked() {
                println!("Button clicked! Name: {}", self.name);
            }
        });
    }
}

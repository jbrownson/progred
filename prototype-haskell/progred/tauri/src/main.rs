#[cfg(debug_assertions)]
const TOGGLE_DEVTOOLS: &str = "toggle-devtools";

#[cfg(debug_assertions)]
use tauri::menu::MenuItemBuilder;
use tauri::menu::{MenuBuilder, SubmenuBuilder};
#[cfg(debug_assertions)]
use tauri::Manager;

fn main() {
    let builder = tauri::Builder::default().setup(|app| {
        let app_menu = SubmenuBuilder::new(app, "Progred").quit().build()?;

        #[cfg(debug_assertions)]
        let menu = {
            let toggle_devtools =
                MenuItemBuilder::with_id(TOGGLE_DEVTOOLS, "Toggle Developer Tools")
                    .accelerator("CmdOrCtrl+Alt+I")
                    .build(app)?;
            let view = SubmenuBuilder::new(app, "View")
                .item(&toggle_devtools)
                .build()?;
            MenuBuilder::new(app).item(&app_menu).item(&view).build()?
        };

        #[cfg(not(debug_assertions))]
        let menu = MenuBuilder::new(app).item(&app_menu).build()?;

        app.set_menu(menu)?;

        Ok(())
    });

    #[cfg(debug_assertions)]
    let builder = builder.on_menu_event(|app, event| {
        if event.id() == TOGGLE_DEVTOOLS {
            let webview = app.get_webview_window("main").unwrap();
            if webview.is_devtools_open() {
                webview.close_devtools();
            } else {
                webview.open_devtools();
            }
        }
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

#[cfg(debug_assertions)]
const TOGGLE_DEVTOOLS: &str = "toggle-devtools";
const TOGGLE_LAYOUT_DEBUG_RECTS: &str = "toggle-layout-debug-rects";

use tauri::menu::CheckMenuItemBuilder;
#[cfg(debug_assertions)]
use tauri::menu::MenuItemBuilder;
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::Manager;

fn main() {
    let builder = tauri::Builder::default().setup(|app| {
        let app_menu = SubmenuBuilder::new(app, "Progred").quit().build()?;

        let toggle_layout_debug_rects =
            CheckMenuItemBuilder::with_id(TOGGLE_LAYOUT_DEBUG_RECTS, "Toggle Layout Debug Rects")
                .checked(false)
                .accelerator("CmdOrCtrl+Shift+D")
                .build(app)?;

        let view = SubmenuBuilder::new(app, "View").item(&toggle_layout_debug_rects);

        #[cfg(debug_assertions)]
        let view = {
            let toggle_devtools =
                MenuItemBuilder::with_id(TOGGLE_DEVTOOLS, "Toggle Developer Tools")
                    .accelerator("CmdOrCtrl+Alt+I")
                    .build(app)?;
            view.item(&toggle_devtools)
        };

        let view = view.build()?;
        let menu = MenuBuilder::new(app).item(&app_menu).item(&view).build()?;

        app.set_menu(menu)?;

        Ok(())
    });

    let builder = builder.on_menu_event(|app, event| {
        if event.id() == TOGGLE_LAYOUT_DEBUG_RECTS {
            if let Some(webview) = app.get_webview_window("main") {
                let _ = webview.eval("window.progred?.toggleLayoutDebugRects?.();");
            }
        }

        #[cfg(debug_assertions)]
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

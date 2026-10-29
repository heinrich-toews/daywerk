// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Tauri shell for Daywerk.
//!
//! The app is a *client*: it talks to a Daywerk server over the REST API, so
//! the shell only has to host the web UI and expose the few native bits the UI
//! needs. Everything else lives in `apps/web-ui`.

use tauri::Manager;

/// Where the app should look for its server. Stored per install so a device
/// keeps its address between launches.
#[tauri::command]
fn default_server_url() -> Option<String> {
    std::env::var("DAYWERK_SERVER_URL").ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_haptics::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main");
                if let Some(window) = window {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![default_server_url])
        .run(tauri::generate_context!())
        .expect("error while running Daywerk");
}

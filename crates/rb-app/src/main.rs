mod commands;
mod dto;
mod error;
mod state;

use commands::{cancel_scan, export_report, repo_info, select_directory, start_scan};
use state::ScanState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ScanState::new())
        .invoke_handler(tauri::generate_handler![
            select_directory,
            repo_info,
            start_scan,
            cancel_scan,
            export_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

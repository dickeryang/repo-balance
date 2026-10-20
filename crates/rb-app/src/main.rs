mod commands;
mod dto;
mod error;
mod state;

use commands::{
    cancel_scan, export_custom_rules, export_report, export_scan_history, import_custom_rules,
    import_scan_history, load_scan_history, repo_info, save_scan_history, scan_estimate,
    select_directories, select_directory, start_remote_scan, start_scan,
};
use state::ScanState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ScanState::new())
        .invoke_handler(tauri::generate_handler![
            select_directory,
            select_directories,
            repo_info,
            scan_estimate,
            start_scan,
            start_remote_scan,
            cancel_scan,
            export_report,
            load_scan_history,
            save_scan_history,
            import_scan_history,
            export_scan_history,
            import_custom_rules,
            export_custom_rules
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

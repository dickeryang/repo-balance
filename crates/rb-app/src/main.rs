mod commands;
mod dto;
mod error;
mod state;

use commands::{
    cancel_scan, check_ignore_rules, export_custom_rules, export_report, export_scan_history,
    ignore_defaults, import_custom_rules, import_scan_history, import_vuln_index,
    load_scan_history, read_ignore_rules, repo_info, save_scan_history, scan_estimate,
    select_directories, select_directory, start_remote_scan, start_scan, vuln_index_info,
    write_ignore_rules,
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
            export_custom_rules,
            import_vuln_index,
            vuln_index_info,
            read_ignore_rules,
            write_ignore_rules,
            check_ignore_rules,
            ignore_defaults
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

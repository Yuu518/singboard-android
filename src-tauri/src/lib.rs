pub mod commands;
pub mod native;
pub mod root;

use commands::{app, binary, config, network, self_update, service, srs, update};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(native::init())
        .invoke_handler(tauri::generate_handler![
            app::root_check,
            app::set_root_enabled,
            app::set_system_bars,
            app::system_insets,
            service::service_status,
            service::service_start,
            service::service_stop,
            service::service_restart,
            service::service_install,
            service::service_component_sync,
            service::service_error_log,
            service::service_boot_hook_exists,
            service::service_create_boot_hook,
            service::service_delete_boot_hook,
            config::validate_config,
            config::detect_runtime_files,
            binary::get_singbox_version,
            binary::get_file_hash,
            srs::srs_match_provider,
            srs::srs_list_provider,
            network::fetch_url,
            network::http_ping,
            network::dns_query,
            update::check_core_update,
            update::probe_asset_core_hash,
            update::perform_core_update,
            self_update::check_panel_update,
            self_update::perform_panel_update,
            self_update::cleanup_panel_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

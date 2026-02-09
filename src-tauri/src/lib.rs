pub mod acp;
pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize the database
    let conn = db::migrations::init_db().expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            } else {
                // Also log in release mode but at info level
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .manage(AppState::new(conn))
        .invoke_handler(tauri::generate_handler![
            // Agent commands
            commands::agent_commands::list_agents,
            commands::agent_commands::get_agent,
            commands::agent_commands::create_agent,
            commands::agent_commands::update_agent,
            commands::agent_commands::delete_agent,
            commands::agent_commands::set_control_hub,
            commands::agent_commands::get_control_hub,
            // Session commands
            commands::session_commands::create_session,
            commands::session_commands::list_sessions,
            commands::session_commands::load_session,
            commands::session_commands::delete_session,
            // Chat commands
            commands::chat_commands::send_prompt,
            commands::chat_commands::cancel_prompt,
            commands::chat_commands::get_messages,
            commands::chat_commands::respond_permission,
            // ACP commands
            commands::acp_commands::discover_agents,
            commands::acp_commands::spawn_agent,
            commands::acp_commands::initialize_agent,
            commands::acp_commands::create_acp_session,
            commands::acp_commands::get_agent_status,
            commands::acp_commands::stop_agent,
            // Orchestration commands
            commands::orchestration_commands::start_orchestration,
            commands::orchestration_commands::cancel_orchestration,
            commands::orchestration_commands::list_task_runs,
            commands::orchestration_commands::get_task_run,
            commands::orchestration_commands::get_task_assignments,
            // Settings commands
            commands::settings_commands::get_settings,
            commands::settings_commands::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

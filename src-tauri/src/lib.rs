pub mod acp;
pub mod chat_tool;
pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod scheduler;
pub mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize the database
    let conn = db::migrations::init_db().expect("Failed to initialize database");

    // Create app state before building
    let app_state = AppState::new(conn);

    // Reset stale chat tool statuses from previous session
    match db::chat_tool_repo::reset_stale_statuses(&app_state) {
        Ok(count) if count > 0 => {
            log::info!("Reset {} stale chat tool statuses on startup", count);
        }
        Err(e) => {
            log::warn!("Failed to reset stale chat tool statuses: {}", e);
        }
        _ => {}
    }

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

            // Start the scheduler using Tauri's async runtime
            let app_handle = app.handle().clone();
            let state = app.state::<AppState>().inner().clone();
            let scheduler_ref = state.scheduler.clone();

            tauri::async_runtime::spawn(async move {
                let scheduler_state = scheduler::start_scheduler(app_handle, state);
                let mut scheduler = scheduler_ref.lock().await;
                *scheduler = Some(scheduler_state);
            });

            // Resume incomplete orchestration tasks from previous session
            let app_handle2 = app.handle().clone();
            let state2 = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                acp::orchestrator::resume_incomplete_tasks(app_handle2, state2).await;
            });

            Ok(())
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Agent commands
            commands::agent_commands::list_agents,
            commands::agent_commands::get_agent,
            commands::agent_commands::create_agent,
            commands::agent_commands::update_agent,
            commands::agent_commands::delete_agent,
            commands::agent_commands::set_control_hub,
            commands::agent_commands::get_control_hub,
            commands::agent_commands::enable_agent,
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
            commands::chat_commands::save_generated_file,
            commands::chat_commands::open_file_with_default_app,
            // ACP commands
            commands::acp_commands::discover_agents,
            commands::acp_commands::spawn_agent,
            commands::acp_commands::initialize_agent,
            commands::acp_commands::create_acp_session,
            commands::acp_commands::get_agent_status,
            commands::acp_commands::stop_agent,
            commands::acp_commands::get_agent_models,
            commands::acp_commands::end_acp_session,
            commands::acp_commands::resume_acp_session,
            commands::acp_commands::ensure_agent_ready,
            commands::acp_commands::install_registry_agent,
            commands::acp_commands::uninstall_registry_agent,
            // Orchestration commands
            commands::orchestration_commands::start_orchestration,
            commands::orchestration_commands::cancel_orchestration,
            commands::orchestration_commands::cancel_agent,
            commands::orchestration_commands::list_task_runs,
            commands::orchestration_commands::get_task_run,
            commands::orchestration_commands::update_task_run_status,
            commands::orchestration_commands::get_task_assignments,
            commands::orchestration_commands::confirm_orchestration,
            commands::orchestration_commands::regenerate_agent,
            commands::orchestration_commands::respond_orch_permission,
            commands::orchestration_commands::rate_task_run,
            commands::orchestration_commands::schedule_task,
            commands::orchestration_commands::pause_scheduled_task,
            commands::orchestration_commands::resume_scheduled_task,
            commands::orchestration_commands::clear_schedule,
            commands::orchestration_commands::discover_workspace_skills,
            // Settings commands
            commands::settings_commands::get_settings,
            commands::settings_commands::update_settings,
            commands::settings_commands::select_working_directory,
            commands::settings_commands::get_working_directory,
            // Workspace commands
            commands::workspace_commands::list_workspaces,
            commands::workspace_commands::create_workspace,
            commands::workspace_commands::update_workspace,
            commands::workspace_commands::delete_workspace,
            commands::workspace_commands::select_workspace_directory,
            // Chat tool commands
            commands::chat_tool_commands::list_chat_tools,
            commands::chat_tool_commands::get_chat_tool,
            commands::chat_tool_commands::create_chat_tool,
            commands::chat_tool_commands::update_chat_tool,
            commands::chat_tool_commands::delete_chat_tool,
            commands::chat_tool_commands::start_chat_tool,
            commands::chat_tool_commands::stop_chat_tool,
            commands::chat_tool_commands::logout_chat_tool,
            commands::chat_tool_commands::get_chat_tool_qr_code,
            commands::chat_tool_commands::list_chat_tool_messages,
            commands::chat_tool_commands::send_chat_tool_message,
            commands::chat_tool_commands::list_chat_tool_contacts,
            commands::chat_tool_commands::set_chat_tool_contact_blocked,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

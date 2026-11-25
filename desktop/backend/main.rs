#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
#[path = "ai-verification/mod.rs"]
mod ai_verification;
mod providers;
mod source;

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    let url = url::Url::parse(&url).map_err(|_| "外部链接无效。")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("仅支持打开 HTTP 或 HTTPS 外部链接。".into());
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|_| "无法打开外部链接。".into())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(providers::Store::new(
                app.path().app_local_data_dir()?.join("providers.json"),
            ));
            app.manage(source::Workspace::new()?);
            app.manage(source::pdf::ParserBridge::default());
            app.manage(ai_verification::ChatGptService::new(
                app.path().app_local_data_dir()?.join("chatgpt"),
            ));
            app.manage(agent::initialize(app.handle().clone()).map_err(std::io::Error::other)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            providers::list_providers,
            providers::save_provider,
            providers::delete_provider,
            source::import_sources,
            source::remove_source,
            source::pdf::pdf_file_size,
            source::pdf::pdf_read_range,
            source::pdf::pdf_connect_parser,
            source::pdf::pdf_resolve_parse,
            source::pdf::pdf_read_document,
            source::pdf::pdf_read_page,
            ai_verification::chatgpt::chatgpt_account_status,
            ai_verification::chatgpt::chatgpt_sign_in,
            ai_verification::chatgpt::chatgpt_cancel_sign_in,
            ai_verification::chatgpt::chatgpt_sign_out,
            agent::get_chatkit_connection,
            agent::list_chat_models,
            agent::initial::initialize_source,
            agent::initial::cancel_source_initialization,
            open_external_url
        ])
        .build(tauri::generate_context!())
        .expect("OpenRead could not start")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<source::Workspace>().cleanup();
            }
        });
}

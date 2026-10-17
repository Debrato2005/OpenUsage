#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::{WebviewWindowBuilder, ipc::InvokeBody, webview::NewWindowResponse};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum StorageState {
    SetupRequired,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Theme {
    System,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Monitoring {
    Stopped,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Provider {
    NotConfigured,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    storage: StorageState,
    theme: Theme,
    schema_version: Option<u32>,
    active_profiles: Option<u8>,
    monitoring: Monitoring,
    provider: Provider,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum CommandError {
    InvalidArguments,
}

#[tauri::command]
fn bootstrap(request: tauri::ipc::Request<'_>) -> Result<Bootstrap, CommandError> {
    match request.body() {
        InvokeBody::Json(body) if body.as_object().is_some_and(|args| args.is_empty()) => {}
        _ => return Err(CommandError::InvalidArguments),
    }

    Ok(Bootstrap {
        storage: StorageState::SetupRequired,
        theme: Theme::System,
        schema_version: None,
        active_profiles: None,
        monitoring: Monitoring::Stopped,
        provider: Provider::NotConfigured,
    })
}

fn is_bundled_url(url: &tauri::Url) -> bool {
    url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && ((url.scheme() == "tauri" && url.host_str() == Some("localhost"))
            || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost")))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![bootstrap])
        .setup(|app| {
            let window = &app.config().app.windows[0];
            WebviewWindowBuilder::from_config(app, window)?
                .on_navigation(is_bundled_url)
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Failed to start OpenUsage");
}

#[cfg(test)]
mod tests {
    use super::is_bundled_url;

    #[test]
    fn navigation_rejects_external_content_and_disguised_local_origins() {
        for url in [
            "https://example.com/",
            "http://127.0.0.1/",
            "http://tauri.localhost.example.com/",
            "http://tauri.localhost:1420/",
            "http://user@tauri.localhost/",
            "tauri://example.com/",
            "file:///tmp/index.html",
            "data:text/html,hello",
        ] {
            assert!(!is_bundled_url(&url.parse().unwrap()), "{url}");
        }
        for url in [
            "tauri://localhost/index.html",
            "tauri://localhost/index.html#main",
            "http://tauri.localhost/index.html",
        ] {
            assert!(is_bundled_url(&url.parse().unwrap()), "{url}");
        }
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod keyvault;
mod workspace;

use keyvault::OsVault;
use openusage_storage::Theme;
use serde::{Deserialize, Deserializer};
use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, State, WebviewWindowBuilder, ipc::InvokeBody, webview::NewWindowResponse,
};
use workspace::{Bootstrap, CommandError, Workspace};
use zeroize::Zeroizing;

#[derive(Default)]
struct WorkspaceService(Arc<Mutex<Option<Workspace>>>);

struct Passphrase(Zeroizing<String>);

impl<'de> Deserialize<'de> for Passphrase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|value| Self(Zeroizing::new(value)))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupArgs {
    #[serde(deserialize_with = "optional_passphrase")]
    passphrase: Option<Passphrase>,
}

fn optional_passphrase<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Passphrase>, D::Error> {
    Option::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnlockArgs {
    passphrase: Passphrase,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupArgs {
    password: Passphrase,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeArgs {
    theme: ThemeChoice,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ThemeChoice {
    System,
    Light,
    Dark,
}

enum Operation {
    Bootstrap,
    Setup(Option<Zeroizing<String>>),
    Unlock(Zeroizing<String>),
    SetTheme(Theme),
}

fn arguments<'a, T: Deserialize<'a>>(
    request: &'a tauri::ipc::Request<'_>,
) -> Result<T, CommandError> {
    match request.body() {
        InvokeBody::Json(body) => T::deserialize(body).map_err(|_| CommandError::InvalidArguments),
        _ => Err(CommandError::InvalidArguments),
    }
}

async fn run_workspace(
    app: AppHandle,
    service: Arc<Mutex<Option<Workspace>>>,
    operation: Operation,
) -> Result<Bootstrap, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::StorageUnavailable)?;
        if state.is_none() {
            let directory = app
                .path()
                .app_local_data_dir()
                .map_err(|_| CommandError::StorageUnavailable)?
                .join("workspace");
            match Workspace::new(directory) {
                Ok(workspace) => *state = Some(workspace),
                Err(error) if matches!(operation, Operation::Bootstrap) => {
                    return Ok(Bootstrap::blocked(error));
                }
                Err(error) => return Err(error),
            }
        }
        let workspace = state.as_mut().ok_or(CommandError::StorageUnavailable)?;
        match operation {
            Operation::Bootstrap => {
                let vault = OsVault::new();
                Ok(workspace.bootstrap(vault.as_ref().map_err(|error| *error)))
            }
            Operation::Setup(passphrase) => {
                let vault = OsVault::new();
                workspace.setup(vault.as_ref().map_err(|error| *error), passphrase)
            }
            Operation::Unlock(passphrase) => workspace.unlock(passphrase),
            Operation::SetTheme(theme) => workspace.set_theme(theme),
        }
    })
    .await
    .map_err(|_| CommandError::StorageUnavailable)?
}

#[tauri::command]
async fn bootstrap(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<Bootstrap, CommandError> {
    match request.body() {
        InvokeBody::Json(body) if body.as_object().is_some_and(|args| args.is_empty()) => {}
        _ => return Err(CommandError::InvalidArguments),
    }
    run_workspace(app, Arc::clone(&service.0), Operation::Bootstrap).await
}

#[tauri::command]
async fn setup_workspace(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<Bootstrap, CommandError> {
    let args: SetupArgs = arguments(&request)?;
    if let Some(passphrase) = &args.passphrase {
        workspace::validate_passphrase(&passphrase.0)?;
    }
    run_workspace(
        app,
        Arc::clone(&service.0),
        Operation::Setup(args.passphrase.map(|passphrase| passphrase.0)),
    )
    .await
}

#[tauri::command]
async fn unlock_workspace(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<Bootstrap, CommandError> {
    let args: UnlockArgs = arguments(&request)?;
    workspace::validate_unlock_passphrase(&args.passphrase.0)?;
    run_workspace(
        app,
        Arc::clone(&service.0),
        Operation::Unlock(args.passphrase.0),
    )
    .await
}

#[tauri::command]
async fn set_theme(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<Bootstrap, CommandError> {
    let args: ThemeArgs = arguments(&request)?;
    let theme = match args.theme {
        ThemeChoice::System => Theme::System,
        ThemeChoice::Light => Theme::Light,
        ThemeChoice::Dark => Theme::Dark,
    };
    run_workspace(app, Arc::clone(&service.0), Operation::SetTheme(theme)).await
}

#[tauri::command]
async fn create_backup(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<backup::BackupDestination, CommandError> {
    let args: BackupArgs = arguments(&request)?;
    workspace::validate_passphrase(&args.password.0)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::BackupUnavailable)?;
        let snapshot = state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .backup_snapshot()?;
        let directory = app
            .path()
            .download_dir()
            .map_err(|_| CommandError::BackupUnavailable)?
            .join("OpenUsage");
        backup::create(&directory, snapshot, args.password.0)
    })
    .await
    .map_err(|_| CommandError::BackupUnavailable)?
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
        .manage(WorkspaceService::default())
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            setup_workspace,
            unlock_workspace,
            set_theme,
            create_backup
        ])
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
    use super::{BackupArgs, SetupArgs, ThemeArgs, UnlockArgs, is_bundled_url};

    #[test]
    fn storage_commands_reject_unexpected_fields_and_incorrect_value_types() {
        for invalid in [
            r#"{}"#,
            r#"{"password":null}"#,
            r#"{"password":[]}"#,
            r#"{"password":"synthetic password","path":"/external"}"#,
        ] {
            assert!(serde_json::from_str::<BackupArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"passphrase":[],"path":"/external"}"#,
            r#"{"passphrase":"synthetic secret","path":"/external"}"#,
        ] {
            assert!(serde_json::from_str::<SetupArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{"passphrase":null}"#,
            r#"{"passphrase":42}"#,
            r#"{"passphrase":"synthetic secret","extra":true}"#,
        ] {
            assert!(serde_json::from_str::<UnlockArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{"theme":["dark"]}"#,
            r#"{"theme":"unknown"}"#,
            r#"{"theme":"dark","path":"/external"}"#,
        ] {
            assert!(serde_json::from_str::<ThemeArgs>(invalid).is_err());
        }
    }

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

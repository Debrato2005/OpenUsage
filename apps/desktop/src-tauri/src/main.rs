#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod backup_inspection;
mod keyvault;
mod provider;
mod restore;
mod workspace;

use keyvault::OsVault;
use openusage_storage::Theme;
use serde::{Deserialize, Deserializer};
use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, State, WebviewWindowBuilder, ipc::InvokeBody, webview::NewWindowResponse,
};
use workspace::{Bootstrap, CommandError, ProfileView, Workspace};
use zeroize::Zeroizing;

#[derive(Default)]
struct WorkspaceService(
    Arc<Mutex<Option<Workspace>>>,
    restore::RestoreCache,
    provider::ProviderCoordinator,
);

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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectBackupArgs {
    file_name: String,
    password: Passphrase,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelRestorePreviewArgs {
    preview_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyRestoreArgs {
    preview_id: String,
    confirmed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckpointPreviewArgs {
    checkpoint_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectProviderArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApproveProviderArgs {
    candidate_id: String,
    confirmed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileCheckArgs {
    profile_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum LoginFlowChoice {
    DeviceCode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BeginLoginArgs {
    profile_id: String,
    approval_id: String,
    flow: LoginFlowChoice,
    confirmed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoginAttemptArgs {
    profile_id: String,
    attempt_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeArgs {
    theme: ThemeChoice,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListProfilesArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateProfileArgs {
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameProfileArgs {
    id: String,
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveProfileArgs {
    id: String,
    archived: bool,
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

enum ProfileOperation {
    List,
    Create(String),
    Rename(String, String),
    SetArchived(String, bool),
}

fn arguments<'a, T: Deserialize<'a>>(
    request: &'a tauri::ipc::Request<'_>,
) -> Result<T, CommandError> {
    match request.body() {
        InvokeBody::Json(body) if body.is_object() => {
            T::deserialize(body).map_err(|_| CommandError::InvalidArguments)
        }
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
            match Workspace::with_services(
                directory,
                app.state::<WorkspaceService>().1.clone(),
                app.state::<WorkspaceService>().2.clone(),
            ) {
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

async fn run_profiles(
    service: Arc<Mutex<Option<Workspace>>>,
    operation: ProfileOperation,
) -> Result<ProfileView, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::ProfileUnavailable)?;
        let workspace = state.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        match operation {
            ProfileOperation::List => workspace.list_profiles(),
            ProfileOperation::Create(label) => workspace.create_profile(&label),
            ProfileOperation::Rename(id, label) => workspace.rename_profile(&id, &label),
            ProfileOperation::SetArchived(id, archived) => {
                workspace.set_profile_archived(&id, archived)
            }
        }
    })
    .await
    .map_err(|_| CommandError::ProfileUnavailable)?
}

#[tauri::command]
async fn list_profiles(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<ProfileView, CommandError> {
    let _: ListProfilesArgs = arguments(&request)?;
    run_profiles(Arc::clone(&service.0), ProfileOperation::List).await
}

#[tauri::command]
async fn create_profile(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<ProfileView, CommandError> {
    let args: CreateProfileArgs = arguments(&request)?;
    workspace::validate_profile_label(&args.label)?;
    run_profiles(Arc::clone(&service.0), ProfileOperation::Create(args.label)).await
}

#[tauri::command]
async fn rename_profile(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<ProfileView, CommandError> {
    let args: RenameProfileArgs = arguments(&request)?;
    workspace::validate_profile_id(&args.id)?;
    workspace::validate_profile_label(&args.label)?;
    run_profiles(
        Arc::clone(&service.0),
        ProfileOperation::Rename(args.id, args.label),
    )
    .await
}

#[tauri::command]
async fn set_profile_archived(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<ProfileView, CommandError> {
    let args: ArchiveProfileArgs = arguments(&request)?;
    workspace::validate_profile_id(&args.id)?;
    run_profiles(
        Arc::clone(&service.0),
        ProfileOperation::SetArchived(args.id, args.archived),
    )
    .await
}

#[tauri::command]
async fn list_backups(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<backup_inspection::BackupList, CommandError> {
    match request.body() {
        InvokeBody::Json(body) if body.as_object().is_some_and(|args| args.is_empty()) => {}
        _ => return Err(CommandError::InvalidArguments),
    }
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let state = service
            .lock()
            .map_err(|_| CommandError::BackupUnavailable)?;
        state
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .backup_inspection_directory()?;
        let directory = app
            .path()
            .download_dir()
            .map_err(|_| CommandError::BackupUnavailable)?
            .join("OpenUsage");
        backup_inspection::list(&directory)
    })
    .await
    .map_err(|_| CommandError::BackupUnavailable)?
}

#[tauri::command]
async fn inspect_backup(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<backup_inspection::BackupInspection, CommandError> {
    let args: InspectBackupArgs = arguments(&request)?;
    backup_inspection::validate_file_name(&args.file_name)?;
    workspace::validate_unlock_passphrase(&args.password.0)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let state = service
            .lock()
            .map_err(|_| CommandError::BackupUnavailable)?;
        let staging = state
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .backup_inspection_directory()?;
        let directory = app
            .path()
            .download_dir()
            .map_err(|_| CommandError::BackupUnavailable)?
            .join("OpenUsage");
        backup_inspection::inspect(&directory, &staging, &args.file_name, args.password.0)
    })
    .await
    .map_err(|_| CommandError::BackupUnavailable)?
}

#[tauri::command]
async fn prepare_restore_preview(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<restore::RestorePreview, CommandError> {
    let args: InspectBackupArgs = arguments(&request)?;
    backup_inspection::validate_file_name(&args.file_name)?;
    workspace::validate_unlock_passphrase(&args.password.0)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::BackupUnavailable)?;
        let workspace = state.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let directory = app
            .path()
            .download_dir()
            .map_err(|_| CommandError::BackupUnavailable)?
            .join("OpenUsage");
        workspace.prepare_restore_preview(&directory, &args.file_name, args.password.0)
    })
    .await
    .map_err(|_| CommandError::BackupUnavailable)?
}

#[tauri::command]
async fn cancel_restore_preview(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<restore::Cancelled, CommandError> {
    let args: CancelRestorePreviewArgs = arguments(&request)?;
    restore::validate_preview_id(&args.preview_id)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::BackupUnavailable)?;
        state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .cancel_restore_preview(&args.preview_id)
    })
    .await
    .map_err(|_| CommandError::BackupUnavailable)?
}

#[tauri::command]
async fn apply_restore(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<restore::Applied, CommandError> {
    let args: ApplyRestoreArgs = arguments(&request)?;
    restore::validate_preview_id(&args.preview_id)?;
    if !args.confirmed {
        return Err(CommandError::InvalidArguments);
    }
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service.lock().map_err(|_| CommandError::RestoreFailed)?;
        state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .apply_restore(&args.preview_id, args.confirmed)
    })
    .await
    .map_err(|_| CommandError::RestoreFailed)?
}

#[tauri::command]
async fn list_restore_checkpoints(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<restore::CheckpointList, CommandError> {
    let _: ListProfilesArgs = arguments(&request)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let state = service
            .lock()
            .map_err(|_| CommandError::RestoreCheckpointFailed)?;
        state
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .list_restore_checkpoints()
    })
    .await
    .map_err(|_| CommandError::RestoreCheckpointFailed)?
}

#[tauri::command]
async fn prepare_checkpoint_restore_preview(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<restore::CheckpointRestorePreview, CommandError> {
    let args: CheckpointPreviewArgs = arguments(&request)?;
    restore::validate_preview_id(&args.checkpoint_id)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::RestoreCheckpointFailed)?;
        state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .prepare_checkpoint_restore_preview(&args.checkpoint_id)
    })
    .await
    .map_err(|_| CommandError::RestoreCheckpointFailed)?
}

#[tauri::command]
async fn inspect_provider_executable(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::ExecutableCandidate, CommandError> {
    let args: InspectProviderArgs = arguments(&request)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let state = service
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        state
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .inspect_provider_executable(&args.path)
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn approve_provider_executable(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::ExecutableApproval, CommandError> {
    let args: ApproveProviderArgs = arguments(&request)?;
    crate::restore::validate_preview_id(&args.candidate_id)?;
    if !args.confirmed {
        return Err(CommandError::InvalidArguments);
    }
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let state = service
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        state
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .approve_provider_executable(&args.candidate_id, args.confirmed)
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn check_profile_connection(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::ConnectionDiagnostic, CommandError> {
    let args: ProfileCheckArgs = arguments(&request)?;
    workspace::validate_profile_id(&args.profile_id)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let pending = {
            let mut state = service
                .lock()
                .map_err(|_| CommandError::ProviderUnavailable)?;
            state
                .as_mut()
                .ok_or(CommandError::WorkspaceLocked)?
                .start_profile_check(&args.profile_id)?
        };
        let joined = pending.wait()?;
        let mut state = service
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .finish_profile_check(joined)
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn cancel_profile_check(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::Cancelled, CommandError> {
    let args: ProfileCheckArgs = arguments(&request)?;
    workspace::validate_profile_id(&args.profile_id)?;
    let provider = service.2.clone();
    // Cancellation needs no Workspace lock and can interrupt a check while another operation waits.
    tauri::async_runtime::spawn_blocking(move || provider.cancel(&args.profile_id))
        .await
        .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn discover_provider_executables(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::ExecutableDiscovery, CommandError> {
    let _: ListProfilesArgs = arguments(&request)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let profiles = {
            let state = service
                .lock()
                .map_err(|_| CommandError::ProviderDiscoveryUnavailable)?;
            state
                .as_ref()
                .ok_or(CommandError::WorkspaceLocked)?
                .provider_discovery_profiles()?
        };
        provider::discover(&profiles)
    })
    .await
    .map_err(|_| CommandError::ProviderDiscoveryUnavailable)?
}

#[tauri::command]
async fn begin_profile_login(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::LoginReceipt, CommandError> {
    let args: BeginLoginArgs = arguments(&request)?;
    workspace::validate_profile_id(&args.profile_id)?;
    restore::validate_preview_id(&args.approval_id)?;
    if !args.confirmed {
        return Err(CommandError::InvalidArguments);
    }
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = service
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        match args.flow {
            LoginFlowChoice::DeviceCode => state
                .as_mut()
                .ok_or(CommandError::WorkspaceLocked)?
                .begin_profile_login(&args.profile_id, &args.approval_id, args.confirmed),
        }
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn await_profile_login_prompt(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::LoginPromptReceipt, CommandError> {
    let args: LoginAttemptArgs = arguments(&request)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let pending = {
            let state = service
                .lock()
                .map_err(|_| CommandError::ProviderUnavailable)?;
            state
                .as_ref()
                .ok_or(CommandError::WorkspaceLocked)?
                .await_profile_login_prompt(&args.profile_id, &args.attempt_id)?
        };
        let ready = pending.wait()?;
        let mut state = service
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .finish_profile_login_prompt(ready)
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn complete_profile_login(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::LoginOutcome, CommandError> {
    let args: LoginAttemptArgs = arguments(&request)?;
    let service = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let pending = {
            let state = service
                .lock()
                .map_err(|_| CommandError::ProviderUnavailable)?;
            state
                .as_ref()
                .ok_or(CommandError::WorkspaceLocked)?
                .complete_profile_login(&args.profile_id, &args.attempt_id)?
        };
        let completed = pending.wait()?;
        let mut state = service
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        state
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .finish_profile_login(completed)
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
}

#[tauri::command]
async fn cancel_profile_login(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<provider::Cancelled, CommandError> {
    let args: LoginAttemptArgs = arguments(&request)?;
    let provider = service.2.clone();
    tauri::async_runtime::spawn_blocking(move || {
        provider.cancel_login(&args.profile_id, &args.attempt_id)
    })
    .await
    .map_err(|_| CommandError::ProviderUnavailable)?
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
            create_backup,
            list_profiles,
            create_profile,
            rename_profile,
            set_profile_archived,
            list_backups,
            inspect_backup,
            prepare_restore_preview,
            cancel_restore_preview,
            apply_restore,
            list_restore_checkpoints,
            prepare_checkpoint_restore_preview,
            inspect_provider_executable,
            approve_provider_executable,
            check_profile_connection,
            cancel_profile_check,
            begin_profile_login,
            await_profile_login_prompt,
            complete_profile_login,
            cancel_profile_login,
            discover_provider_executables
        ])
        .setup(|app| {
            let window = &app.config().app.windows[0];
            WebviewWindowBuilder::from_config(app, window)?
                .on_navigation(is_bundled_url)
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Failed to start OpenUsage")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<WorkspaceService>().1.close();
                if app.state::<WorkspaceService>().2.close().is_err() {
                    // Typed cleanup failure only. Never log provider frames, credentials, or paths.
                    eprintln!("OpenUsage could not complete owned provider cleanup.");
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyRestoreArgs, ApproveProviderArgs, ArchiveProfileArgs, BackupArgs, BeginLoginArgs,
        CancelRestorePreviewArgs, CheckpointPreviewArgs, CreateProfileArgs, InspectBackupArgs,
        InspectProviderArgs, ListProfilesArgs, LoginAttemptArgs, ProfileCheckArgs,
        RenameProfileArgs, SetupArgs, ThemeArgs, UnlockArgs, is_bundled_url,
    };

    #[test]
    fn storage_commands_reject_unexpected_fields_and_incorrect_value_types() {
        for invalid in [
            r#"{}"#,
            r#"{"profileId":"synthetic","flow":"device_code","confirmed":true}"#,
            r#"{"profileId":"synthetic","approvalId":"synthetic","flow":"api_key","confirmed":true}"#,
            r#"{"profileId":"synthetic","approvalId":"synthetic","flow":"device_code","confirmed":"true"}"#,
            r#"{"profileId":"synthetic","approvalId":"synthetic","flow":"device_code","confirmed":true,"url":"https://external.invalid"}"#,
        ] {
            assert!(serde_json::from_str::<BeginLoginArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"profileId":"synthetic"}"#,
            r#"{"profileId":"synthetic","attemptId":null}"#,
            r#"{"profileId":"synthetic","attemptId":"synthetic","providerLoginId":"injected"}"#,
        ] {
            assert!(serde_json::from_str::<LoginAttemptArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"path":null}"#,
            r#"{"path":"/synthetic","args":["exec"]}"#,
        ] {
            assert!(serde_json::from_str::<InspectProviderArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"candidateId":"synthetic"}"#,
            r#"{"candidateId":"synthetic","confirmed":"true"}"#,
            r#"{"candidateId":"synthetic","confirmed":true,"sha256":"caller-controlled"}"#,
        ] {
            assert!(serde_json::from_str::<ApproveProviderArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"profileId":null}"#,
            r#"{"profileId":"synthetic","home":"/external"}"#,
            r#"{"profileId":"synthetic","timeout":1}"#,
        ] {
            assert!(serde_json::from_str::<ProfileCheckArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"checkpointId":null}"#,
            r#"{"checkpointId":"synthetic","path":"/external"}"#,
            r#"{"checkpointId":"synthetic","password":"unnecessary"}"#,
        ] {
            assert!(serde_json::from_str::<CheckpointPreviewArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"previewId":"synthetic"}"#,
            r#"{"previewId":"synthetic","confirmed":"true"}"#,
            r#"{"previewId":"synthetic","confirmed":true,"path":"/external"}"#,
        ] {
            assert!(serde_json::from_str::<ApplyRestoreArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"previewId":null}"#,
            r#"{"previewId":[]}"#,
            r#"{"previewId":"synthetic","path":"/external"}"#,
            r#"{"previewId":"synthetic","apply":true}"#,
        ] {
            assert!(serde_json::from_str::<CancelRestorePreviewArgs>(invalid).is_err());
        }
        assert!(serde_json::from_str::<ListProfilesArgs>(r#"{"home":"/external"}"#).is_err());
        for invalid in [
            r#"{}"#,
            r#"{"label":null}"#,
            r#"{"label":[]}"#,
            r#"{"label":"Synthetic","id":"caller-selected"}"#,
            r#"{"label":"Synthetic","environment":"wsl"}"#,
            r#"{"label":"Synthetic","home":"/external"}"#,
        ] {
            assert!(serde_json::from_str::<CreateProfileArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{"label":"Synthetic"}"#,
            r#"{"id":[],"label":"Synthetic"}"#,
            r#"{"id":"synthetic","label":"Synthetic","extra":true}"#,
        ] {
            assert!(serde_json::from_str::<RenameProfileArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{"id":"synthetic"}"#,
            r#"{"id":"synthetic","archived":"true"}"#,
            r#"{"id":"synthetic","archived":1}"#,
            r#"{"id":"synthetic","archived":true,"deleteHistory":true}"#,
        ] {
            assert!(serde_json::from_str::<ArchiveProfileArgs>(invalid).is_err());
        }
        for invalid in [
            r#"{}"#,
            r#"{"fileName":null,"password":"synthetic"}"#,
            r#"{"fileName":"archive","password":null}"#,
            r#"{"fileName":"archive","password":[]}"#,
            r#"{"fileName":"archive","password":"synthetic","path":"/external"}"#,
        ] {
            assert!(serde_json::from_str::<InspectBackupArgs>(invalid).is_err());
        }
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

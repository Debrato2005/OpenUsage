use crate::{WorkspaceService, arguments, workspace::CommandError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    time_zone: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimeZone {
    time_zone: String,
}

async fn access(
    service: State<'_, WorkspaceService>,
    value: Option<String>,
) -> Result<TimeZone, CommandError> {
    let state = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = state.lock().map_err(|_| CommandError::StorageUnavailable)?;
        let workspace = guard.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        Ok(TimeZone {
            time_zone: workspace.time_zone(value.as_deref())?,
        })
    })
    .await
    .map_err(|_| CommandError::StorageUnavailable)?
}
#[tauri::command]
pub async fn load_time_zone(
    service: State<'_, WorkspaceService>,
) -> Result<TimeZone, CommandError> {
    access(service, None).await
}
#[tauri::command]
pub async fn set_time_zone(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<TimeZone, CommandError> {
    let args: Args = arguments(&request)?;
    if !openusage_storage::valid_time_zone(&args.time_zone) {
        return Err(CommandError::InvalidArguments);
    }
    access(service, Some(args.time_zone)).await
}

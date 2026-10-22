//! Strict subscription commands. The workspace lock covers each storage transaction.
use crate::workspace::subscriptions::{
    SavedSubscription, SubscriptionDeletion, SubscriptionInput, SubscriptionsView,
};
use crate::{
    WorkspaceService, arguments,
    workspace::{CommandError, Workspace},
};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListArgs {
    profile_id: String,
    after_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateArgs {
    profile_id: String,
    input: SubscriptionInput,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateArgs {
    profile_id: String,
    subscription_id: String,
    expected_revision: String,
    input: SubscriptionInput,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArchiveArgs {
    profile_id: String,
    subscription_id: String,
    expected_revision: String,
    archived: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteArgs {
    profile_id: String,
    subscription_id: String,
    expected_revision: String,
    confirmed: bool,
}

async fn access<T: Send + 'static>(
    service: State<'_, WorkspaceService>,
    operation: impl FnOnce(&mut Workspace) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    let workspace = Arc::clone(&service.0);
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = workspace
            .lock()
            .map_err(|_| CommandError::StorageUnavailable)?;
        operation(guard.as_mut().ok_or(CommandError::WorkspaceLocked)?)
    })
    .await
    .map_err(|_| CommandError::StorageUnavailable)?
}
#[tauri::command]
pub async fn list_subscriptions(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<SubscriptionsView, CommandError> {
    let args: ListArgs = arguments(&request)?;
    access(service, move |w| {
        w.list_subscriptions(&args.profile_id, args.after_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn create_subscription(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<SavedSubscription, CommandError> {
    let args: CreateArgs = arguments(&request)?;
    access(service, move |w| {
        w.create_subscription(&args.profile_id, args.input)
    })
    .await
}
#[tauri::command]
pub async fn update_subscription(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<SavedSubscription, CommandError> {
    let args: UpdateArgs = arguments(&request)?;
    access(service, move |w| {
        w.update_subscription(
            &args.profile_id,
            &args.subscription_id,
            &args.expected_revision,
            args.input,
        )
    })
    .await
}
#[tauri::command]
pub async fn set_subscription_archived(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<SavedSubscription, CommandError> {
    let args: ArchiveArgs = arguments(&request)?;
    access(service, move |w| {
        w.set_subscription_archived(
            &args.profile_id,
            &args.subscription_id,
            &args.expected_revision,
            args.archived,
        )
    })
    .await
}
#[tauri::command]
pub async fn delete_subscription(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<SubscriptionDeletion, CommandError> {
    let args: DeleteArgs = arguments(&request)?;
    if !args.confirmed {
        return Err(CommandError::InvalidArguments);
    }
    access(service, move |w| {
        w.delete_subscription(
            &args.profile_id,
            &args.subscription_id,
            &args.expected_revision,
        )
    })
    .await
}

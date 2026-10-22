use crate::{
    readings_export,
    workspace::{CommandError, Workspace},
};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationExportReceipt {
    file_name: String,
    directory: String,
    format: &'static str,
    schema_version: u32,
    table_count: u32,
    record_count: String,
    byte_count: String,
    exported_at: String,
}

pub fn create(
    directory: &Path,
    workspace: &mut Workspace,
    exported_at: String,
) -> Result<ApplicationExportReceipt, CommandError> {
    let (file, summary) = readings_export::publish(
        directory,
        "openusage-application-",
        "json",
        5 * 1024 * 1024 * 1024,
        |writer| workspace.write_application_export(writer, &exported_at),
    )?;
    Ok(ApplicationExportReceipt {
        file_name: file.file_name,
        directory: file.directory,
        format: "json",
        schema_version: summary.schema_version,
        table_count: summary.table_count,
        record_count: summary.record_count.to_string(),
        byte_count: file.byte_count.to_string(),
        exported_at,
    })
}

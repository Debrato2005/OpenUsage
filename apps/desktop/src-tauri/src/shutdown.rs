//! One shutdown path signals all owned workers before waiting outside the GUI thread.
use crate::WorkspaceService;
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::{AppHandle, ExitRequestApi, Manager};

const IDLE: u8 = 0;
const STOPPING: u8 = 1;
const READY: u8 = 2;

#[derive(Default)]
pub(crate) struct Shutdown(AtomicU8);

impl Shutdown {
    pub(crate) fn request(app: &AppHandle, api: &ExitRequestApi, code: Option<i32>) {
        let shutdown = app.state::<Self>();
        if shutdown.0.load(Ordering::Acquire) == READY {
            return;
        }
        api.prevent_exit();
        if shutdown
            .0
            .compare_exchange(IDLE, STOPPING, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let service = app.state::<WorkspaceService>();
            // Signal both loops before the coordinator signals and joins its owned children.
            // No workspace or GUI lock is held while any worker is joined.
            service.4.request_stop();
            service.3.request_stop();
            service.1.close();
            let provider = service.2.close();
            let scanner = service.4.close();
            let monitoring = service.3.close();
            let failed = provider.is_err() || scanner.is_err() || monitoring.is_err();
            if failed {
                // No provider frames, credentials, process arguments, or paths in diagnostics.
                eprintln!("OpenUsage could not complete owned worker cleanup.");
            }
            app.state::<Self>().0.store(READY, Ordering::Release);
            app.exit(if failed { 1 } else { code.unwrap_or(0) });
        });
    }
}

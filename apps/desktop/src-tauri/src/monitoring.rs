//! One owned timer waits outside the workspace lock and exits after provider cleanup.
use crate::{
    provider::ProviderCoordinator,
    workspace::{CommandError, Workspace},
};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(crate) struct MonitoringWorker {
    stop: Arc<(AtomicBool, Mutex<()>, Condvar)>,
    thread: Mutex<Option<JoinHandle<()>>>,
    provider: ProviderCoordinator,
}
impl MonitoringWorker {
    pub(crate) fn start(
        workspace: Arc<Mutex<Option<Workspace>>>,
        provider: ProviderCoordinator,
    ) -> Self {
        let stop = Arc::new((AtomicBool::new(false), Mutex::new(()), Condvar::new()));
        let signal = stop.clone();
        let thread = std::thread::spawn(move || {
            let notifications = crate::notifications::NotificationSink::default();
            let mut next_notification_check = Instant::now();
            while !signal.0.load(Ordering::Acquire) {
                if Instant::now() >= next_notification_check {
                    if let Ok(mut state) = workspace.lock()
                        && let Some(workspace) = state.as_mut()
                    {
                        workspace.dispatch_next_reset_notification(&notifications);
                    }
                    // This reads only the local outbox. Provider polling stays unchanged.
                    next_notification_check = Instant::now() + Duration::from_secs(10);
                }
                let job = workspace.lock().ok().and_then(|mut state| {
                    state
                        .as_mut()
                        .and_then(Workspace::next_scheduled_collection)
                });
                if let Some(job) = job {
                    job.run(&workspace);
                    continue;
                }
                let guard = signal.1.lock().unwrap_or_else(|p| p.into_inner());
                let _ = signal
                    .2
                    .wait_timeout_while(guard, Duration::from_millis(250), |_| {
                        !signal.0.load(Ordering::Acquire)
                    });
            }
        });
        Self {
            stop,
            thread: Mutex::new(Some(thread)),
            provider,
        }
    }
    pub(crate) fn close(&self) -> Result<(), CommandError> {
        self.stop.0.store(true, Ordering::Release);
        self.stop.2.notify_all();
        // Closing the coordinator joins owned provider workers before the timer can be joined.
        // No workspace lock is held while either kind of worker is stopped.
        let cleanup = self.provider.close();
        let joined = self
            .thread
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map_or(Ok(()), |thread| {
                thread
                    .join()
                    .map_err(|_| CommandError::ProviderCleanupFailed)
            });
        cleanup.and(joined)
    }
}
impl Drop for MonitoringWorker {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

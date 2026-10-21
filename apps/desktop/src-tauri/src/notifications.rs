//! Rust-only native notification submission. Acknowledgement never means the user saw it.
//!
//! Linux uses the official Desktop Notifications D-Bus protocol so its positive reply can
//! be recorded. Tauri's notification plugin returns before the OS send completes.
//! Windows/macOS use the same native component as that plugin, with one bounded worker.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const DEADLINE: Duration = Duration::from_secs(3);
const APPLICATION_ID: &str = "app.openusage.desktop";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeliveryOutcome {
    /// The native API acknowledged submission. Visibility is controlled by the OS.
    Accepted,
    /// No submission was made, for example because the service could not be reached.
    Unavailable,
    /// The platform explicitly refused submission.
    Rejected,
    /// Submission may have reached the OS. Never automatically repeat this event.
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NotificationInputError {
    Identifier,
    Label,
    Window,
    Evidence,
    Threshold,
}

/// Construct only from accepted local quota evidence, never a provider response directly.
/// No Debug implementation: profile names must not enter diagnostic logs by accident.
#[derive(Clone)]
pub(crate) struct ResetNotification {
    title: String,
    body: String,
}

impl ResetNotification {
    pub(crate) fn new(
        event_id: &str,
        profile_id: &str,
        profile_label: &str,
        window_duration_mins: u64,
        evidence_at_utc: &str,
        threshold_percent: Option<u8>,
    ) -> Result<Self, NotificationInputError> {
        for value in [event_id, profile_id] {
            if uuid::Uuid::parse_str(value)
                .ok()
                .is_none_or(|id| id.to_string() != value)
            {
                return Err(NotificationInputError::Identifier);
            }
        }
        crate::workspace::validate_profile_label(profile_label)
            .map_err(|_| NotificationInputError::Label)?;
        if profile_label
            .chars()
            .any(|c| matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        {
            return Err(NotificationInputError::Label);
        }
        if !(1..=9_007_199_254_740_991).contains(&window_duration_mins) {
            return Err(NotificationInputError::Window);
        }
        let bytes = evidence_at_utc.as_bytes();
        if bytes.len() != 24
            || !evidence_at_utc.is_ascii()
            || bytes[10] != b'T'
            || bytes[19] != b'.'
            || bytes[23] != b'Z'
            || !bytes[20..23].iter().all(u8::is_ascii_digit)
            || !evidence_at_utc[17..19]
                .parse::<u8>()
                .is_ok_and(|seconds| seconds < 60)
            || evidence_at_utc.parse::<jiff::Timestamp>().is_err()
        {
            return Err(NotificationInputError::Evidence);
        }
        // A custom profile name may itself be an email or credential-shaped value.
        // Use an opaque local alias rather than disclose it on the lock screen.
        let label = if profile_label.contains(['@', '/', '\\'])
            || profile_label.starts_with("sk-")
            || profile_label.starts_with("Bearer ")
        {
            format!("Account {}", &profile_id[..8])
        } else {
            profile_label.trim().to_owned()
        };
        let window = if window_duration_mins.is_multiple_of(1440) {
            format!("{}-day", window_duration_mins / 1440)
        } else if window_duration_mins.is_multiple_of(60) {
            format!("{}-hour", window_duration_mins / 60)
        } else {
            format!("{window_duration_mins}-minute")
        };
        let (title, detail) = match threshold_percent {
            None => (
                format!("{label}: reset confirmed"),
                format!("The {window} quota window reset."),
            ),
            Some(level @ (80 | 90 | 100)) => (
                format!("{label}: {level}% quota used"),
                format!("The {window} quota window reached {level}% used."),
            ),
            Some(_) => return Err(NotificationInputError::Threshold),
        };
        Ok(Self {
            title,
            body: format!("{detail}\nEvidence: {evidence_at_utc}\nOpen OpenUsage to view usage."),
        })
    }
}

#[derive(Default)]
struct Inner {
    busy: AtomicBool,
    #[cfg(target_os = "linux")]
    connection: std::sync::Mutex<Option<zbus::Connection>>,
}

/// Clones share the connection and one-in-flight limit. This performs no OS work at creation.
#[derive(Clone, Default)]
pub(crate) struct NotificationSink(Arc<Inner>);

struct SubmissionGuard(Arc<Inner>);
impl Drop for SubmissionGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::Release);
    }
}

impl NotificationSink {
    /// Call from the owned blocking alert worker, never the UI/event-loop thread.
    /// Windows/macOS keep the in-flight guard if a native call outlives the deadline.
    pub(crate) fn submit(&self, notification: &ResetNotification) -> DeliveryOutcome {
        if self
            .0
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return DeliveryOutcome::Unavailable;
        }
        let guard = SubmissionGuard(Arc::clone(&self.0));
        #[cfg(target_os = "linux")]
        {
            let _guard = guard;
            self.submit_linux(notification)
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            self.submit_native(notification.clone(), guard)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _guard = guard;
            let _ = notification;
            DeliveryOutcome::Unavailable
        }
    }

    #[cfg(target_os = "linux")]
    fn submit_linux(&self, notification: &ResetNotification) -> DeliveryOutcome {
        let mut connection = match self.0.connection.lock() {
            Ok(mut slot) => slot.take().filter(|connection| !connection.is_closed()),
            Err(_) => return DeliveryOutcome::Unavailable,
        };
        let mut request_started = false;
        let mut transport_failed = false;
        let outcome = tauri::async_runtime::block_on(async {
            let operation = async {
                if connection.is_none() {
                    connection = Some(
                        zbus::Connection::session()
                            .await
                            .map_err(|_| DeliveryOutcome::Unavailable)?,
                    );
                }
                let connection = connection.as_ref().ok_or(DeliveryOutcome::Unavailable)?;
                let hints = std::collections::HashMap::<&str, zbus::zvariant::Value<'_>>::from([
                    ("desktop-entry", zbus::zvariant::Value::from(APPLICATION_ID)),
                    ("urgency", zbus::zvariant::Value::from(1_u8)),
                ]);
                let body = notification
                    .body
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                let arguments = (
                    "OpenUsage",
                    0_u32,
                    APPLICATION_ID,
                    notification.title.as_str(),
                    body.as_str(),
                    Vec::<&str>::new(),
                    hints,
                    -1_i32,
                );
                request_started = true;
                let reply = connection
                    .call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "Notify",
                        &arguments,
                    )
                    .await
                    .map_err(|error| {
                        if matches!(
                            &error,
                            zbus::Error::InputOutput(_)
                                | zbus::Error::Connection(_, _)
                                | zbus::Error::Handshake(_)
                        ) {
                            transport_failed = true;
                        }
                        match error {
                            zbus::Error::MethodError(name, _, _)
                                if matches!(
                                    name.as_str(),
                                    "org.freedesktop.DBus.Error.ServiceUnknown"
                                        | "org.freedesktop.DBus.Error.NameHasNoOwner"
                                ) =>
                            {
                                DeliveryOutcome::Unavailable
                            }
                            zbus::Error::MethodError(name, _, _)
                                if name.as_str() == "org.freedesktop.DBus.Error.Disconnected" =>
                            {
                                transport_failed = true;
                                DeliveryOutcome::Uncertain
                            }
                            zbus::Error::MethodError(name, _, _)
                                if matches!(
                                    name.as_str(),
                                    "org.freedesktop.DBus.Error.NoReply"
                                        | "org.freedesktop.DBus.Error.Timeout"
                                        | "org.freedesktop.DBus.Error.TimedOut"
                                ) =>
                            {
                                DeliveryOutcome::Uncertain
                            }
                            zbus::Error::MethodError(_, _, _) => DeliveryOutcome::Rejected,
                            _ => DeliveryOutcome::Uncertain,
                        }
                    })?;
                let id = reply
                    .body()
                    .deserialize::<u32>()
                    .map_err(|_| DeliveryOutcome::Uncertain)?;
                if id == 0 {
                    Err(DeliveryOutcome::Uncertain)
                } else {
                    Ok(DeliveryOutcome::Accepted)
                }
            };
            match tokio::time::timeout(DEADLINE, operation).await {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(outcome)) => outcome,
                Err(_) if request_started => DeliveryOutcome::Uncertain,
                Err(_) => DeliveryOutcome::Unavailable,
            }
        });
        // Keep live connections: GNOME may dismiss notifications when their sender disconnects.
        // Failed transports are discarded only for the next distinct event, never a retry here.
        if let Ok(mut slot) = self.0.connection.lock() {
            *slot = connection.filter(|connection| !transport_failed && !connection.is_closed());
        }
        outcome
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn submit_native(
        &self,
        notification: ResetNotification,
        guard: SubmissionGuard,
    ) -> DeliveryOutcome {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("openusage-notification".into())
            .spawn(move || {
                let _guard = guard;
                let mut native = notify_rust::Notification::new();
                native.summary(&notification.title).body(&notification.body);
                #[cfg(target_os = "windows")]
                native.app_id(APPLICATION_ID);
                #[cfg(target_os = "macos")]
                if notify_rust::set_application(APPLICATION_ID).is_err() {
                    let _ = sender.send(DeliveryOutcome::Unavailable);
                    return;
                }
                let outcome = if native.show().is_ok() {
                    DeliveryOutcome::Accepted
                } else {
                    DeliveryOutcome::Rejected
                };
                let _ = sender.send(outcome);
            });
        if spawned.is_err() {
            return DeliveryOutcome::Unavailable;
        }
        match receiver.recv_timeout(DEADLINE) {
            Ok(outcome) => outcome,
            Err(_) => DeliveryOutcome::Uncertain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EVENT: &str = "11111111-1111-4111-8111-111111111111";
    const PROFILE: &str = "22222222-2222-4222-8222-222222222222";
    const TIME: &str = "2026-10-22T12:00:00.000Z";

    #[test]
    fn usage_messages_keep_exact_levels_and_refuse_unsupported_thresholds() {
        for level in [80, 90, 100] {
            let message =
                ResetNotification::new(EVENT, PROFILE, "Account A", 10080, TIME, Some(level))
                    .unwrap();
            assert_eq!(message.title, format!("Account A: {level}% quota used"));
            assert!(
                message
                    .body
                    .starts_with(&format!("The 7-day quota window reached {level}% used."))
            );
            assert!(!message.body.contains(EVENT));
        }
        for level in [0, 79, 85, 99, 101] {
            assert!(
                ResetNotification::new(EVENT, PROFILE, "Account A", 300, TIME, Some(level))
                    .is_err()
            );
        }
    }

    #[test]
    fn reset_message_uses_only_account_window_evidence_and_next_action() {
        let message = ResetNotification::new(EVENT, PROFILE, "Account A", 300, TIME, None).unwrap();
        assert_eq!(message.title, "Account A: reset confirmed");
        assert_eq!(
            message.body,
            "The 5-hour quota window reset.\nEvidence: 2026-10-22T12:00:00.000Z\nOpen OpenUsage to view usage."
        );
        assert!(!message.body.contains(EVENT));
        assert!(!message.body.contains(PROFILE));
        let weekly =
            ResetNotification::new(EVENT, PROFILE, "Account A", 10080, TIME, None).unwrap();
        assert!(weekly.body.starts_with("The 7-day quota window reset."));
        let email =
            ResetNotification::new(EVENT, PROFILE, "owner@example.test", 300, TIME, None).unwrap();
        assert_eq!(email.title, "Account 22222222: reset confirmed");
        assert!(!email.title.contains('@'));
        let path =
            ResetNotification::new(EVENT, PROFILE, "/private/workspace", 300, TIME, None).unwrap();
        assert_eq!(path.title, "Account 22222222: reset confirmed");
    }

    #[test]
    fn unsafe_or_noncanonical_inputs_are_refused_before_os_work() {
        assert!(ResetNotification::new("bad", PROFILE, "Account A", 300, TIME, None).is_err());
        assert!(ResetNotification::new(EVENT, "bad", "Account A", 300, TIME, None).is_err());
        for label in ["", "bad\nlabel", "hidden\u{202e}label"] {
            assert!(ResetNotification::new(EVENT, PROFILE, label, 300, TIME, None).is_err());
        }
        assert!(ResetNotification::new(EVENT, PROFILE, "Account A", 0, TIME, None).is_err());
        for time in [
            "2026-02-30T12:00:00.000Z",
            "2026-10-22T12:00:60.000Z",
            "2026-10-22T12:00:00+00:00",
            "2026-10-22T12:00:00.000Z\n",
        ] {
            assert!(ResetNotification::new(EVENT, PROFILE, "Account A", 300, time, None).is_err());
        }
    }
}

//! Narrow preferences IPC. Queuing and submission remain Rust-owned.
use super::*;
use openusage_storage::{
    AlertQuietHours, ResetAlertOverride, ResetAlertPreferences, ThresholdAlertPreferences,
};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NotificationPreferences {
    reset: ResetNotificationPreferences,
    usage: ThresholdNotificationPreferences,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ThresholdNotificationPreferences {
    enabled: bool,
    profile_overrides: Vec<ProfileOverride>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResetNotificationPreferences {
    enabled: bool,
    time_zone: String,
    quiet_enabled: bool,
    quiet_start: String,
    quiet_end: String,
    profile_overrides: Vec<ProfileOverride>,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileOverride {
    profile_id: String,
    enabled: bool,
}
fn minute(value: &str) -> Result<u16, CommandError> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || ![bytes[0], bytes[1], bytes[3], bytes[4]]
            .iter()
            .all(u8::is_ascii_digit)
    {
        return Err(CommandError::InvalidArguments);
    }
    let hour = u16::from(bytes[0] - b'0') * 10 + u16::from(bytes[1] - b'0');
    let minute = u16::from(bytes[3] - b'0') * 10 + u16::from(bytes[4] - b'0');
    if hour > 23 || minute > 59 {
        return Err(CommandError::InvalidArguments);
    }
    Ok(hour * 60 + minute)
}
impl TryFrom<ResetNotificationPreferences> for ResetAlertPreferences {
    type Error = CommandError;
    fn try_from(value: ResetNotificationPreferences) -> Result<Self, Self::Error> {
        if value.profile_overrides.len() > 100 || value.time_zone.len() > 128 {
            return Err(CommandError::InvalidArguments);
        }
        let start_minute = minute(&value.quiet_start)?;
        let end_minute = minute(&value.quiet_end)?;
        if start_minute == end_minute {
            return Err(CommandError::InvalidArguments);
        }
        Ok(Self {
            enabled: value.enabled,
            quiet_enabled: value.quiet_enabled,
            quiet_hours: AlertQuietHours {
                time_zone: value.time_zone,
                start_minute,
                end_minute,
            },
            profile_overrides: value
                .profile_overrides
                .into_iter()
                .map(|item| ResetAlertOverride {
                    profile_id: item.profile_id,
                    enabled: item.enabled,
                })
                .collect(),
        })
    }
}
impl From<ResetAlertPreferences> for ResetNotificationPreferences {
    fn from(value: ResetAlertPreferences) -> Self {
        Self {
            enabled: value.enabled,
            quiet_enabled: value.quiet_enabled,
            time_zone: value.quiet_hours.time_zone,
            quiet_start: format!(
                "{:02}:{:02}",
                value.quiet_hours.start_minute / 60,
                value.quiet_hours.start_minute % 60
            ),
            quiet_end: format!(
                "{:02}:{:02}",
                value.quiet_hours.end_minute / 60,
                value.quiet_hours.end_minute % 60
            ),
            profile_overrides: value
                .profile_overrides
                .into_iter()
                .map(|item| ProfileOverride {
                    profile_id: item.profile_id,
                    enabled: item.enabled,
                })
                .collect(),
        }
    }
}
impl Workspace {
    pub(crate) fn load_notification_preferences(
        &self,
    ) -> Result<NotificationPreferences, CommandError> {
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let usage = storage
            .load_threshold_alert_preferences()
            .map_err(|_| CommandError::StorageUnavailable)?;
        Ok(NotificationPreferences {
            reset: self.load_reset_notification_preferences()?,
            usage: ThresholdNotificationPreferences {
                enabled: usage.enabled,
                profile_overrides: usage
                    .profile_overrides
                    .into_iter()
                    .map(|item| ProfileOverride {
                        profile_id: item.profile_id,
                        enabled: item.enabled,
                    })
                    .collect(),
            },
        })
    }
    pub(crate) fn save_notification_preferences(
        &mut self,
        preferences: NotificationPreferences,
    ) -> Result<NotificationPreferences, CommandError> {
        if preferences.usage.profile_overrides.len() > 100 {
            return Err(CommandError::InvalidArguments);
        }
        let reset = preferences.reset.try_into()?;
        let usage = ThresholdAlertPreferences {
            enabled: preferences.usage.enabled,
            profile_overrides: preferences
                .usage
                .profile_overrides
                .into_iter()
                .map(|item| ResetAlertOverride {
                    profile_id: item.profile_id,
                    enabled: item.enabled,
                })
                .collect(),
        };
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .save_alert_preferences(&reset, &usage)
            .map_err(|error| match error {
                StorageError::InvalidAlertPreferences => CommandError::InvalidArguments,
                _ => CommandError::StorageUnavailable,
            })?;
        self.load_notification_preferences()
    }

    pub(crate) fn load_reset_notification_preferences(
        &self,
    ) -> Result<ResetNotificationPreferences, CommandError> {
        self.storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .load_reset_alert_preferences()
            .map(Into::into)
            .map_err(|_| CommandError::StorageUnavailable)
    }
    pub(crate) fn save_reset_notification_preferences(
        &mut self,
        preferences: ResetNotificationPreferences,
    ) -> Result<ResetNotificationPreferences, CommandError> {
        let preferences = preferences.try_into()?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        storage
            .save_reset_alert_preferences(&preferences)
            .map_err(|error| match error {
                StorageError::InvalidAlertPreferences => CommandError::InvalidArguments,
                _ => CommandError::StorageUnavailable,
            })?;
        self.load_reset_notification_preferences()
    }
}

impl Workspace {
    /// The caller holds the workspace lock across the bounded OS request. Restore
    /// and preference changes cannot overtake a claimed notification.
    pub(crate) fn dispatch_next_reset_notification(
        &mut self,
        sink: &crate::notifications::NotificationSink,
    ) {
        let Some(storage) = self.storage.as_mut() else {
            return;
        };
        let Some(now) = crate::restore::timestamp(std::time::SystemTime::now()) else {
            return;
        };
        let Ok(Some(event)) = storage.claim_usage_notification(&now) else {
            return;
        };
        let notification = crate::notifications::ResetNotification::new(
            &event.event_id,
            &event.profile_id,
            &event.profile_label,
            event.window_duration_mins,
            &event.evidence_at_utc,
            event.threshold_percent,
        );
        use crate::notifications::DeliveryOutcome;
        let outcome = match notification {
            Ok(notification) => sink.submit(&notification),
            Err(_) => DeliveryOutcome::Rejected,
        };
        let completion = match outcome {
            DeliveryOutcome::Accepted => openusage_storage::NotificationOutcome::Submitted,
            DeliveryOutcome::Unavailable | DeliveryOutcome::Rejected => {
                openusage_storage::NotificationOutcome::Failed
            }
            // A request can reach the OS before its acknowledgement is lost.
            // Its durable uncertain receipt prevents automatic replay.
            DeliveryOutcome::Uncertain => return,
        };
        let _ = storage.complete_reset_notification(&event.event_id, &event.attempt_id, completion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_times_are_exact_and_preferences_reject_extra_fields() {
        for value in ["0:00", "24:00", "23:60", "22:00:00", " 22:00", "２２:00"] {
            assert!(minute(value).is_err());
        }
        assert_eq!(minute("22:00"), Ok(1320));
        assert_eq!(minute("08:00"), Ok(480));
        let valid = serde_json::json!({"enabled":true,"timeZone":"Europe/London","quietEnabled":true,"quietStart":"22:00","quietEnd":"08:00","profileOverrides":[]});
        let parsed: ResetNotificationPreferences = serde_json::from_value(valid.clone()).unwrap();
        let domain: ResetAlertPreferences = parsed.clone().try_into().unwrap();
        assert_eq!(ResetNotificationPreferences::from(domain), parsed);
        let mut extra = valid.clone();
        extra["path"] = serde_json::json!("/external");
        assert!(serde_json::from_value::<ResetNotificationPreferences>(extra).is_err());
        let mut wrong = valid;
        wrong["enabled"] = serde_json::json!([true]);
        assert!(serde_json::from_value::<ResetNotificationPreferences>(wrong).is_err());
    }
}

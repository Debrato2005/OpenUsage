//! User-confirmed local terms, independent of provider authorization and readings.
use super::*;
use openusage_storage::{SubscriptionError, SubscriptionInterval, SubscriptionRecord};
use serde::Deserialize;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SubscriptionInput {
    plan: String,
    tier: Option<String>,
    currency: String,
    price: Option<String>,
    interval_unit: Option<String>,
    interval_count: Option<u16>,
    start_date: Option<String>,
    renewal_date: Option<String>,
    end_date: Option<String>,
    cancellation_date: Option<String>,
    auto_renew: Option<bool>,
}
impl SubscriptionInput {
    fn into_storage(self) -> Result<openusage_storage::SubscriptionInput, CommandError> {
        let interval_unit = self
            .interval_unit
            .as_deref()
            .map(|unit| match unit {
                "day" => Ok(SubscriptionInterval::Day),
                "week" => Ok(SubscriptionInterval::Week),
                "month" => Ok(SubscriptionInterval::Month),
                "year" => Ok(SubscriptionInterval::Year),
                "once" => Ok(SubscriptionInterval::Once),
                _ => Err(CommandError::InvalidSubscription),
            })
            .transpose()?;
        Ok(openusage_storage::SubscriptionInput {
            plan: self.plan,
            tier: self.tier,
            currency: self.currency,
            price: self.price,
            interval_unit,
            interval_count: self.interval_count,
            start_date: self.start_date,
            renewal_date: self.renewal_date,
            end_date: self.end_date,
            cancellation_date: self.cancellation_date,
            auto_renew: self.auto_renew,
        })
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedSubscription {
    #[serde(flatten)]
    input: SubscriptionInput,
    id: String,
    profile_id: String,
    provider: String,
    amount_minor: Option<String>,
    currency_scale: u8,
    archived: bool,
    revision: String,
    source: String,
    created_at_utc: String,
    confirmed_at_utc: String,
    updated_at_utc: String,
}
impl From<SubscriptionRecord> for SavedSubscription {
    fn from(r: SubscriptionRecord) -> Self {
        Self {
            input: SubscriptionInput {
                plan: r.plan,
                tier: r.tier,
                currency: r.currency,
                price: r.price,
                interval_unit: r.interval_unit.map(|unit| unit.as_str().into()),
                interval_count: r.interval_count,
                start_date: r.start_date,
                renewal_date: r.renewal_date,
                end_date: r.end_date,
                cancellation_date: r.cancellation_date,
                auto_renew: r.auto_renew,
            },
            id: r.id,
            profile_id: r.profile_id,
            provider: r.provider,
            amount_minor: r.amount_minor,
            currency_scale: r.currency_scale,
            archived: r.archived,
            revision: r.revision,
            source: r.source,
            created_at_utc: r.created_at_utc,
            confirmed_at_utc: r.confirmed_at_utc,
            updated_at_utc: r.updated_at_utc,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubscriptionsView {
    profile_id: String,
    records: Vec<SavedSubscription>,
    next_cursor: Option<String>,
    currencies: Vec<Currency>,
}
#[derive(Serialize)]
struct Currency {
    code: &'static str,
    scale: u8,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubscriptionDeletion {
    deleted: bool,
    subscription_id: String,
    profile_id: String,
}
fn error(value: SubscriptionError) -> CommandError {
    match value {
        SubscriptionError::InvalidInput => CommandError::InvalidSubscription,
        SubscriptionError::Changed => CommandError::SubscriptionChanged,
        _ => CommandError::SubscriptionUnavailable,
    }
}
fn now() -> Result<String, CommandError> {
    restore::timestamp(std::time::SystemTime::now()).ok_or(CommandError::SubscriptionUnavailable)
}
impl Workspace {
    pub fn list_subscriptions(
        &mut self,
        profile: &str,
        after: Option<&str>,
    ) -> Result<SubscriptionsView, CommandError> {
        let page = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .list_subscriptions(profile, after)
            .map_err(error)?;
        Ok(SubscriptionsView {
            profile_id: page.profile_id,
            records: page.records.into_iter().map(Into::into).collect(),
            next_cursor: page.next_cursor,
            currencies: openusage_storage::subscription_currencies()
                .iter()
                .map(|&(code, scale)| Currency { code, scale })
                .collect(),
        })
    }
    pub fn create_subscription(
        &mut self,
        profile: &str,
        input: SubscriptionInput,
    ) -> Result<SavedSubscription, CommandError> {
        let input = input.into_storage()?;
        let record = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .create_subscription(
                profile,
                &provider::new_id()?,
                &provider::new_id()?,
                input,
                &now()?,
            )
            .map_err(error)?;
        self.restore_preview.clear();
        Ok(record.into())
    }
    pub fn update_subscription(
        &mut self,
        profile: &str,
        id: &str,
        revision: &str,
        input: SubscriptionInput,
    ) -> Result<SavedSubscription, CommandError> {
        let input = input.into_storage()?;
        let record = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .update_subscription(profile, id, revision, &provider::new_id()?, input, &now()?)
            .map_err(error)?;
        self.restore_preview.clear();
        Ok(record.into())
    }
    pub fn set_subscription_archived(
        &mut self,
        profile: &str,
        id: &str,
        revision: &str,
        archived: bool,
    ) -> Result<SavedSubscription, CommandError> {
        let record = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .set_subscription_archived(
                profile,
                id,
                revision,
                &provider::new_id()?,
                archived,
                &now()?,
            )
            .map_err(error)?;
        self.restore_preview.clear();
        Ok(record.into())
    }
    pub fn delete_subscription(
        &mut self,
        profile: &str,
        id: &str,
        revision: &str,
    ) -> Result<SubscriptionDeletion, CommandError> {
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .delete_subscription(profile, id, revision)
            .map_err(error)?;
        self.restore_preview.clear();
        Ok(SubscriptionDeletion {
            deleted: true,
            subscription_id: id.into(),
            profile_id: profile.into(),
        })
    }
}

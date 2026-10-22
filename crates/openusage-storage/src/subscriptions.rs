//! Local subscription terms are independent of provider readings and payments.

use crate::{Storage, activity, backup_validation, currency_scales::CURRENCY_SCALES};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use std::fmt;

const PAGE_SIZE: usize = 50;
pub(crate) const COLUMNS: &str = "id,profile_id,provider,plan,tier,currency,currency_scale,amount_minor,interval_unit,interval_count,start_date,renewal_date,end_date,cancellation_date,auto_renew,archived,source,revision,created_at_utc,confirmed_at_utc,updated_at_utc";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionInterval {
    Day,
    Week,
    Month,
    Year,
    Once,
}
impl SubscriptionInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
            Self::Once => "once",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        match value {
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            "once" => Some(Self::Once),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionInput {
    pub plan: String,
    pub tier: Option<String>,
    pub currency: String,
    /// Major units, parsed without floating-point arithmetic or rounding.
    pub price: Option<String>,
    pub interval_unit: Option<SubscriptionInterval>,
    pub interval_count: Option<u16>,
    pub start_date: Option<String>,
    pub renewal_date: Option<String>,
    pub end_date: Option<String>,
    pub cancellation_date: Option<String>,
    pub auto_renew: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionRecord {
    pub id: String,
    pub profile_id: String,
    pub provider: String,
    pub plan: String,
    pub tier: Option<String>,
    pub currency: String,
    pub currency_scale: u8,
    pub amount_minor: Option<String>,
    /// Canonical major units for display and editing; null remains unknown.
    pub price: Option<String>,
    pub interval_unit: Option<SubscriptionInterval>,
    pub interval_count: Option<u16>,
    pub start_date: Option<String>,
    pub renewal_date: Option<String>,
    pub end_date: Option<String>,
    pub cancellation_date: Option<String>,
    pub auto_renew: Option<bool>,
    pub archived: bool,
    pub source: String,
    pub revision: String,
    pub created_at_utc: String,
    pub confirmed_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionPage {
    pub profile_id: String,
    pub records: Vec<SubscriptionRecord>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionError {
    InvalidInput,
    ProfileUnavailable,
    NotFound,
    Changed,
    ReadFailed,
    WriteFailed,
}
impl fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "The subscription details did not pass validation.",
            Self::ProfileUnavailable => "The selected account is unavailable.",
            Self::NotFound => "The subscription record is unavailable.",
            Self::Changed => "The subscription changed. Reload it before saving.",
            Self::ReadFailed => "The subscription records could not be read.",
            Self::WriteFailed => "The subscription change could not be saved.",
        })
    }
}
impl std::error::Error for SubscriptionError {}
type Result<T> = std::result::Result<T, SubscriptionError>;

pub fn subscription_currencies() -> &'static [(&'static str, u8)] {
    CURRENCY_SCALES
}

impl Storage {
    pub fn create_subscription(
        &mut self,
        profile_id: &str,
        id: &str,
        revision: &str,
        input: SubscriptionInput,
        now_utc: &str,
    ) -> Result<SubscriptionRecord> {
        ids(&[profile_id, id, revision])?;
        require(activity::activity_utc_timestamp(now_utc))?;
        let (scale, minor) = validate_input(&input)?;
        let minor = minor
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()
            .map_err(|_| SubscriptionError::InvalidInput)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(&tx, profile_id)?;
        tx.execute("INSERT INTO subscriptions(id,profile_id,provider,plan,tier,currency,currency_scale,amount_minor,interval_unit,interval_count,start_date,renewal_date,end_date,cancellation_date,auto_renew,archived,source,revision,created_at_utc,confirmed_at_utc,updated_at_utc) VALUES(?1,?2,'openai',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,0,'user_confirmed',?15,?16,?16,?16)", params![id,profile_id,input.plan,input.tier,input.currency,scale,minor,input.interval_unit.map(SubscriptionInterval::as_str),input.interval_count,input.start_date,input.renewal_date,input.end_date,input.cancellation_date,input.auto_renew,revision,now_utc]).map_err(write_error)?;
        let record = load(&tx, profile_id, id)?;
        tx.commit().map_err(write_error)?;
        Ok(record)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_subscription(
        &mut self,
        profile_id: &str,
        id: &str,
        expected_revision: &str,
        new_revision: &str,
        input: SubscriptionInput,
        now_utc: &str,
    ) -> Result<SubscriptionRecord> {
        ids(&[profile_id, id, expected_revision, new_revision])?;
        require(expected_revision != new_revision && activity::activity_utc_timestamp(now_utc))?;
        let (scale, minor) = validate_input(&input)?;
        let minor = minor
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()
            .map_err(|_| SubscriptionError::InvalidInput)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(&tx, profile_id)?;
        let current = current(&tx, profile_id, id, expected_revision)?;
        require(now_utc >= current.updated_at_utc.as_str())?;
        tx.execute("UPDATE subscriptions SET plan=?3,tier=?4,currency=?5,currency_scale=?6,amount_minor=?7,interval_unit=?8,interval_count=?9,start_date=?10,renewal_date=?11,end_date=?12,cancellation_date=?13,auto_renew=?14,revision=?15,confirmed_at_utc=?16,updated_at_utc=?16 WHERE profile_id=?1 AND id=?2",params![profile_id,id,input.plan,input.tier,input.currency,scale,minor,input.interval_unit.map(SubscriptionInterval::as_str),input.interval_count,input.start_date,input.renewal_date,input.end_date,input.cancellation_date,input.auto_renew,new_revision,now_utc]).map_err(write_error)?;
        let record = load(&tx, profile_id, id)?;
        tx.commit().map_err(write_error)?;
        Ok(record)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_subscription_archived(
        &mut self,
        profile_id: &str,
        id: &str,
        expected_revision: &str,
        new_revision: &str,
        archived: bool,
        now_utc: &str,
    ) -> Result<SubscriptionRecord> {
        ids(&[profile_id, id, expected_revision, new_revision])?;
        require(expected_revision != new_revision && activity::activity_utc_timestamp(now_utc))?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(&tx, profile_id)?;
        let existing = current(&tx, profile_id, id, expected_revision)?;
        require(now_utc >= existing.updated_at_utc.as_str())?;
        if existing.archived == archived {
            tx.commit().map_err(write_error)?;
            return Ok(existing);
        }
        tx.execute("UPDATE subscriptions SET archived=?3,revision=?4,updated_at_utc=?5 WHERE profile_id=?1 AND id=?2",params![profile_id,id,archived,new_revision,now_utc]).map_err(write_error)?;
        let record = load(&tx, profile_id, id)?;
        tx.commit().map_err(write_error)?;
        Ok(record)
    }

    pub fn delete_subscription(
        &mut self,
        profile_id: &str,
        id: &str,
        expected_revision: &str,
    ) -> Result<()> {
        ids(&[profile_id, id, expected_revision])?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(&tx, profile_id)?;
        current(&tx, profile_id, id, expected_revision)?;
        tx.execute(
            "DELETE FROM subscriptions WHERE profile_id=?1 AND id=?2",
            params![profile_id, id],
        )
        .map_err(write_error)?;
        tx.commit().map_err(write_error)
    }

    /// Active and archived records, newest first, from one deferred WAL snapshot.
    pub fn list_subscriptions(
        &mut self,
        profile_id: &str,
        after_id: Option<&str>,
    ) -> Result<SubscriptionPage> {
        ids(&[profile_id])?;
        if let Some(id) = after_id {
            ids(&[id])?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(read_error)?;
        require_profile(&tx, profile_id)?;
        let cursor = after_id.map(|id| load(&tx, profile_id, id)).transpose()?;
        let suffix = " ORDER BY created_at_utc DESC,id DESC LIMIT 51";
        let mut records = if let Some(cursor) = cursor {
            let mut statement=tx.prepare(&format!("SELECT {COLUMNS} FROM subscriptions WHERE profile_id=?1 AND (created_at_utc,id)<(?2,?3){suffix}")).map_err(read_error)?;
            statement
                .query_map(
                    params![profile_id, cursor.created_at_utc, cursor.id],
                    read_record,
                )
                .map_err(read_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(read_error)?
        } else {
            let mut statement = tx
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM subscriptions WHERE profile_id=?1{suffix}"
                ))
                .map_err(read_error)?;
            statement
                .query_map([profile_id], read_record)
                .map_err(read_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(read_error)?
        };
        let next_cursor = (records.len() > PAGE_SIZE).then(|| records[PAGE_SIZE - 1].id.clone());
        records.truncate(PAGE_SIZE);
        for record in &records {
            validate_record(record)?;
        }
        tx.commit().map_err(read_error)?;
        Ok(SubscriptionPage {
            profile_id: profile_id.into(),
            records,
            next_cursor,
        })
    }
}

fn current(
    connection: &Connection,
    profile_id: &str,
    id: &str,
    expected_revision: &str,
) -> Result<SubscriptionRecord> {
    let record = load(connection, profile_id, id)?;
    if record.revision != expected_revision {
        return Err(SubscriptionError::Changed);
    }
    Ok(record)
}
fn load(connection: &Connection, profile_id: &str, id: &str) -> Result<SubscriptionRecord> {
    let record = connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM subscriptions WHERE profile_id=?1 AND id=?2"),
            params![profile_id, id],
            read_record,
        )
        .optional()
        .map_err(read_error)?
        .ok_or(SubscriptionError::NotFound)?;
    validate_record(&record)?;
    Ok(record)
}
fn require_profile(connection: &Connection, profile_id: &str) -> Result<()> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 AND environment='native')",
            [profile_id],
            |r| r.get(0),
        )
        .map_err(read_error)?;
    if exists {
        Ok(())
    } else {
        Err(SubscriptionError::ProfileUnavailable)
    }
}
fn read_record(row: &Row<'_>) -> rusqlite::Result<SubscriptionRecord> {
    let minor = row.get::<_, Option<i64>>(7)?.map(|value| value.to_string());
    let scale: u8 = row.get(6)?;
    let price = minor
        .as_deref()
        .map(|value| format_minor(value, scale))
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let interval: Option<String> = row.get(8)?;
    let interval = interval
        .map(|value| SubscriptionInterval::parse(&value).ok_or(rusqlite::Error::InvalidQuery))
        .transpose()?;
    let auto_renew = row
        .get::<_, Option<i64>>(14)?
        .map(strict_bool)
        .transpose()?;
    let archived = strict_bool(row.get(15)?)?;
    Ok(SubscriptionRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        provider: row.get(2)?,
        plan: row.get(3)?,
        tier: row.get(4)?,
        currency: row.get(5)?,
        currency_scale: scale,
        amount_minor: minor,
        price,
        interval_unit: interval,
        interval_count: row.get(9)?,
        start_date: row.get(10)?,
        renewal_date: row.get(11)?,
        end_date: row.get(12)?,
        cancellation_date: row.get(13)?,
        auto_renew,
        archived,
        source: row.get(16)?,
        revision: row.get(17)?,
        created_at_utc: row.get(18)?,
        confirmed_at_utc: row.get(19)?,
        updated_at_utc: row.get(20)?,
    })
}
fn strict_bool(value: i64) -> rusqlite::Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn validate_input(input: &SubscriptionInput) -> Result<(u8, Option<String>)> {
    require(
        safe_text(&input.plan, 100)
            && input
                .tier
                .as_deref()
                .is_none_or(|value| safe_text(value, 100)),
    )?;
    let scale = currency_scale(&input.currency)?;
    match (input.interval_unit, input.interval_count) {
        (None | Some(SubscriptionInterval::Once), None) => {}
        (
            Some(
                SubscriptionInterval::Day
                | SubscriptionInterval::Week
                | SubscriptionInterval::Month
                | SubscriptionInterval::Year,
            ),
            Some(1..=120),
        ) => {}
        _ => return Err(SubscriptionError::InvalidInput),
    }
    for date in [
        &input.start_date,
        &input.renewal_date,
        &input.end_date,
        &input.cancellation_date,
    ]
    .into_iter()
    .flatten()
    {
        require(activity::activity_date(date))?;
    }
    if let Some(start) = &input.start_date {
        require(
            input.end_date.as_ref().is_none_or(|end| start <= end)
                && input
                    .renewal_date
                    .as_ref()
                    .is_none_or(|renewal| start <= renewal),
        )?;
    }
    let minor = input
        .price
        .as_deref()
        .map(|value| parse_major(value, scale))
        .transpose()?;
    Ok((scale, minor))
}
fn validate_record(record: &SubscriptionRecord) -> Result<()> {
    ids(&[&record.id, &record.profile_id, &record.revision])?;
    require(record.provider == "openai" && record.source == "user_confirmed")?;
    require(currency_scale(&record.currency)? == record.currency_scale)?;
    let (_, minor) = validate_input(&SubscriptionInput {
        plan: record.plan.clone(),
        tier: record.tier.clone(),
        currency: record.currency.clone(),
        price: record.price.clone(),
        interval_unit: record.interval_unit,
        interval_count: record.interval_count,
        start_date: record.start_date.clone(),
        renewal_date: record.renewal_date.clone(),
        end_date: record.end_date.clone(),
        cancellation_date: record.cancellation_date.clone(),
        auto_renew: record.auto_renew,
    })?;
    require(minor == record.amount_minor)?;
    require(
        [
            &record.created_at_utc,
            &record.confirmed_at_utc,
            &record.updated_at_utc,
        ]
        .into_iter()
        .all(|time| activity::activity_utc_timestamp(time))
            && record.created_at_utc <= record.confirmed_at_utc
            && record.confirmed_at_utc <= record.updated_at_utc,
    )
}
fn currency_scale(code: &str) -> Result<u8> {
    CURRENCY_SCALES
        .iter()
        .find_map(|(currency, scale)| (*currency == code).then_some(*scale))
        .ok_or(SubscriptionError::InvalidInput)
}
fn parse_major(value: &str, scale: u8) -> Result<String> {
    require(!value.is_empty() && value.len() <= 64 && scale <= 4)?;
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    require(
        !whole.is_empty()
            && whole.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.len() <= usize::from(scale)
            && (!value.contains('.') || (scale > 0 && !fraction.is_empty())),
    )?;
    let mut minor = 0_i64;
    for byte in whole
        .bytes()
        .chain(fraction.bytes())
        .chain(std::iter::repeat_n(
            b'0',
            usize::from(scale) - fraction.len(),
        ))
    {
        minor = minor
            .checked_mul(10)
            .and_then(|n| n.checked_add(i64::from(byte - b'0')))
            .ok_or(SubscriptionError::InvalidInput)?;
    }
    Ok(minor.to_string())
}
fn format_minor(value: &str, scale: u8) -> Result<String> {
    require(scale <= 4 && activity::activity_decimal(value))?;
    if scale == 0 {
        return Ok(value.into());
    }
    let mut digits = value.to_owned();
    while digits.len() <= usize::from(scale) {
        digits.insert(0, '0');
    }
    digits.insert(digits.len() - usize::from(scale), '.');
    Ok(digits)
}
fn safe_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && value.chars().count() <= max
        && !value.chars().any(char::is_control)
}
fn ids(values: &[&str]) -> Result<()> {
    require(
        values
            .iter()
            .all(|value| backup_validation::canonical_uuid(value)),
    )
}
fn require(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(SubscriptionError::InvalidInput)
    }
}
fn read_error(_: rusqlite::Error) -> SubscriptionError {
    SubscriptionError::ReadFailed
}
fn write_error(_: rusqlite::Error) -> SubscriptionError {
    SubscriptionError::WriteFailed
}

pub(crate) fn validate(
    connection: &Connection,
    schema: &str,
) -> std::result::Result<(), crate::BackupValidationError> {
    let invalid = || crate::BackupValidationError::InvalidDatabase;
    if !matches!(schema, "main" | "openusage_checkpoint") {
        return Err(invalid());
    }
    let mut statement = connection
        .prepare(&format!("SELECT {COLUMNS} FROM {schema}.subscriptions"))
        .map_err(|_| invalid())?;
    let mut rows = statement.query([]).map_err(|_| invalid())?;
    while let Some(row) = rows.next().map_err(|_| invalid())? {
        let record = read_record(row).map_err(|_| invalid())?;
        validate_record(&record).map_err(|_| invalid())?;
        let native:bool=connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {schema}.profiles WHERE id=?1 AND environment='native')"),[&record.profile_id],|r|r.get(0)).map_err(|_|invalid())?;
        if !native {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CURRENT_SCHEMA_VERSION, RestoreHomeMapping};
    const KEY: [u8; 32] = [0x61; 32];
    const NOW: &str = "2026-10-22T12:00:00.000Z";
    const LATER: &str = "2026-10-23T12:00:00.000Z";
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn input() -> SubscriptionInput {
        SubscriptionInput {
            plan: "ChatGPT Plus".into(),
            tier: Some("Personal".into()),
            currency: "USD".into(),
            price: Some("20".into()),
            interval_unit: Some(SubscriptionInterval::Month),
            interval_count: Some(1),
            start_date: Some("2026-10-01".into()),
            renewal_date: Some("2026-11-01".into()),
            end_date: None,
            cancellation_date: None,
            auto_renew: None,
        }
    }
    fn setup() -> (tempfile::TempDir, Storage) {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("subscriptions.db"), &KEY).unwrap();
        storage
            .create_profile(&id(1), "Account A", &id(11))
            .unwrap();
        storage
            .create_profile(&id(2), "Account B", &id(12))
            .unwrap();
        (dir, storage)
    }
    fn create(storage: &mut Storage, n: u32) -> SubscriptionRecord {
        storage
            .create_subscription(&id(1), &id(n), &id(n + 10_000), input(), NOW)
            .unwrap()
    }
    #[test]
    fn exact_money_handles_iso_scales_unknown_zero_and_checked_overflow() {
        for (code, major, minor, canonical) in [
            ("USD", "00020.1", "2010", "20.10"),
            ("JPY", "20", "20", "20"),
            ("BHD", "20.123", "20123", "20.123"),
            ("CLF", "20.1234", "201234", "20.1234"),
            (
                "USD",
                "92233720368547758.07",
                "9223372036854775807",
                "92233720368547758.07",
            ),
            ("USD", "0", "0", "0.00"),
        ] {
            let mut value = input();
            value.currency = code.into();
            value.price = Some(major.into());
            let (scale, result) = validate_input(&value).unwrap();
            assert_eq!(result.as_deref(), Some(minor));
            assert_eq!(format_minor(minor, scale).unwrap(), canonical);
        }
        let mut value = input();
        value.price = None;
        assert_eq!(validate_input(&value).unwrap().1, None);
        for price in [
            "-1",
            "+1",
            "1e2",
            "1,000",
            " 20",
            "20 ",
            ".5",
            "1.",
            "1.000",
            "92233720368547758.08",
            "١",
            "NaN",
        ] {
            value.price = Some(price.into());
            assert_eq!(
                validate_input(&value),
                Err(SubscriptionError::InvalidInput),
                "{price}"
            );
        }
        value.currency = "JPY".into();
        value.price = Some("1.0".into());
        assert!(validate_input(&value).is_err());
        value.currency = "XXX".into();
        value.price = None;
        assert!(validate_input(&value).is_err());
    }
    #[test]
    fn independent_terms_require_real_dates_without_inventing_relationships() {
        let mut value = input();
        value.cancellation_date = Some("2026-09-01".into());
        value.end_date = Some("2026-12-01".into());
        assert!(validate_input(&value).is_ok());
        value.renewal_date = Some("2026-02-30".into());
        assert!(validate_input(&value).is_err());
        value.renewal_date = Some("2026-09-30".into());
        assert!(validate_input(&value).is_err());
        value.renewal_date = None;
        value.interval_unit = None;
        value.interval_count = None;
        assert!(validate_input(&value).is_ok());
        value.interval_count = Some(1);
        assert!(validate_input(&value).is_err());
        value.interval_unit = Some(SubscriptionInterval::Once);
        assert!(validate_input(&value).is_err());
        value.interval_count = None;
        assert!(validate_input(&value).is_ok());
        value.plan = " Plus ".into();
        assert!(validate_input(&value).is_err());
    }
    #[test]
    fn crud_stale_edits_profile_isolation_archiving_and_restart_preserve_records() {
        let (dir, mut storage) = setup();
        let original = create(&mut storage, 100);
        assert_eq!(original.price.as_deref(), Some("20.00"));
        assert_eq!(original.amount_minor.as_deref(), Some("2000"));
        assert_eq!(original.source, "user_confirmed");
        assert_eq!(
            storage
                .connection
                .query_row(
                    "SELECT typeof(amount_minor) FROM subscriptions WHERE id=?1",
                    [&original.id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "integer"
        );
        assert_eq!(
            storage.list_subscriptions(&id(2), None).unwrap().records,
            vec![]
        );
        assert_eq!(
            storage.update_subscription(
                &id(2),
                &original.id,
                &original.revision,
                &id(20_000),
                input(),
                LATER
            ),
            Err(SubscriptionError::NotFound)
        );
        let mut revised = input();
        revised.price = Some("0".into());
        revised.auto_renew = Some(false);
        let edited = storage
            .update_subscription(
                &id(1),
                &original.id,
                &original.revision,
                &id(20_001),
                revised,
                LATER,
            )
            .unwrap();
        assert_eq!(edited.created_at_utc, NOW);
        assert_eq!(edited.confirmed_at_utc, LATER);
        assert_eq!(
            storage.delete_subscription(&id(1), &original.id, &original.revision),
            Err(SubscriptionError::Changed)
        );
        let archived = storage
            .set_subscription_archived(
                &id(1),
                &edited.id,
                &edited.revision,
                &id(20_002),
                true,
                LATER,
            )
            .unwrap();
        assert_eq!(archived.confirmed_at_utc, edited.confirmed_at_utc);
        assert_eq!(
            storage
                .set_subscription_archived(
                    &id(1),
                    &archived.id,
                    &archived.revision,
                    &id(20_003),
                    true,
                    LATER
                )
                .unwrap(),
            archived
        );
        storage.set_profile_archived(&id(1), true).unwrap();
        create(&mut storage, 101); // Archived native profiles retain editable local records.
        drop(storage);
        let mut storage = Storage::open(&dir.path().join("subscriptions.db"), &KEY).unwrap();
        let records = storage.list_subscriptions(&id(1), None).unwrap().records;
        assert_eq!(records.len(), 2);
        assert!(records.contains(&archived));
        storage
            .delete_subscription(&id(1), &archived.id, &archived.revision)
            .unwrap();
        assert_eq!(
            storage
                .list_subscriptions(&id(1), None)
                .unwrap()
                .records
                .len(),
            1
        );
    }
    #[test]
    fn pagination_includes_all_history_and_refuses_other_profile_cursor() {
        let (_dir, mut storage) = setup();
        for n in 100..=160 {
            create(&mut storage, n);
        }
        let first = storage.list_subscriptions(&id(1), None).unwrap();
        assert_eq!(first.records.len(), 50);
        let second = storage
            .list_subscriptions(&id(1), first.next_cursor.as_deref())
            .unwrap();
        assert_eq!(second.records.len(), 11);
        assert!(second.next_cursor.is_none());
        let mut ids = first
            .records
            .iter()
            .chain(&second.records)
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 61);
        assert_eq!(
            storage.list_subscriptions(&id(2), first.next_cursor.as_deref()),
            Err(SubscriptionError::NotFound)
        );
        assert_eq!(
            storage.list_subscriptions(&id(1), Some("not-a-cursor")),
            Err(SubscriptionError::InvalidInput)
        );
    }
    #[test]
    fn failed_writes_leave_terms_revision_and_restore_revision_unchanged() {
        let (_dir, mut storage) = setup();
        let original = create(&mut storage, 100);
        storage.connection.execute_batch("CREATE TEMP TRIGGER reject_subscription_update BEFORE UPDATE ON subscriptions BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        let revision = storage.restore_state().unwrap().revision;
        assert_eq!(
            storage.update_subscription(
                &id(1),
                &original.id,
                &original.revision,
                &id(30_000),
                input(),
                LATER
            ),
            Err(SubscriptionError::WriteFailed)
        );
        assert_eq!(
            storage.list_subscriptions(&id(1), None).unwrap().records,
            vec![original]
        );
        assert_eq!(storage.restore_state().unwrap().revision, revision);
    }
    #[test]
    fn backup_checkpoint_restore_and_export_keep_every_subscription_without_credentials() {
        let (dir, mut source) = setup();
        for n in 100..=155 {
            create(&mut source, n);
        }
        let expected = source
            .connection
            .prepare(&format!("SELECT {COLUMNS} FROM subscriptions ORDER BY id"))
            .unwrap()
            .query_map([], read_record)
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let snapshot = source.backup_snapshot().unwrap();
        crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
        let checkpoint = dir.path().join("checkpoint.db");
        source.create_encrypted_checkpoint(&checkpoint).unwrap();
        let mut reopened = Storage::open(&checkpoint, &KEY).unwrap();
        assert_eq!(
            reopened
                .list_subscriptions(&id(1), None)
                .unwrap()
                .records
                .len(),
            50
        );
        let (_target_dir, mut target) = setup();
        create(&mut target, 900);
        let revision = target.restore_state().unwrap().revision;
        target
            .replace_records_from_backup(
                &snapshot.bytes,
                snapshot.schema_version,
                &revision,
                &[
                    RestoreHomeMapping {
                        profile_id: id(1),
                        managed_home_reference: id(31),
                    },
                    RestoreHomeMapping {
                        profile_id: id(2),
                        managed_home_reference: id(32),
                    },
                ],
            )
            .unwrap();
        let restored = target
            .connection
            .prepare(&format!("SELECT {COLUMNS} FROM subscriptions ORDER BY id"))
            .unwrap()
            .query_map([], read_record)
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(restored, expected);
        let mut json = Vec::new();
        target.write_application_export(&mut json, LATER).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&json).unwrap();
        let table = document["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["name"] == "subscriptions")
            .unwrap();
        assert_eq!(table["rows"].as_array().unwrap().len(), 56);
        assert_eq!(document["schemaVersion"], CURRENT_SCHEMA_VERSION);
        assert_eq!(
            target
                .connection
                .query_row(
                    "SELECT approved_home_reference FROM profiles WHERE id=?1",
                    [id(1)],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            id(31)
        );
    }
    #[test]
    fn invalid_subscription_content_is_refused_by_backup_validation() {
        for bad in [
            "currency_scale=3",
            "amount_minor=-1",
            "auto_renew=2",
            "archived=2",
            "start_date='2026-02-30'",
            "source='provider'",
            "revision='invalid'",
            "interval_count=NULL",
        ] {
            let (_dir, mut storage) = setup();
            create(&mut storage, 100);
            storage.connection.execute_batch(&format!("PRAGMA ignore_check_constraints=ON; UPDATE subscriptions SET {bad}; PRAGMA ignore_check_constraints=OFF;")).unwrap();
            assert_eq!(
                crate::backup_validation::validate_connection(
                    &storage.connection,
                    CURRENT_SCHEMA_VERSION
                ),
                Err(crate::BackupValidationError::InvalidDatabase),
                "{bad}"
            );
        }
    }
    #[test]
    fn schema8_migrates_transactionally_and_old_backup_normalization_keeps_source() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::migrate(&mut connection, &crate::MIGRATIONS[..8]).unwrap();
        connection
            .execute_batch("UPDATE settings SET theme='dark';")
            .unwrap();
        connection
            .pragma_update(None, "journal_mode", "delete")
            .unwrap();
        let image = connection.serialize("main").unwrap().to_vec();
        crate::validate_backup_snapshot(&image, 8).unwrap();
        crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
            &image,
            8,
            false,
            |normalized, _| {
                assert_eq!(
                    normalized
                        .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                        .unwrap(),
                    CURRENT_SCHEMA_VERSION
                );
                assert_eq!(
                    normalized
                        .query_row("SELECT count(*) FROM subscriptions", [], |r| r
                            .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            8
        );
        let mut failing = crate::MIGRATIONS.to_vec();
        failing[8] = "CREATE TABLE subscriptions(id TEXT); SELECT missing_column FROM settings;";
        assert!(crate::migrate(&mut connection, &failing).is_err());
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE name='subscriptions'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            8
        );
    }
}

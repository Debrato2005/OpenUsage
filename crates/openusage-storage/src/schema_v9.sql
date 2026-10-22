-- Local user-confirmed subscription terms. These records never prove payment
-- or change the provider's quota, plan, authorization or remote subscription.
CREATE TABLE subscriptions (
 id TEXT PRIMARY KEY NOT NULL,
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 provider TEXT NOT NULL CHECK(provider='openai'),
 plan TEXT NOT NULL CHECK(length(plan) BETWEEN 1 AND 100),
 tier TEXT CHECK(tier IS NULL OR length(tier) BETWEEN 1 AND 100),
 currency TEXT NOT NULL CHECK(length(currency)=3),
 currency_scale INTEGER NOT NULL CHECK(currency_scale BETWEEN 0 AND 4),
 amount_minor INTEGER CHECK(amount_minor IS NULL OR amount_minor>=0),
 interval_unit TEXT CHECK(interval_unit IS NULL OR interval_unit IN ('day','week','month','year','once')),
 interval_count INTEGER,
 start_date TEXT CHECK(start_date IS NULL OR length(start_date)=10),
 renewal_date TEXT CHECK(renewal_date IS NULL OR length(renewal_date)=10),
 end_date TEXT CHECK(end_date IS NULL OR length(end_date)=10),
 cancellation_date TEXT CHECK(cancellation_date IS NULL OR length(cancellation_date)=10),
 auto_renew INTEGER CHECK(auto_renew IS NULL OR auto_renew IN (0,1)),
 archived INTEGER NOT NULL CHECK(archived IN (0,1)),
 source TEXT NOT NULL CHECK(source='user_confirmed'),
 revision TEXT NOT NULL,
 created_at_utc TEXT NOT NULL,
 confirmed_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 CHECK((interval_unit IS NULL AND interval_count IS NULL)
    OR (interval_unit IS NOT NULL AND interval_unit='once' AND interval_count IS NULL)
    OR (interval_unit IS NOT NULL AND interval_unit IN ('day','week','month','year') AND interval_count IS NOT NULL AND interval_count BETWEEN 1 AND 120)),
 CHECK(start_date IS NULL OR end_date IS NULL OR start_date<=end_date),
 CHECK(start_date IS NULL OR renewal_date IS NULL OR start_date<=renewal_date)
) STRICT;
CREATE INDEX subscriptions_profile_order ON subscriptions(profile_id,created_at_utc DESC,id DESC);
CREATE INDEX subscriptions_renewal_deadline ON subscriptions(renewal_date,profile_id) WHERE archived=0 AND renewal_date IS NOT NULL;
CREATE INDEX subscriptions_end_deadline ON subscriptions(end_date,profile_id) WHERE archived=0 AND end_date IS NOT NULL;
CREATE INDEX subscriptions_cancellation_deadline ON subscriptions(cancellation_date,profile_id) WHERE archived=0 AND cancellation_date IS NOT NULL;

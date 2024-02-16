use std::{
    fmt::Display,
    ops::{Add, Sub},
};

use chrono::prelude::*;
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{IsNull, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
    AsChangeset, AsExpression, FromSqlRow, Identifiable, Insertable, Queryable,
};
use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use crate::schema::*;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug, Clone, Serialize)]
#[typeshare]
pub struct Frame {
    id: i32,

    pub project: i32,

    pub start: Timestamp,
    pub end: Option<Timestamp>,
}

impl Frame {
    pub fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug, Clone, Serialize)]
pub struct Tag {
    id: i32,
    pub name: String,
    pub archived: bool,
    pub last_access_time: Timestamp,
}

impl Tag {
    pub fn id(&self) -> i32 {
        self.id
    }
}

#[derive(
    Queryable, Identifiable, Insertable, AsChangeset, Debug, Clone, Serialize, Deserialize,
)]
#[typeshare]
pub struct Project {
    id: i32,
    pub name: String,

    /// Whether this project can be selected in the UI or not.
    /// When a `Project` is archived, then it will not be visible in the TUI for starting/stopping
    /// frames.
    pub archived: bool,

    /// Last time this project was used in a `Frame` (start or end).
    /// Can be used for sorting projects in LRU fashion.
    pub last_access_time: Timestamp,
}

impl Project {
    pub fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Insertable, Debug)]
#[diesel(table_name = tags_per_project)]
pub struct TagProject {
    pub project_id: i32,
    pub tag_id: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = tags)]
pub struct NewTag<'a> {
    pub name: &'a str,
    pub last_access_time: &'a Timestamp,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = projects)]
pub struct NewProject<'a> {
    pub name: &'a str,
    pub last_access_time: &'a Timestamp,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = frames)]
pub struct NewFrame<'a> {
    pub project: i32,
    pub start: &'a Timestamp,
    pub end: Option<&'a Timestamp>,
}

#[derive(
    Debug,
    AsExpression,
    FromSqlRow,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Clone,
    Copy,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type=diesel::sql_types::Text)]
#[typeshare(serialized_as = "string")]
pub struct Timestamp(pub DateTime<FixedOffset>);

impl<DB> FromSql<Text, DB> for Timestamp
where
    DB: Backend,
    *const str: FromSql<Text, DB>,
{
    fn from_sql(bytes: <DB as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let text_ptr = <*const str as FromSql<Text, DB>>::from_sql(bytes)?;
        let text = unsafe { &*text_ptr };
        Ok(Timestamp(DateTime::parse_from_rfc3339(text)?))
    }
}

impl ToSql<Text, Sqlite> for Timestamp {
    fn to_sql(
        &self,
        out: &mut diesel::serialize::Output<'_, '_, Sqlite>,
    ) -> diesel::serialize::Result {
        let s = self.0.to_rfc3339();
        out.set_value(s);
        Ok(IsNull::No)
    }
}

impl Timestamp {
    /// Create a naive timestamp from the given year, month, day, hour, minute, second.
    ///
    /// # Panics
    ///
    /// This function panics if the given time is invalid, e.g. hour 28.
    /// ```should_panic
    /// # use ttt::model::Timestamp;
    /// let invalid = Timestamp::from_ymdhms(2022, 13, 39, 28, 70, 42);
    /// ```
    pub fn from_ymdhms(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> Self {
        Timestamp::from_naive(
            NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(h, min, s)
                .unwrap(),
        )
    }

    pub fn now() -> Self {
        let local_time = chrono::Local::now();
        let time = local_time.with_timezone(
            &chrono::FixedOffset::east_opt(local_time.offset().local_minus_utc())
                .expect("Time offset out of bounds"),
        );
        Self(time)
    }

    pub fn from_naive(time: NaiveDateTime) -> Self {
        let local_time = chrono::Local::now();
        let tz = chrono::FixedOffset::east_opt(local_time.offset().local_minus_utc())
            .expect("Time offset out of bounds");
        Timestamp(time.and_local_timezone(tz).earliest().expect("Time broke"))
    }

    pub fn to_local(self) -> DateTime<Local> {
        self.0.into()
    }

    pub fn to_naive(self) -> NaiveDateTime {
        self.0.naive_local()
    }

    /// Returns the elapsed time from this timestamp till now.
    pub fn elapsed(&self) -> chrono::Duration {
        Self::now().0 - self.0
    }

    /// Return a new timestamp at the same date, but at midnight (00:00:00).
    pub fn at_midnight(&self) -> Self {
        Self(
            self.0
                .with_hour(0)
                .and_then(|o| o.with_minute(0))
                .and_then(|o| o.with_second(0))
                .and_then(|o| o.with_nanosecond(0))
                .unwrap(),
        )
    }
}

macro_rules! ImplOpForTimestamp {
    ($trait:ident, $name:ident $type:ty => $function:ident) => {
        impl $trait<$type> for Timestamp {
            type Output = Option<Timestamp>;

            fn $name(self, rhs: $type) -> Self::Output {
                Some(Timestamp(self.0.$function(rhs)?))
            }
        }
    };
}

ImplOpForTimestamp!(Add, add chrono::Days => checked_add_days);
ImplOpForTimestamp!(Sub, sub chrono::Days => checked_sub_days);
ImplOpForTimestamp!(Add, add chrono::Months => checked_add_months);
ImplOpForTimestamp!(Sub, sub chrono::Months => checked_sub_months);

/// Models a span of time.
/// The span starts with the first [`Timestamp`] and ends just before the second,
/// that is, it is a half open range.
///
/// This type guarantees that `start() < end()`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TimeSpan(Timestamp, Timestamp);

impl TimeSpan {
    pub fn new(start: Timestamp, end: Timestamp) -> Result<Self, TimeSpanError> {
        if end <= start {
            return Err(TimeSpanError::EndBeforeStart(start, end));
        }

        Ok(Self(start, end))
    }

    pub fn start(&self) -> Timestamp {
        self.0
    }

    pub fn end(&self) -> Timestamp {
        self.1
    }

    /// Return a new timespan that starts with `self` and ends with `other`.
    ///
    /// For Example:
    /// ```
    /// # use ttt::model::{Timestamp, TimeSpan};
    /// let today_morning = Timestamp::from_ymdhms(2022, 01, 02, 0, 0, 0);
    /// let today_noon = Timestamp::from_ymdhms(2022, 01, 02, 12, 0, 0);
    /// let yesterday_morning = Timestamp::from_ymdhms(2022, 01, 01, 0, 0, 0);
    /// let yesterday_noon = Timestamp::from_ymdhms(2022, 01, 01, 12, 0, 0);
    ///
    /// let today = TimeSpan::new(today_morning, today_noon).unwrap();
    /// let yesterday = TimeSpan::new(yesterday_morning, yesterday_noon).unwrap();
    ///
    /// assert_eq!(
    ///     yesterday.extend(today).unwrap(),
    ///     TimeSpan::new(yesterday_morning, today_noon).unwrap()
    /// );
    /// ```
    /// # Errors
    /// Returns an error if other ends before self starts.
    #[allow(dead_code)]
    pub fn extend(&self, other: Self) -> Result<Self, TimeSpanError> {
        Self::new(self.start(), other.end())
    }
}

#[derive(Debug)]
pub enum TimeSpanError {
    EndBeforeStart(Timestamp, Timestamp),
}

impl std::error::Error for TimeSpanError {}

impl Display for TimeSpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TimeSpanError as T;
        match self {
            T::EndBeforeStart(s, e) => write!(f, "'{s:?}' is after '{e:?}' but should be before."),
        }
    }
}

use crate::schema::*;
use chrono::prelude::*;
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{IsNull, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
    AsChangeset, AsExpression, FromSqlRow, Identifiable, Insertable, Queryable, SqliteConnection,
};

#[derive(Queryable, Identifiable, AsChangeset)]
pub struct Frame {
    pub id: i32,

    pub project: i32,

    pub start: Timestamp,
    pub end: Option<Timestamp>,
}

#[derive(Queryable, Identifiable, AsChangeset)]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub archived: bool,
    pub last_access_time: Timestamp,
}

#[derive(Queryable, Identifiable, AsChangeset, Debug)]
pub struct Project {
    pub id: i32,
    pub name: String,

    /// Whether this project can be selected in the UI or not.
    /// When a `Project` is archived, then it will not be visible in the TUI for starting/stopping
    /// frames.
    pub archived: bool,

    /// Last time this project was used in a `Frame` (start or end).
    /// Can be used for sorting projects in LRU fashion.
    pub last_access_time: Timestamp,
}

/// Trait to model database objects that store their last access time,
/// for example for sorting.
pub trait HasAccessTime {
    /// Update the last access time of this object to the given time.
    /// This function updates the object itself and also the corresponding database entry.
    fn touch(
        &mut self,
        connection: &mut SqliteConnection,
        time: &Timestamp,
    ) -> Result<(), diesel::result::Error>;

    /// Update the last access time of this object to the current time.
    fn touch_now(
        &mut self,
        connection: &mut SqliteConnection,
    ) -> Result<(), diesel::result::Error> {
        let t = Timestamp::now();
        self.touch(connection, &t)
    }
}

impl HasAccessTime for Project {
    fn touch(
        &mut self,
        connection: &mut SqliteConnection,
        time: &Timestamp,
    ) -> Result<(), diesel::result::Error> {
        self.last_access_time = *time;

        use diesel::RunQueryDsl;
        diesel::update(&*self).set(&*self).execute(connection)?;

        Ok(())
    }
}

impl HasAccessTime for Tag {
    fn touch(
        &mut self,
        connection: &mut SqliteConnection,
        time: &Timestamp,
    ) -> Result<(), diesel::result::Error> {
        self.last_access_time = *time;

        use diesel::RunQueryDsl;
        diesel::update(&*self).set(&*self).execute(connection)?;

        Ok(())
    }
}

#[derive(Insertable)]
#[diesel(table_name = tags_per_project)]
pub struct TagProject {
    pub project_id: i32,
    pub tag_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = tags)]
pub struct NewTag<'a> {
    pub name: &'a str,
    pub last_access_time: &'a Timestamp,
}

#[derive(Insertable)]
#[diesel(table_name = projects)]
pub struct NewProject<'a> {
    pub name: &'a str,
    pub last_access_time: &'a Timestamp,
}

#[derive(Insertable)]
#[diesel(table_name = frames)]
pub struct NewFrame<'a> {
    pub project: i32,
    pub start: &'a Timestamp,
    pub end: Option<&'a Timestamp>,
}

#[derive(Debug, AsExpression, FromSqlRow, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[diesel(sql_type=diesel::sql_types::Text)]
pub struct Timestamp(pub DateTime<FixedOffset>);

impl<DB> FromSql<Text, DB> for Timestamp
where
    DB: Backend,
    *const str: FromSql<Text, DB>,
{
    fn from_sql(bytes: diesel::backend::RawValue<'_, DB>) -> diesel::deserialize::Result<Self> {
        let text_ptr = <*const str as FromSql<Text, DB>>::from_sql(bytes)?;
        let text = unsafe { &*text_ptr };
        Ok(Timestamp(DateTime::parse_from_rfc3339(text)?))
    }
}

impl ToSql<Text, Sqlite> for Timestamp {
    fn to_sql<'b>(
        &self,
        out: &mut diesel::serialize::Output<'b, '_, Sqlite>,
    ) -> diesel::serialize::Result {
        let s = self.0.to_rfc3339();
        out.set_value(s);
        Ok(IsNull::No)
    }
}

impl Timestamp {
    pub fn now() -> Self {
        let local_time = chrono::Local::now();
        let time = local_time.with_timezone(&chrono::FixedOffset::east(
            local_time.offset().local_minus_utc(),
        ));
        Self(time)
    }

    pub fn from_naive(time: NaiveDateTime) -> Self {
        let local_time = chrono::Local::now();
        let tz = chrono::FixedOffset::east(local_time.offset().local_minus_utc());
        Timestamp(chrono::DateTime::<chrono::FixedOffset>::from_local(
            time, tz,
        ))
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
}

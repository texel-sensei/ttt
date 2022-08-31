use std::{marker::PhantomData, time::SystemTime};

use diesel::{Queryable, Identifiable, AsChangeset, Insertable};
use crate::schema::*;

#[repr(transparent)]
struct ID<T>(i32, PhantomData<T>);

#[derive(Queryable, Identifiable, AsChangeset)]
pub struct Frame {
    pub id: i32,

    pub project: i32,

    pub start: SystemTime,
    pub end: Option<SystemTime>,

}

#[derive(Queryable, Identifiable, AsChangeset)]
pub struct Project {
    pub id: i32,
    pub name: String,

    /// Whether this project can be selected in the UI or not.
    /// When a `Project` is archived, then it will not be visible in the TUI for starting/stopping
    /// frames.
    pub archived: bool,

    /// Last time this project was used in a `Frame` (start or end).
    /// Can be used for sorting projects in LRU fashion.
    pub last_access_time: SystemTime,
}

#[derive(Insertable)]
#[diesel(table_name = projects)]
pub struct NewProject<'a> {
    pub name: &'a str,
}

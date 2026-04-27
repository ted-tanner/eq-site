use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::upcoming_events;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = upcoming_events)]
pub struct UpcomingEvent {
    pub id: String,
    pub name: String,
    pub event_date: String,
    pub event_time: Option<String>,
    pub end_date: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Insertable)]
#[diesel(table_name = upcoming_events)]
pub struct NewUpcomingEvent<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub event_date: &'a str,
    pub event_time: Option<&'a str>,
    pub end_date: Option<&'a str>,
    pub end_time: Option<&'a str>,
    pub location: Option<&'a str>,
    pub description: Option<&'a str>,
    pub created_at: i64,
    pub updated_at: i64,
}

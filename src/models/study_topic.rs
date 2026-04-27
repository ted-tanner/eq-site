use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::study_topics;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = study_topics)]
pub struct StudyTopic {
    pub id: String,
    pub week_start: String,
    pub name: String,
    pub description: Option<String>,
    pub hyperlink: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Insertable)]
#[diesel(table_name = study_topics)]
pub struct NewStudyTopic<'a> {
    pub id: &'a str,
    pub week_start: &'a str,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub hyperlink: Option<&'a str>,
    pub created_at: i64,
    pub updated_at: i64,
}

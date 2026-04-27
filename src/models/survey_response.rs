use diesel::{Insertable, Queryable, Selectable};
use serde::Serialize;

use crate::schema::survey_responses;

#[derive(Clone, Debug, Queryable, Selectable, Serialize)]
#[diesel(table_name = survey_responses)]
pub struct SurveyResponse {
    pub id: String,
    pub food_suggestions: Option<String>,
    pub dietary_restrictions: Option<String>,
    pub created_at: i64,
}

#[derive(Insertable)]
#[diesel(table_name = survey_responses)]
pub struct NewSurveyResponse<'a> {
    pub id: &'a str,
    pub food_suggestions: Option<&'a str>,
    pub dietary_restrictions: Option<&'a str>,
    pub created_at: i64,
}

use chrono::{Datelike, Local};

use crate::db::public::{CreateSurveyResponseInput, PublicDao};
use crate::db::{self, DbPool};
use crate::models::study_topic::StudyTopic;
use crate::models::survey_response::SurveyResponse;
use crate::models::upcoming_event::UpcomingEvent;
use crate::services::{ServiceError, block_dao};

#[derive(Clone)]
pub struct CreateSurveyResponseServiceInput {
    pub food_suggestions: Option<String>,
    pub dietary_restrictions: Option<String>,
}

pub struct PublicService {
    db_pool: DbPool,
}

impl PublicService {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub async fn landing(
        &self,
    ) -> Result<(Vec<UpcomingEvent>, Option<StudyTopic>, bool), ServiceError> {
        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
        let week_start = current_week_start();
        let pool = self.db_pool.clone();
        block_dao(move || PublicDao::new(&pool).landing(&today, &week_start)).await
    }

    pub async fn upcoming_study_topics(&self) -> Result<Vec<StudyTopic>, ServiceError> {
        let week_start = current_week_start();
        let pool = self.db_pool.clone();
        block_dao(move || PublicDao::new(&pool).upcoming_study_topics(&week_start)).await
    }

    pub async fn create_survey_response(
        &self,
        input: CreateSurveyResponseServiceInput,
    ) -> Result<SurveyResponse, ServiceError> {
        let input = CreateSurveyResponseInput {
            food_suggestions: truncated_optional(input.food_suggestions),
            dietary_restrictions: truncated_optional(input.dietary_restrictions),
        };
        let pool = self.db_pool.clone();
        block_dao(move || PublicDao::new(&pool).create_survey_response(input, db::now_ts())).await
    }
}

fn current_week_start() -> String {
    let today = Local::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    monday.format("%Y-%m-%d").to_string()
}

fn truncated_optional(value: Option<String>) -> Option<String> {
    value.map(|value| value.chars().take(512).collect())
}

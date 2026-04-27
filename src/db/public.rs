use diesel::prelude::*;
use diesel::{OptionalExtension, SelectableHelper};
use uuid::Uuid;

use crate::db::{DaoError, DbPool};
use crate::models::study_topic::StudyTopic;
use crate::models::survey_response::{NewSurveyResponse, SurveyResponse};
use crate::models::upcoming_event::UpcomingEvent;
use crate::schema::{study_topics, survey_responses, upcoming_events};

#[derive(Clone)]
pub struct CreateSurveyResponseInput {
    pub food_suggestions: Option<String>,
    pub dietary_restrictions: Option<String>,
}

pub struct PublicDao {
    db_pool: DbPool,
}

impl PublicDao {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub fn landing(
        &self,
        today: &str,
        week_start: &str,
    ) -> Result<(Vec<UpcomingEvent>, Option<StudyTopic>, bool), DaoError> {
        let mut conn = self.db_pool.get()?;
        let events = upcoming_events::table
            .filter(
                upcoming_events::event_date
                    .ge(today)
                    .or(upcoming_events::end_date.ge(today)),
            )
            .order(upcoming_events::event_date.asc())
            .then_order_by(upcoming_events::event_time.asc())
            .then_order_by(upcoming_events::created_at.asc())
            .select(UpcomingEvent::as_select())
            .load::<UpcomingEvent>(&mut conn)?;
        let current_study_topic = study_topics::table
            .filter(study_topics::week_start.eq(week_start))
            .select(StudyTopic::as_select())
            .first::<StudyTopic>(&mut conn)
            .optional()?;
        let has_upcoming_study_topics = study_topics::table
            .filter(study_topics::week_start.gt(week_start))
            .select(study_topics::id)
            .first::<String>(&mut conn)
            .optional()?
            .is_some();
        Ok((events, current_study_topic, has_upcoming_study_topics))
    }

    pub fn upcoming_study_topics(&self, week_start: &str) -> Result<Vec<StudyTopic>, DaoError> {
        let mut conn = self.db_pool.get()?;
        study_topics::table
            .filter(study_topics::week_start.gt(week_start))
            .order(study_topics::week_start.asc())
            .select(StudyTopic::as_select())
            .load::<StudyTopic>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn create_survey_response(
        &self,
        input: CreateSurveyResponseInput,
        now: i64,
    ) -> Result<SurveyResponse, DaoError> {
        let mut conn = self.db_pool.get()?;
        let id = Uuid::now_v7().to_string();
        diesel::insert_into(survey_responses::table)
            .values(NewSurveyResponse {
                id: &id,
                food_suggestions: input.food_suggestions.as_deref(),
                dietary_restrictions: input.dietary_restrictions.as_deref(),
                created_at: now,
            })
            .execute(&mut conn)?;
        survey_responses::table
            .find(id)
            .select(SurveyResponse::as_select())
            .first::<SurveyResponse>(&mut conn)
            .map_err(DaoError::from)
    }
}

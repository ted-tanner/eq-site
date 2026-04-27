use diesel::prelude::*;
use diesel::{OptionalExtension, SelectableHelper};
use uuid::Uuid;

use crate::db::{DaoError, DbPool};
use crate::models::post::{POST_APPROVED, POST_PENDING_APPROVAL, Post};
use crate::models::reply::Reply;
use crate::models::study_topic::{NewStudyTopic, StudyTopic};
use crate::models::survey_response::SurveyResponse;
use crate::models::upcoming_event::{NewUpcomingEvent, UpcomingEvent};
use crate::models::user::{ACCOUNT_ACTIVE, User};
use crate::schema::{
    posts, replies, study_topics, survey_responses, upcoming_events, user_notifications, users,
};

#[derive(Clone)]
pub struct EventInput {
    pub name: String,
    pub event_date: String,
    pub event_time: Option<String>,
    pub end_date: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone)]
pub struct StudyTopicInput {
    pub week_start: String,
    pub name: String,
    pub description: Option<String>,
    pub hyperlink: Option<String>,
}

pub struct AdminDao {
    db_pool: DbPool,
}

impl AdminDao {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub fn list_pending(&self) -> Result<Vec<User>, DaoError> {
        let mut conn = self.db_pool.get()?;
        let pending_users = users::table
            .filter(users::account_status.eq("pending_approval"))
            .select(User::as_select())
            .load::<User>(&mut conn)?;
        Ok(pending_users)
    }

    pub fn list_pending_anonymous_posts(&self) -> Result<Vec<Post>, DaoError> {
        let mut conn = self.db_pool.get()?;
        posts::table
            .filter(posts::is_anonymous.eq(true))
            .filter(posts::approval_status.eq(POST_PENDING_APPROVAL))
            .order(posts::created_at.asc())
            .select(Post::as_select())
            .load::<Post>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn approve_anonymous_post(&self, post_id: &str, now: i64) -> Result<Post, DaoError> {
        let mut conn = self.db_pool.get()?;
        let updated = diesel::update(
            posts::table
                .find(post_id)
                .filter(posts::is_anonymous.eq(true))
                .filter(posts::approval_status.eq(POST_PENDING_APPROVAL)),
        )
        .set((
            posts::approval_status.eq(POST_APPROVED),
            posts::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        if updated == 0 {
            return Err(DaoError::NotFound);
        }
        posts::table
            .find(post_id)
            .select(Post::as_select())
            .first::<Post>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn list_events(&self) -> Result<Vec<UpcomingEvent>, DaoError> {
        let mut conn = self.db_pool.get()?;
        upcoming_events::table
            .order(upcoming_events::event_date.asc())
            .then_order_by(upcoming_events::event_time.asc())
            .then_order_by(upcoming_events::created_at.asc())
            .select(UpcomingEvent::as_select())
            .load::<UpcomingEvent>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn create_event(&self, input: EventInput, now: i64) -> Result<UpcomingEvent, DaoError> {
        let mut conn = self.db_pool.get()?;
        let id = Uuid::now_v7().to_string();
        diesel::insert_into(upcoming_events::table)
            .values(NewUpcomingEvent {
                id: &id,
                name: &input.name,
                event_date: &input.event_date,
                event_time: input.event_time.as_deref(),
                end_date: input.end_date.as_deref(),
                end_time: input.end_time.as_deref(),
                location: input.location.as_deref(),
                description: input.description.as_deref(),
                created_at: now,
                updated_at: now,
            })
            .execute(&mut conn)?;
        upcoming_events::table
            .find(id)
            .select(UpcomingEvent::as_select())
            .first::<UpcomingEvent>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn update_event(
        &self,
        event_id: &str,
        input: EventInput,
        now: i64,
    ) -> Result<UpcomingEvent, DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::update(upcoming_events::table.find(event_id))
            .set((
                upcoming_events::name.eq(input.name),
                upcoming_events::event_date.eq(input.event_date),
                upcoming_events::event_time.eq(input.event_time),
                upcoming_events::end_date.eq(input.end_date),
                upcoming_events::end_time.eq(input.end_time),
                upcoming_events::location.eq(input.location),
                upcoming_events::description.eq(input.description),
                upcoming_events::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        upcoming_events::table
            .find(event_id)
            .select(UpcomingEvent::as_select())
            .first::<UpcomingEvent>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn delete_event(&self, event_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::delete(upcoming_events::table.find(event_id)).execute(&mut conn)?;
        Ok(())
    }

    pub fn list_study_topics(&self) -> Result<Vec<StudyTopic>, DaoError> {
        let mut conn = self.db_pool.get()?;
        study_topics::table
            .order(study_topics::week_start.asc())
            .select(StudyTopic::as_select())
            .load::<StudyTopic>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn list_survey_responses(&self, limit: i64) -> Result<Vec<SurveyResponse>, DaoError> {
        let mut conn = self.db_pool.get()?;
        survey_responses::table
            .order(survey_responses::created_at.desc())
            .limit(limit)
            .select(SurveyResponse::as_select())
            .load::<SurveyResponse>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn create_study_topic(
        &self,
        input: StudyTopicInput,
        now: i64,
    ) -> Result<StudyTopic, DaoError> {
        let mut conn = self.db_pool.get()?;
        let existing = study_topics::table
            .filter(study_topics::week_start.eq(&input.week_start))
            .select(study_topics::id)
            .first::<String>(&mut conn)
            .optional()?;
        if existing.is_some() {
            return Err(DaoError::Requirement(
                "A study topic already exists for that week".to_string(),
            ));
        }
        let id = Uuid::now_v7().to_string();
        diesel::insert_into(study_topics::table)
            .values(NewStudyTopic {
                id: &id,
                week_start: &input.week_start,
                name: &input.name,
                description: input.description.as_deref(),
                hyperlink: input.hyperlink.as_deref(),
                created_at: now,
                updated_at: now,
            })
            .execute(&mut conn)?;
        study_topics::table
            .find(id)
            .select(StudyTopic::as_select())
            .first::<StudyTopic>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn update_study_topic(
        &self,
        topic_id: &str,
        input: StudyTopicInput,
        now: i64,
    ) -> Result<StudyTopic, DaoError> {
        let mut conn = self.db_pool.get()?;
        let existing = study_topics::table
            .filter(study_topics::week_start.eq(&input.week_start))
            .filter(study_topics::id.ne(topic_id))
            .select(study_topics::id)
            .first::<String>(&mut conn)
            .optional()?;
        if existing.is_some() {
            return Err(DaoError::Requirement(
                "A study topic already exists for that week".to_string(),
            ));
        }
        diesel::update(study_topics::table.find(topic_id))
            .set((
                study_topics::week_start.eq(input.week_start),
                study_topics::name.eq(input.name),
                study_topics::description.eq(input.description),
                study_topics::hyperlink.eq(input.hyperlink),
                study_topics::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        study_topics::table
            .find(topic_id)
            .select(StudyTopic::as_select())
            .first::<StudyTopic>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn delete_study_topic(&self, topic_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::delete(study_topics::table.find(topic_id)).execute(&mut conn)?;
        Ok(())
    }

    pub fn list_users(&self, page: i64, page_size: i64) -> Result<Vec<User>, DaoError> {
        let mut conn = self.db_pool.get()?;
        users::table
            .order(users::created_at.desc())
            .limit(page_size)
            .offset((page - 1) * page_size)
            .select(User::as_select())
            .load::<User>(&mut conn)
            .map_err(DaoError::from)
    }

    pub fn approve_user(
        &self,
        user_id: &str,
        actor_user_id: &str,
        now: i64,
    ) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::update(users::table.find(user_id))
            .set((
                users::account_status.eq(ACCOUNT_ACTIVE),
                users::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        diesel::insert_into(user_notifications::table)
            .values((
                user_notifications::id.eq(Uuid::now_v7().to_string()),
                user_notifications::user_id.eq(user_id.to_string()),
                user_notifications::kind.eq("user_approved"),
                user_notifications::post_id.eq::<Option<String>>(None),
                user_notifications::reply_id.eq::<Option<String>>(None),
                user_notifications::actor_user_id.eq(Some(actor_user_id.to_string())),
                user_notifications::message
                    .eq("Your account has been approved by a member of the EQ presidency"),
                user_notifications::created_at.eq(now),
                user_notifications::read_at.eq::<Option<i64>>(None),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_admin(&self, target_id: &str, is_admin: bool) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        if !is_admin {
            let admin_count: i64 = users::table
                .filter(users::is_admin.eq(true))
                .count()
                .get_result(&mut conn)?;
            let target = users::table
                .find(target_id)
                .select(User::as_select())
                .first::<User>(&mut conn)?;
            if target.is_admin && admin_count <= 1 {
                return Err(DaoError::Requirement(
                    "There must always be at least one admin".to_string(),
                ));
            }
        }
        diesel::update(users::table.find(target_id))
            .set(users::is_admin.eq(is_admin))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_user_status(&self, user_id: &str, status: &str, now: i64) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        diesel::update(users::table.find(user_id))
            .set((users::account_status.eq(status), users::updated_at.eq(now)))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_user(&self, target_id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        let target = users::table
            .find(target_id)
            .select(User::as_select())
            .first::<User>(&mut conn)?;
        if target.is_admin {
            let admin_count: i64 = users::table
                .filter(users::is_admin.eq(true))
                .count()
                .get_result(&mut conn)?;
            if admin_count <= 1 {
                return Err(DaoError::Requirement(
                    "There must always be at least one admin".to_string(),
                ));
            }
        }
        diesel::delete(users::table.find(target_id)).execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_content(&self, kind: &str, id: &str) -> Result<(), DaoError> {
        let mut conn = self.db_pool.get()?;
        match kind {
            "post" => {
                let _ = posts::table
                    .find(id)
                    .select(Post::as_select())
                    .first::<Post>(&mut conn)
                    .optional()?;
                diesel::delete(posts::table.find(id)).execute(&mut conn)?;
            }
            "reply" => {
                let _ = replies::table
                    .find(id)
                    .select(Reply::as_select())
                    .first::<Reply>(&mut conn)
                    .optional()?;
                diesel::delete(replies::table.find(id)).execute(&mut conn)?;
            }
            _ => return Err(DaoError::InvalidInput("Unknown content kind".to_string())),
        }
        Ok(())
    }
}

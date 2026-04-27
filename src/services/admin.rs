use chrono::{Datelike, NaiveDate, NaiveTime, Weekday};

use crate::db::admin::{AdminDao, EventInput, StudyTopicInput};
use crate::db::{self, DbPool};
use crate::models::post::Post;
use crate::models::study_topic::StudyTopic;
use crate::models::survey_response::SurveyResponse;
use crate::models::upcoming_event::UpcomingEvent;
use crate::models::user::User;
use crate::services::auth::AuthService;
use crate::services::{ServiceError, block_dao};

#[derive(Clone)]
pub struct UpsertEventInput {
    pub name: String,
    pub event_date: String,
    pub event_time: Option<String>,
    pub end_date: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone)]
pub struct UpsertStudyTopicInput {
    pub week_start: String,
    pub name: String,
    pub description: Option<String>,
    pub hyperlink: Option<String>,
}

pub struct AdminService {
    db_pool: DbPool,
}

impl AdminService {
    pub fn new(db_pool: &DbPool) -> Self {
        Self {
            db_pool: db_pool.clone(),
        }
    }

    pub async fn list_pending(&self, admin_id: String) -> Result<Vec<User>, ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).list_pending()).await
    }

    pub async fn list_pending_anonymous_posts(
        &self,
        admin_id: String,
    ) -> Result<Vec<Post>, ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).list_pending_anonymous_posts()).await
    }

    pub async fn approve_anonymous_post(
        &self,
        admin_id: String,
        post_id: String,
    ) -> Result<Post, ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).approve_anonymous_post(&post_id, db::now_ts())).await
    }

    pub async fn list_events(&self, admin_id: String) -> Result<Vec<UpcomingEvent>, ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).list_events()).await
    }

    pub async fn create_event(
        &self,
        admin_id: String,
        input: UpsertEventInput,
    ) -> Result<UpcomingEvent, ServiceError> {
        self.require_admin(admin_id).await?;
        let input = validate_event(input)?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).create_event(input, db::now_ts())).await
    }

    pub async fn update_event(
        &self,
        admin_id: String,
        event_id: String,
        input: UpsertEventInput,
    ) -> Result<UpcomingEvent, ServiceError> {
        self.require_admin(admin_id).await?;
        let input = validate_event(input)?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).update_event(&event_id, input, db::now_ts())).await
    }

    pub async fn delete_event(
        &self,
        admin_id: String,
        event_id: String,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).delete_event(&event_id)).await
    }

    pub async fn list_study_topics(
        &self,
        admin_id: String,
    ) -> Result<Vec<StudyTopic>, ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).list_study_topics()).await
    }

    pub async fn list_survey_responses(
        &self,
        admin_id: String,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<SurveyResponse>, i64, i64, bool), ServiceError> {
        self.require_admin(admin_id).await?;
        let page_size = page_size.clamp(1, 100);
        let page = page.max(1);
        let pool = self.db_pool.clone();
        let mut rows = block_dao(move || {
            AdminDao::new(&pool).list_survey_responses(page, page_size, page_size + 1)
        })
        .await?;
        let has_more = rows.len() > page_size as usize;
        rows.truncate(page_size as usize);
        Ok((rows, page, page_size, has_more))
    }

    pub async fn create_study_topic(
        &self,
        admin_id: String,
        input: UpsertStudyTopicInput,
    ) -> Result<StudyTopic, ServiceError> {
        self.require_admin(admin_id).await?;
        let input = validate_study_topic(input)?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).create_study_topic(input, db::now_ts()))
            .await
            .map_study_topic_conflict()
    }

    pub async fn update_study_topic(
        &self,
        admin_id: String,
        topic_id: String,
        input: UpsertStudyTopicInput,
    ) -> Result<StudyTopic, ServiceError> {
        self.require_admin(admin_id).await?;
        let input = validate_study_topic(input)?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).update_study_topic(&topic_id, input, db::now_ts()))
            .await
            .map_study_topic_conflict()
    }

    pub async fn delete_study_topic(
        &self,
        admin_id: String,
        topic_id: String,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).delete_study_topic(&topic_id)).await
    }

    pub async fn list_users(
        &self,
        admin_id: String,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<User>, i64, i64), ServiceError> {
        self.require_admin(admin_id).await?;
        let page_size = page_size.clamp(1, 100);
        let page = page.max(1);
        let pool = self.db_pool.clone();
        let users = block_dao(move || AdminDao::new(&pool).list_users(page, page_size)).await?;
        Ok((users, page, page_size))
    }

    pub async fn approve_user(
        &self,
        admin_id: String,
        user_id: String,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id.clone()).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).approve_user(&user_id, &admin_id, db::now_ts()))
            .await
    }

    pub async fn set_admin(
        &self,
        admin_id: String,
        target_id: String,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).set_admin(&target_id, is_admin))
            .await
            .map_admin_conflict()
    }

    pub async fn set_user_status(
        &self,
        admin_id: String,
        user_id: String,
        status: String,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id).await?;
        let status = status.trim().to_string();
        match status.as_str() {
            "active" | "suspended" | "locked" => {}
            _ => return Err(ServiceError::bad_request("Invalid status")),
        }
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).set_user_status(&user_id, &status, db::now_ts()))
            .await
    }

    pub async fn reset_password(
        &self,
        admin_id: String,
        user_id: String,
    ) -> Result<String, ServiceError> {
        self.require_admin(admin_id).await?;
        AuthService::new(&self.db_pool)
            .admin_reset_password(user_id)
            .await
    }

    pub async fn delete_user(
        &self,
        admin_id: String,
        target_id: String,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).delete_user(&target_id))
            .await
            .map_admin_conflict()
    }

    pub async fn delete_content(
        &self,
        admin_id: String,
        kind: String,
        id: String,
    ) -> Result<(), ServiceError> {
        self.require_admin(admin_id).await?;
        let pool = self.db_pool.clone();
        block_dao(move || AdminDao::new(&pool).delete_content(&kind, &id)).await
    }

    async fn require_admin(&self, user_id: String) -> Result<User, ServiceError> {
        let user = AuthService::new(&self.db_pool).load_user(user_id).await?;
        if user.is_admin {
            Ok(user)
        } else {
            Err(ServiceError::forbidden(
                "Only a member of the EQ presidency can do that",
            ))
        }
    }
}

trait AdminResultExt<T> {
    fn map_admin_conflict(self) -> Result<T, ServiceError>;
    fn map_study_topic_conflict(self) -> Result<T, ServiceError>;
}

impl<T> AdminResultExt<T> for Result<T, ServiceError> {
    fn map_admin_conflict(self) -> Result<T, ServiceError> {
        self.map_err(|error| match error {
            ServiceError::BadRequest(message)
                if message == "There must always be at least one admin" =>
            {
                ServiceError::conflict(message)
            }
            other => other,
        })
    }

    fn map_study_topic_conflict(self) -> Result<T, ServiceError> {
        self.map_err(|error| match error {
            ServiceError::BadRequest(message)
                if message == "A study topic already exists for that week" =>
            {
                ServiceError::conflict(message)
            }
            other => other,
        })
    }
}

fn validate_event(input: UpsertEventInput) -> Result<EventInput, ServiceError> {
    let event_date = validated_iso_date(&input.event_date, "event date")?;
    let event_time = validated_optional_time(&input.event_time, "event time")?;
    let end_date = validated_optional_date(&input.end_date, "end date")?;
    let end_time = validated_optional_time(&input.end_time, "end time")?;
    validate_event_end(&event_date, &event_time, &end_date, &end_time)?;
    Ok(EventInput {
        name: required_trimmed(&input.name, "Event name")?,
        event_date,
        event_time,
        end_date,
        end_time,
        location: optional_trimmed(&input.location),
        description: optional_trimmed(&input.description),
    })
}

fn validate_study_topic(input: UpsertStudyTopicInput) -> Result<StudyTopicInput, ServiceError> {
    Ok(StudyTopicInput {
        week_start: validated_week_start(&input.week_start)?,
        name: required_trimmed(&input.name, "Study topic name")?,
        description: optional_trimmed(&input.description),
        hyperlink: validated_hyperlink(&input.hyperlink)?,
    })
}

fn required_trimmed(value: &str, field: &str) -> Result<String, ServiceError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ServiceError::bad_request(format!("{field} is required")));
    }
    Ok(value.to_string())
}

fn optional_trimmed(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn validated_iso_date(value: &str, field: &str) -> Result<String, ServiceError> {
    let value = value.trim();
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| ServiceError::bad_request(format!("Invalid {field}")))?;
    Ok(parsed.format("%Y-%m-%d").to_string())
}

fn validated_optional_date(
    value: &Option<String>,
    field: &str,
) -> Result<Option<String>, ServiceError> {
    let Some(value) = optional_trimmed(value) else {
        return Ok(None);
    };
    validated_iso_date(&value, field).map(Some)
}

fn validated_optional_time(
    value: &Option<String>,
    field: &str,
) -> Result<Option<String>, ServiceError> {
    let Some(value) = optional_trimmed(value) else {
        return Ok(None);
    };
    let parsed = NaiveTime::parse_from_str(&value, "%H:%M")
        .map_err(|_| ServiceError::bad_request(format!("Invalid {field}")))?;
    Ok(Some(parsed.format("%H:%M").to_string()))
}

fn validate_event_end(
    event_date: &str,
    event_time: &Option<String>,
    end_date: &Option<String>,
    end_time: &Option<String>,
) -> Result<(), ServiceError> {
    let start_date = NaiveDate::parse_from_str(event_date, "%Y-%m-%d")
        .map_err(|_| ServiceError::bad_request("Invalid event date"))?;
    let effective_end_date = match end_date {
        Some(end_date) => NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
            .map_err(|_| ServiceError::bad_request("Invalid end date"))?,
        None => start_date,
    };
    if effective_end_date < start_date {
        return Err(ServiceError::bad_request(
            "End date cannot be before event date",
        ));
    }
    if effective_end_date == start_date
        && let (Some(event_time), Some(end_time)) = (event_time, end_time)
    {
        let start_time = NaiveTime::parse_from_str(event_time, "%H:%M")
            .map_err(|_| ServiceError::bad_request("Invalid event time"))?;
        let end_time = NaiveTime::parse_from_str(end_time, "%H:%M")
            .map_err(|_| ServiceError::bad_request("Invalid end time"))?;
        if end_time < start_time {
            return Err(ServiceError::bad_request(
                "End time cannot be before event time",
            ));
        }
    }
    Ok(())
}

fn validated_week_start(value: &str) -> Result<String, ServiceError> {
    let parsed = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| ServiceError::bad_request("Invalid week_start"))?;
    if parsed.weekday() != Weekday::Mon {
        return Err(ServiceError::bad_request(
            "Please choose the Monday that starts the study week",
        ));
    }
    Ok(parsed.format("%Y-%m-%d").to_string())
}

fn validated_hyperlink(value: &Option<String>) -> Result<Option<String>, ServiceError> {
    let Some(value) = optional_trimmed(value) else {
        return Ok(None);
    };
    if value.starts_with("http://") || value.starts_with("https://") {
        Ok(Some(value))
    } else {
        Err(ServiceError::bad_request(
            "Hyperlink must start with http:// or https://",
        ))
    }
}

use actix_web::{HttpResponse, web};
use serde::Deserialize;

use crate::AppState;
use crate::handlers::HandlerError;
use crate::services::public::{CreateSurveyResponseServiceInput, PublicService};

#[derive(Debug, Deserialize)]
pub struct CreateSurveyResponseRequest {
    pub food_suggestions: Option<String>,
    pub dietary_restrictions: Option<String>,
}

pub async fn landing(state: web::Data<AppState>) -> Result<HttpResponse, HandlerError> {
    let (events, current_study_topic, has_upcoming_study_topics) =
        PublicService::new(&state.db_pool).landing().await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "upcoming_events": events,
        "current_study_topic": current_study_topic,
        "has_upcoming_study_topics": has_upcoming_study_topics
    })))
}

pub async fn upcoming_study_topics(
    state: web::Data<AppState>,
) -> Result<HttpResponse, HandlerError> {
    let topics = PublicService::new(&state.db_pool)
        .upcoming_study_topics()
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "topics": topics })))
}

pub async fn create_survey_response(
    state: web::Data<AppState>,
    body: web::Json<CreateSurveyResponseRequest>,
) -> Result<HttpResponse, HandlerError> {
    let created = PublicService::new(&state.db_pool)
        .create_survey_response(CreateSurveyResponseServiceInput {
            food_suggestions: body.food_suggestions.clone(),
            dietary_restrictions: body.dietary_restrictions.clone(),
        })
        .await?;
    Ok(HttpResponse::Created().json(created))
}

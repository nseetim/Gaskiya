use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("ipfs error: {0}")]
    Ipfs(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::NotFound => {
                HttpResponse::NotFound().json(serde_json::json!({ "error": self.to_string() }))
            }
            AppError::BadRequest(_) => {
                HttpResponse::BadRequest().json(serde_json::json!({ "error": self.to_string() }))
            }
            AppError::Db(_) | AppError::Serde(_) | AppError::Ipfs(_) => {
                log::error!("internal error: {self}");
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": "internal error" }))
            }
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;

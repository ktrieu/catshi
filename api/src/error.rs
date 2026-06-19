use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

pub struct AppError {
    inner: anyhow::Error,
    status: StatusCode,
}

impl<T> From<T> for AppError
where
    T: Into<anyhow::Error>,
{
    fn from(value: T) -> Self {
        // Generically assume a 500 if we're just using the ? operator.
        Self {
            inner: value.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl AppError {
    pub fn with_status_code(err: anyhow::Error, status: StatusCode) -> Self {
        Self { inner: err, status }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let json = Json(ErrorResponse {
            error: self.inner.to_string(),
        });

        (self.status, json).into_response()
    }
}

pub type HandlerResult<T> = Result<T, AppError>;

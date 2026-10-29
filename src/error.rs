use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

/// Spec standard error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ValidationFailed,
    Unauthorized,
    Forbidden,
    StoreInactive,
    NotFound,
    VisitAlreadyResolved,
    CouponAlreadyRedeemed,
    PairingCodeUsed,
    PairingCodeExpired,
    CouponExpired,
    VisitOutOfRadius,
    VisitNoParking,
    VisitTimeout,
    DailyLimit,
    PinInvalid,
    PinLocked,
    InternalError,
}

impl ErrorCode {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ErrorCode::ValidationFailed => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden | ErrorCode::StoreInactive => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::VisitAlreadyResolved
            | ErrorCode::CouponAlreadyRedeemed
            | ErrorCode::PairingCodeUsed => StatusCode::CONFLICT,
            ErrorCode::PairingCodeExpired | ErrorCode::CouponExpired => StatusCode::GONE,
            ErrorCode::VisitOutOfRadius
            | ErrorCode::VisitNoParking
            | ErrorCode::VisitTimeout
            | ErrorCode::DailyLimit
            | ErrorCode::PinInvalid => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::PinLocked => StatusCode::LOCKED,
            ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Inner error object inside `{ "error": { ... } }`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub code: ErrorCode,
    pub message: String,
    #[schema(value_type = Object)]
    pub details: Value,
}

/// Top-level error envelope (NFR-B4)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Value,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: json!({}),
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ValidationFailed, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }
}

impl From<worker::Error> for AppError {
    fn from(err: worker::Error) -> Self {
        Self::internal(format!("Database / Worker error: {}", err))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.code.status_code();
        let body = ErrorResponse {
            error: ErrorDetail {
                code: self.code,
                message: self.message,
                details: self.details,
            },
        };
        (status, Json(body)).into_response()
    }
}

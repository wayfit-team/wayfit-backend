use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        FromRequest, FromRequestParts,
    },
    http::request::Parts,
};
use serde_json::json;

use crate::error::AppError;

/// `axum::Json` 대신 쓰는 요청 본문 추출기.
/// 형식 오류를 axum 기본 텍스트 응답 대신 공통 에러 형식(400 VALIDATION_FAILED)으로 돌려준다 (NFR-B4).
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

/// `axum::extract::Query` 대신 쓰는 쿼리 추출기. 형식 오류를 400 VALIDATION_FAILED로 돌려준다.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct AppQuery<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        let message = match &rejection {
            JsonRejection::MissingJsonContentType(_) => {
                "Content-Type: application/json 헤더가 필요합니다"
            }
            JsonRejection::JsonSyntaxError(_) => "요청 본문이 올바른 JSON이 아닙니다",
            JsonRejection::JsonDataError(_) => "요청 본문의 필드가 누락되었거나 형식이 올바르지 않습니다",
            _ => "요청 본문을 읽을 수 없습니다",
        };
        AppError::validation(message).with_details(json!({ "reason": rejection.body_text() }))
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        AppError::validation("쿼리 파라미터가 누락되었거나 형식이 올바르지 않습니다")
            .with_details(json!({ "reason": rejection.body_text() }))
    }
}

/// Extracts vehicle UUID from `X-Device-Id` header
#[derive(Debug, Clone)]
pub struct DeviceId(pub String);

impl<S> FromRequestParts<S> for DeviceId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let val = parts
            .headers
            .get("x-device-id")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::unauthorized("X-Device-Id 헤더가 필요합니다 (Pleos 차량 앱 기기 UUID)")
            })?;

        Ok(DeviceId(val.to_string()))
    }
}

/// Extracts Kakao OAuth user ID from `X-Kakao-User-Id` header
#[derive(Debug, Clone)]
pub struct KakaoUserId(pub String);

impl<S> FromRequestParts<S> for KakaoUserId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let val = parts
            .headers
            .get("x-kakao-user-id")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::unauthorized(
                    "X-Kakao-User-Id 헤더가 필요합니다 (카카오 OAuth 회원 식별자)",
                )
            })?;

        Ok(KakaoUserId(val.to_string()))
    }
}

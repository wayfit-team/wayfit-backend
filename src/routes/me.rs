use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use worker::wasm_bindgen::JsValue;

use crate::db::{CouponRow, UserDeviceRow};
use crate::error::AppError;
use crate::extractors::KakaoUserId;
use crate::schemas::coupons::CouponListResponse;
use crate::schemas::pairing::{LinkedDeviceItem, MyDevicesResponse};
use crate::state::AppState;
use crate::time::now_utc;

/// 내 카카오 계정에 연결된 차량 기기 목록 조회 (REQ-B18)
#[utoipa::path(
    get,
    path = "/v1/me/devices",
    tag = "Driver Wallet (PWA)",
    security(("session_token" = [])),
    responses(
        (status = 200, description = "연결된 차량 기기 목록", body = MyDevicesResponse),
        (status = 401, description = "로그인 필요 또는 세션 만료 (UNAUTHORIZED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_my_devices(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
) -> Result<Json<MyDevicesResponse>, AppError> {
    let db = state.db()?;
    let rows: Vec<UserDeviceRow> = db
        .prepare(
            "SELECT * FROM user_devices
             WHERE kakao_user_id = ?1
             ORDER BY linked_at DESC",
        )
        .bind(&[JsValue::from_str(&kakao_user_id)])?
        .all()
        .await?
        .results::<UserDeviceRow>()?;

    let devices = rows
        .into_iter()
        .map(|r| LinkedDeviceItem {
            device_id: r.device_id,
            linked_at: r.linked_at,
        })
        .collect();

    Ok(Json(MyDevicesResponse { devices }))
}

/// 내 카카오 계정에서 특정 차량 기기 연결 해제 (REQ-B18)
#[utoipa::path(
    delete,
    path = "/v1/me/devices/{id}",
    tag = "Driver Wallet (PWA)",
    params(("id" = String, Path, description = "연결 해제할 차량 device_id")),
    security(("session_token" = [])),
    responses(
        (status = 204, description = "연결 해제 완료"),
        (status = 404, description = "연결되어 있지 않은 차량 기기", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn unlink_my_device(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
    Path(device_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let db = state.db()?;
    let res = db
        .prepare("DELETE FROM user_devices WHERE kakao_user_id = ?1 AND device_id = ?2")
        .bind(&[
            JsValue::from_str(&kakao_user_id),
            JsValue::from_str(&device_id),
        ])?
        .run()
        .await?;

    let changes = res.meta()?.and_then(|m| m.changes).unwrap_or(0);
    if changes == 0 {
        return Err(AppError::not_found("연결된 차량 기기를 찾을 수 없습니다"));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// 내 카카오 계정에 연결된 모든 차량 기기의 쿠폰 통합 조회 (REQ-B12, REQ-B14)
#[utoipa::path(
    get,
    path = "/v1/me/coupons",
    tag = "Driver Wallet (PWA)",
    security(("session_token" = [])),
    responses(
        (status = 200, description = "연결된 모든 차량의 쿠폰 목록 (최신 발급순)", body = CouponListResponse),
        (status = 401, description = "로그인 필요 또는 세션 만료 (UNAUTHORIZED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_my_coupons(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
) -> Result<Json<CouponListResponse>, AppError> {
    let db = state.db()?;
    let now = now_utc();

    let rows: Vec<CouponRow> = db
        .prepare(
            "SELECT c.* FROM coupons c
             INNER JOIN user_devices ud ON c.device_id = ud.device_id
             WHERE ud.kakao_user_id = ?1
             ORDER BY c.issued_at DESC",
        )
        .bind(&[JsValue::from_str(&kakao_user_id)])?
        .all()
        .await?
        .results::<CouponRow>()?;

    let coupons = rows.into_iter().map(|r| r.into_dto(now)).collect();
    Ok(Json(CouponListResponse { coupons }))
}

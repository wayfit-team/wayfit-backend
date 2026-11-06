use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use chrono::Duration;
use worker::wasm_bindgen::JsValue;

use crate::db::{CountRow, PairingCodeRow, UserDeviceRow};
use crate::error::{AppError, ErrorCode};
use crate::extractors::{AppJson, DeviceId, KakaoUserId};
use crate::ids::generate_pairing_code8;
use crate::schemas::pairing::{
    ClaimPairingRequest, ClaimPairingResponse, CreatePairingCodeResponse,
    DevicePairingStatusResponse,
};
use crate::state::AppState;
use crate::time::{format_rfc3339, now_utc, parse_rfc3339};

/// 차량 앱용 8자리 숫자 QR 페어링 코드 발급 (10분 유효, REQ-B15)
#[utoipa::path(
    post,
    path = "/v1/pairing-codes",
    tag = "Vehicle (Pleos App)",
    security(("device_token" = [])),
    responses(
        (status = 201, description = "발급된 8자리 페어링 코드 및 QR URL", body = CreatePairingCodeResponse),
        (status = 401, description = "기기 토큰 누락 또는 무효 (UNAUTHORIZED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn create_pairing_code(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
) -> Result<(StatusCode, Json<CreatePairingCodeResponse>), AppError> {
    // QR 이 엉뚱한 사이트로 가지 않도록, 주소가 설정되지 않았으면 코드를 만들기 전에 실패한다
    let base_url = state.pwa_base_url()?;
    let db = state.db()?;
    let now = now_utc();
    let expires_at = format_rfc3339(now + Duration::minutes(10));

    // 숫자 8자리라 이전 코드와 겹칠 수 있다. 겹치면 새로 뽑는다.
    let mut issued = None;
    for _ in 0..5 {
        let candidate = generate_pairing_code8();
        let res = db
            .prepare(
                "INSERT OR IGNORE INTO pairing_codes (code, device_id, expires_at)
                 VALUES (?1, ?2, ?3)",
            )
            .bind(&[
                JsValue::from_str(&candidate),
                JsValue::from_str(&device_id),
                JsValue::from_str(&expires_at),
            ])?
            .run()
            .await?;
        if res.meta()?.and_then(|m| m.changes).unwrap_or(0) > 0 {
            issued = Some(candidate);
            break;
        }
    }
    let code = issued.ok_or_else(|| AppError::internal("페어링 코드 발급에 실패했습니다"))?;

    let pairing_url = format!("{}/pair?code={}", base_url.trim_end_matches('/'), code);

    Ok((
        StatusCode::CREATED,
        Json(CreatePairingCodeResponse {
            code,
            expires_at,
            pairing_url,
        }),
    ))
}

/// 차량 기기에 연결된 폰(카카오 계정) 수 조회 (QR 화면 자동 갱신용, REQ-B17)
#[utoipa::path(
    get,
    path = "/v1/devices/me/pairing",
    tag = "Vehicle (Pleos App)",
    security(("device_token" = [])),
    responses(
        (status = 200, description = "연결된 사용자 계정 수", body = DevicePairingStatusResponse),
        (status = 401, description = "기기 토큰 누락 또는 무효 (UNAUTHORIZED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_device_pairing_status(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
) -> Result<Json<DevicePairingStatusResponse>, AppError> {
    let db = state.db()?;
    let row: Option<CountRow> = db
        .prepare("SELECT COUNT(*) as cnt FROM user_devices WHERE device_id = ?1")
        .bind(&[JsValue::from_str(&device_id)])?
        .first(None)
        .await?;

    Ok(Json(DevicePairingStatusResponse {
        device_id,
        linked_user_count: row.map(|r| r.cnt).unwrap_or(0),
    }))
}

/// PWA에서 8자리 숫자 페어링 코드 제출 → 카카오 계정에 차량 기기 연결 (REQ-B16, EDGE-B5)
#[utoipa::path(
    post,
    path = "/v1/pairing/claim",
    tag = "Driver Wallet (PWA)",
    request_body = ClaimPairingRequest,
    security(("session_token" = [])),
    responses(
        (status = 200, description = "차량 기기 연결 완료 (멱등)", body = ClaimPairingResponse),
        (status = 400, description = "숫자 8자리가 아닌 코드 (VALIDATION_FAILED)", body = crate::error::ErrorResponse),
        (status = 404, description = "존재하지 않는 페어링 코드", body = crate::error::ErrorResponse),
        (status = 409, description = "이미 다른 사용자가 사용한 코드 (PAIRING_CODE_USED)", body = crate::error::ErrorResponse),
        (status = 410, description = "유효기간(10분)이 만료된 코드 (PAIRING_CODE_EXPIRED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn claim_pairing_code(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
    AppJson(req): AppJson<ClaimPairingRequest>,
) -> Result<Json<ClaimPairingResponse>, AppError> {
    // QR URL 또는 직접 입력 모두 허용: 공백·하이픈 등 숫자 외 문자는 무시한다
    let code: String = req.code.chars().filter(|c| c.is_ascii_digit()).collect();
    if code.len() != 8 {
        return Err(AppError::validation("페어링 코드는 숫자 8자리입니다"));
    }

    let db = state.db()?;
    let now = now_utc();
    let now_str = format_rfc3339(now);

    let row: Option<PairingCodeRow> = db
        .prepare("SELECT * FROM pairing_codes WHERE code = ?1")
        .bind(&[JsValue::from_str(&code)])?
        .first(None)
        .await?;

    let row = row.ok_or_else(|| AppError::not_found("유효하지 않은 페어링 코드입니다"))?;

    // EDGE-B5: 이미 사용된 코드 처리 (동일 사용자가 같은 기기를 재연결하면 멱등 성공)
    if row.used_at.is_some() {
        if row.used_by_kakao_id.as_deref() == Some(kakao_user_id.as_str()) {
            let existing: Option<UserDeviceRow> = db
                .prepare(
                    "SELECT * FROM user_devices WHERE kakao_user_id = ?1 AND device_id = ?2",
                )
                .bind(&[
                    JsValue::from_str(&kakao_user_id),
                    JsValue::from_str(&row.device_id),
                ])?
                .first(None)
                .await?;

            let linked_at = existing
                .map(|e| e.linked_at)
                .unwrap_or_else(|| now_str.clone());

            return Ok(Json(ClaimPairingResponse {
                kakao_user_id,
                device_id: row.device_id,
                linked_at,
            }));
        }

        return Err(AppError::new(
            ErrorCode::PairingCodeUsed,
            "이미 사용된 1회용 페어링 코드입니다",
        ));
    }

    // EDGE-B5: 만료 여부 확인 (410 PAIRING_CODE_EXPIRED)
    if let Some(exp) = parse_rfc3339(&row.expires_at) {
        if now > exp {
            return Err(AppError::new(
                ErrorCode::PairingCodeExpired,
                "페어링 코드 유효시간(10분)이 만료되었습니다. 차량 화면에서 새로 발급해 주세요",
            ));
        }
    }

    // Mark code as used
    db.prepare(
        "UPDATE pairing_codes
         SET used_at = ?1, used_by_kakao_id = ?2
         WHERE code = ?3",
    )
    .bind(&[
        JsValue::from_str(&now_str),
        JsValue::from_str(&kakao_user_id),
        JsValue::from_str(&code),
    ])?
    .run()
    .await?;

    // Link user <-> device (idempotent if already linked via previous code)
    db.prepare(
        "INSERT OR IGNORE INTO user_devices (kakao_user_id, device_id, linked_at)
         VALUES (?1, ?2, ?3)",
    )
    .bind(&[
        JsValue::from_str(&kakao_user_id),
        JsValue::from_str(&row.device_id),
        JsValue::from_str(&now_str),
    ])?
    .run()
    .await?;

    Ok(Json(ClaimPairingResponse {
        kakao_user_id,
        device_id: row.device_id,
        linked_at: now_str,
    }))
}

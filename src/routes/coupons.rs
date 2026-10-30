use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Duration;
use serde_json::json;
use worker::wasm_bindgen::JsValue;

use crate::db::{CountRow, CouponRow, StoreRow};
use crate::error::{AppError, ErrorCode};
use crate::extractors::{AppJson, DeviceId, KakaoUserId};
use crate::schemas::coupons::{
    CouponListResponse, CouponStatus, RedeemCouponRequest, RedeemCouponResponse,
};
use crate::state::AppState;
use crate::time::{format_rfc3339, now_utc, parse_rfc3339, to_kst_date_str};

/// 차량 앱 기기에 귀속된 쿠폰 목록 조회 (REQ-B12, REQ-B14)
#[utoipa::path(
    get,
    path = "/v1/devices/me/coupons",
    tag = "Vehicle (Pleos App)",
    security(("device_token" = [])),
    responses(
        (status = 200, description = "차량 기기에 발급된 쿠폰 목록 (최신순)", body = CouponListResponse),
        (status = 401, description = "기기 토큰 누락 또는 무효 (UNAUTHORIZED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_device_coupons(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
) -> Result<Json<CouponListResponse>, AppError> {
    let db = state.db()?;
    let now = now_utc();

    let rows: Vec<CouponRow> = db
        .prepare("SELECT * FROM coupons WHERE device_id = ?1 ORDER BY issued_at DESC")
        .bind(&[JsValue::from_str(&device_id)])?
        .all()
        .await?
        .results::<CouponRow>()?;

    let coupons = rows.into_iter().map(|r| r.into_dto(now)).collect();
    Ok(Json(CouponListResponse { coupons }))
}

/// 매장 계산대 PIN 4자리로 쿠폰 사용 처리 (REQ-B13, EDGE-B6~B8)
#[utoipa::path(
    post,
    path = "/v1/coupons/{id}/redeem",
    tag = "Driver Wallet (PWA)",
    params(("id" = String, Path, description = "사용 처리할 쿠폰 ID")),
    request_body = RedeemCouponRequest,
    security(("session_token" = [])),
    responses(
        (status = 200, description = "쿠폰 사용 처리 완료 (REDEEMED)", body = RedeemCouponResponse),
        (status = 403, description = "본인 연결 기기의 쿠폰이 아니거나 비활성 매장 (STORE_INACTIVE)", body = crate::error::ErrorResponse),
        (status = 404, description = "존재하지 않는 쿠폰", body = crate::error::ErrorResponse),
        (status = 409, description = "이미 사용된 쿠폰 (COUPON_ALREADY_REDEEMED)", body = crate::error::ErrorResponse),
        (status = 410, description = "유효기간이 만료된 쿠폰 (COUPON_EXPIRED)", body = crate::error::ErrorResponse),
        (status = 422, description = "매장 PIN 불일치 (PIN_INVALID)", body = crate::error::ErrorResponse),
        (status = 423, description = "PIN 5회 연속 실패로 15분 잠금 (PIN_LOCKED)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn redeem_coupon(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
    Path(coupon_id): Path<String>,
    AppJson(req): AppJson<RedeemCouponRequest>,
) -> Result<Json<RedeemCouponResponse>, AppError> {
    let db = state.db()?;
    let now = now_utc();

    let coupon: Option<CouponRow> = db
        .prepare("SELECT * FROM coupons WHERE id = ?1")
        .bind(&[JsValue::from_str(&coupon_id)])?
        .first(None)
        .await?;

    let mut coupon = coupon.ok_or_else(|| AppError::not_found("쿠폰을 찾을 수 없습니다"))?;

    // Check that this Kakao user is linked to the coupon's device_id
    let link_cnt: Option<CountRow> = db
        .prepare(
            "SELECT COUNT(*) as cnt FROM user_devices
             WHERE kakao_user_id = ?1 AND device_id = ?2",
        )
        .bind(&[
            JsValue::from_str(&kakao_user_id),
            JsValue::from_str(&coupon.device_id),
        ])?
        .first(None)
        .await?;

    if link_cnt.map(|r| r.cnt).unwrap_or(0) == 0 {
        return Err(AppError::forbidden(
            "내 계정에 연결된 차량의 쿠폰만 사용할 수 있습니다",
        ));
    }

    // EDGE-B8: 매장이 삭제·비활성화된 뒤 남은 쿠폰 → 사용 처리 시 403 STORE_INACTIVE
    let store: Option<StoreRow> = db
        .prepare("SELECT * FROM stores WHERE id = ?1")
        .bind(&[JsValue::from_str(&coupon.store_id)])?
        .first(None)
        .await?;

    let store = match store {
        Some(s) if s.subscription_active != 0 => s,
        _ => {
            return Err(AppError::new(
                ErrorCode::StoreInactive,
                "비활성화되었거나 종료된 매장의 쿠폰은 사용할 수 없습니다",
            ))
        }
    };

    // Check current status
    if coupon.status == "REDEEMED" {
        return Err(AppError::new(
            ErrorCode::CouponAlreadyRedeemed,
            "이미 사용 완료된 쿠폰입니다",
        ));
    }

    if coupon.effective_status(now) == CouponStatus::Expired {
        return Err(AppError::new(
            ErrorCode::CouponExpired,
            "유효기간이 만료된 쿠폰입니다",
        ));
    }

    // REQ-B13: Check if PIN is currently locked
    if let Some(ref locked_str) = coupon.locked_until {
        if let Some(locked_dt) = parse_rfc3339(locked_str) {
            if locked_dt > now {
                let retry_sec = (locked_dt - now).num_seconds().max(1);
                return Err(AppError::new(
                    ErrorCode::PinLocked,
                    "PIN 입력이 5회 연속 실패하여 잠시 잠겼습니다",
                )
                .with_details(json!({ "retryAfterSec": retry_sec })));
            }
        }
    }

    // EDGE-B7: 매장 PIN 변경 후 기존 쿠폰도 현재 매장의 새 PIN으로 검증
    if req.pin.trim() != store.pin_code {
        let new_fails = coupon.pin_fail_count + 1;
        if new_fails >= 5 {
            let lock_until = now + Duration::minutes(15);
            let lock_str = format_rfc3339(lock_until);
            db.prepare(
                "UPDATE coupons SET pin_fail_count = ?1, locked_until = ?2 WHERE id = ?3",
            )
            .bind(&[
                JsValue::from_f64(new_fails as f64),
                JsValue::from_str(&lock_str),
                JsValue::from_str(&coupon.id),
            ])?
            .run()
            .await?;

            return Err(AppError::new(
                ErrorCode::PinLocked,
                "PIN 5회 연속 오류로 15분간 사용 처리가 잠겼습니다",
            )
            .with_details(json!({ "retryAfterSec": 900 })));
        } else {
            db.prepare("UPDATE coupons SET pin_fail_count = ?1 WHERE id = ?2")
                .bind(&[
                    JsValue::from_f64(new_fails as f64),
                    JsValue::from_str(&coupon.id),
                ])?
                .run()
                .await?;

            return Err(
                AppError::new(ErrorCode::PinInvalid, "매장 PIN 번호가 일치하지 않습니다")
                    .with_details(json!({ "failCount": new_fails, "remainingAttempts": 5 - new_fails })),
            );
        }
    }

    // EDGE-B6: 동시 사용 요청 방지를 위한 원자적 조건부 업데이트 (status = 'ISSUED' 조건)
    let now_str = format_rfc3339(now);
    let today_kst = to_kst_date_str(now);
    let update_res = db
        .prepare(
            "UPDATE coupons
             SET status = 'REDEEMED',
                 redeemed_at = ?1,
                 redeemed_kst_date = ?2,
                 pin_fail_count = 0,
                 locked_until = NULL
             WHERE id = ?3 AND status = 'ISSUED'",
        )
        .bind(&[
            JsValue::from_str(&now_str),
            JsValue::from_str(&today_kst),
            JsValue::from_str(&coupon.id),
        ])?
        .run()
        .await?;

    let changes = update_res.meta()?.and_then(|m| m.changes).unwrap_or(0);
    if changes == 0 {
        return Err(AppError::new(
            ErrorCode::CouponAlreadyRedeemed,
            "이미 사용 처리된 쿠폰입니다",
        ));
    }

    coupon.status = "REDEEMED".to_string();
    coupon.redeemed_at = Some(now_str);
    coupon.redeemed_kst_date = Some(today_kst);
    coupon.pin_fail_count = 0;
    coupon.locked_until = None;

    Ok(Json(RedeemCouponResponse {
        coupon: coupon.into_dto(now),
    }))
}

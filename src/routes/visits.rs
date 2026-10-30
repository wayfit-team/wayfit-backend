use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Duration;
use worker::wasm_bindgen::JsValue;

use crate::db::{CountRow, CouponRow, StoreRow, VisitRow};
use crate::domain::verify::{evaluate_visit_verification, EvidenceItem, VerifyContext, VerifyOutcome};
use crate::error::{AppError, ErrorCode};
use crate::extractors::DeviceId;
use crate::ids::{generate_coupon_code6, new_id};
use crate::schemas::visits::{
    CreateVisitRequest, CreateVisitResponse, VerifyVisitRequest, VerifyVisitResponse, VisitStatus,
    VisitSummary,
};
use crate::state::AppState;
use crate::time::{format_rfc3339, now_utc, parse_rfc3339, to_kst_date_str};

/// 방문 생성 — "들를게요" 선택 시 호출 (REQ-B5)
#[utoipa::path(
    post,
    path = "/v1/visits",
    tag = "Vehicle (Pleos App)",
    request_body = CreateVisitRequest,
    security(("device_id" = [])),
    responses(
        (status = 201, description = "생성된 방문 정보 (이전 진행 중 방문은 자동 취소됨)", body = CreateVisitResponse),
        (status = 404, description = "존재하지 않는 매장", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn create_visit(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
    Json(req): Json<CreateVisitRequest>,
) -> Result<(StatusCode, Json<CreateVisitResponse>), AppError> {
    if req.store_id.trim().is_empty() || req.trip_id.trim().is_empty() {
        return Err(AppError::validation("storeId와 tripId는 필수입니다"));
    }

    let db = state.db()?;
    let store: Option<StoreRow> = db
        .prepare("SELECT * FROM stores WHERE id = ?1")
        .bind(&[JsValue::from_str(req.store_id.trim())])?
        .first(None)
        .await?;

    let store = store.ok_or_else(|| AppError::not_found("해당 매장을 찾을 수 없습니다"))?;
    if store.paused != 0 || store.subscription_active == 0 {
        return Err(AppError::new(
            ErrorCode::StoreInactive,
            "현재 추천이 일시중지되었거나 비활성화된 매장입니다",
        ));
    }

    let now = now_utc();
    let now_str = format_rfc3339(now);
    let kst_date = to_kst_date_str(now);

    // REQ-B5: 기기당 진행 중 방문은 1개이며, 새 방문을 만들면 이전 ACCEPTED 방문은 CANCELLED가 된다
    db.prepare(
        "UPDATE visits
         SET status = 'CANCELLED', resolved_at = ?1
         WHERE device_id = ?2 AND status = 'ACCEPTED'",
    )
    .bind(&[JsValue::from_str(&now_str), JsValue::from_str(&device_id)])?
    .run()
    .await?;

    let visit_id = new_id("visit");
    db.prepare(
        "INSERT INTO visits (id, device_id, store_id, trip_id, status, created_at, kst_date)
         VALUES (?1, ?2, ?3, ?4, 'ACCEPTED', ?5, ?6)",
    )
    .bind(&[
        JsValue::from_str(&visit_id),
        JsValue::from_str(&device_id),
        JsValue::from_str(&store.id),
        JsValue::from_str(req.trip_id.trim()),
        JsValue::from_str(&now_str),
        JsValue::from_str(&kst_date),
    ])?
    .run()
    .await?;

    let summary = VisitSummary {
        id: visit_id,
        store_id: store.id,
        trip_id: req.trip_id.trim().to_string(),
        status: VisitStatus::Accepted,
        created_at: now_str,
        resolved_at: None,
        reject_reason: None,
        confidence: None,
    };

    Ok((StatusCode::CREATED, Json(CreateVisitResponse { visit: summary })))
}

/// 진행 중 방문 취소 (REQ-B6)
#[utoipa::path(
    delete,
    path = "/v1/visits/{id}",
    tag = "Vehicle (Pleos App)",
    params(("id" = String, Path, description = "취소할 방문 ID")),
    security(("device_id" = [])),
    responses(
        (status = 200, description = "취소된 방문 정보", body = CreateVisitResponse),
        (status = 404, description = "존재하지 않거나 다른 기기의 방문", body = crate::error::ErrorResponse),
        (status = 409, description = "이미 완료되었거나 취소된 방문", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn cancel_visit(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
    Path(visit_id): Path<String>,
) -> Result<Json<CreateVisitResponse>, AppError> {
    let db = state.db()?;
    let visit: Option<VisitRow> = db
        .prepare("SELECT * FROM visits WHERE id = ?1 AND device_id = ?2")
        .bind(&[
            JsValue::from_str(&visit_id),
            JsValue::from_str(&device_id),
        ])?
        .first(None)
        .await?;

    let mut visit = visit.ok_or_else(|| AppError::not_found("방문 기록을 찾을 수 없습니다"))?;
    let now = now_utc();

    if visit.status != "ACCEPTED" {
        return Err(AppError::new(
            ErrorCode::VisitAlreadyResolved,
            "이미 처리 완료되었거나 취소된 방문입니다",
        ));
    }

    let now_str = format_rfc3339(now);
    db.prepare(
        "UPDATE visits SET status = 'CANCELLED', resolved_at = ?1
         WHERE id = ?2 AND device_id = ?3",
    )
    .bind(&[
        JsValue::from_str(&now_str),
        JsValue::from_str(&visit_id),
        JsValue::from_str(&device_id),
    ])?
    .run()
    .await?;

    visit.status = "CANCELLED".to_string();
    visit.resolved_at = Some(now_str);

    Ok(Json(CreateVisitResponse {
        visit: visit.to_summary(now),
    }))
}

/// 차량 센서 증거 제출 및 방문 인증·쿠폰 발급 (REQ-B8~B11, EDGE-B1~B4, EDGE-B10)
#[utoipa::path(
    post,
    path = "/v1/visits/{id}/verify",
    tag = "Vehicle (Pleos App)",
    params(("id" = String, Path, description = "인증할 방문 ID")),
    request_body = VerifyVisitRequest,
    security(("device_id" = [])),
    responses(
        (status = 200, description = "방문 인증 성공 및 발급된 쿠폰 반환 (멱등)", body = VerifyVisitResponse),
        (status = 404, description = "존재하지 않거나 다른 기기의 방문 (EDGE-B10)", body = crate::error::ErrorResponse),
        (status = 409, description = "취소된 방문", body = crate::error::ErrorResponse),
        (status = 422, description = "인증 조건 미달 (반경 밖, 주차 미확인, 시간 초과, 일일 한도)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn verify_visit(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
    Path(visit_id): Path<String>,
    Json(req): Json<VerifyVisitRequest>,
) -> Result<Json<VerifyVisitResponse>, AppError> {
    let db = state.db()?;
    let now = now_utc();

    // EDGE-B10: 존재하지 않거나 남의 방문 ID로 인증 요청 → 404
    let visit: Option<VisitRow> = db
        .prepare("SELECT * FROM visits WHERE id = ?1 AND device_id = ?2")
        .bind(&[
            JsValue::from_str(&visit_id),
            JsValue::from_str(&device_id),
        ])?
        .first(None)
        .await?;

    let mut visit = visit.ok_or_else(|| AppError::not_found("방문 기록을 찾을 수 없습니다"))?;

    // REQ-B10: 인증 요청은 멱등이다. 이미 VERIFIED면 기존 쿠폰과 함께 반환한다.
    if visit.status == "VERIFIED" {
        let existing_coupon: Option<CouponRow> = db
            .prepare("SELECT * FROM coupons WHERE visit_id = ?1")
            .bind(&[JsValue::from_str(&visit.id)])?
            .first(None)
            .await?;

        return Ok(Json(VerifyVisitResponse {
            visit: visit.to_summary(now),
            coupon: existing_coupon.map(|c| c.into_dto(now)),
        }));
    }

    if visit.status == "CANCELLED" {
        return Err(AppError::new(
            ErrorCode::VisitAlreadyResolved,
            "이미 취소된 방문입니다",
        ));
    }

    if visit.status == "REJECTED" {
        let code = match visit.reject_reason.as_deref() {
            Some("VISIT_OUT_OF_RADIUS") => ErrorCode::VisitOutOfRadius,
            Some("VISIT_NO_PARKING") => ErrorCode::VisitNoParking,
            Some("VISIT_TIMEOUT") => ErrorCode::VisitTimeout,
            Some("DAILY_LIMIT") => ErrorCode::DailyLimit,
            _ => ErrorCode::VisitAlreadyResolved,
        };
        return Err(AppError::new(
            code,
            "이미 인증 거절 판정이 완료된 방문입니다 (멱등 응답)",
        ));
    }

    // EDGE-B4: 방문 진행 중 매장이 일시중지되더라도 이미 ACCEPTED인 방문은 인증 허용
    let store: Option<StoreRow> = db
        .prepare("SELECT * FROM stores WHERE id = ?1")
        .bind(&[JsValue::from_str(&visit.store_id)])?
        .first(None)
        .await?;
    let store = store.ok_or_else(|| AppError::not_found("매장 정보를 찾을 수 없습니다"))?;

    // Parse & store evidences (NFR-B3)
    let mut parsed_evidences = Vec::with_capacity(req.evidence.len());
    for dto in &req.evidence {
        let occurred_at = parse_rfc3339(&dto.occurred_at).ok_or_else(|| {
            AppError::validation(format!(
                "유효하지 않은 occurredAt 타임스탬프 형식입니다: {}",
                dto.occurred_at
            ))
        })?;
        parsed_evidences.push(EvidenceItem {
            evidence_type: dto.evidence_type,
            occurred_at,
            lat: dto.lat,
            lng: dto.lng,
        });

        let lat_js = dto.lat.map(JsValue::from_f64).unwrap_or(JsValue::NULL);
        let lng_js = dto.lng.map(JsValue::from_f64).unwrap_or(JsValue::NULL);
        let source_js = dto
            .source
            .map(|s| JsValue::from_str(s.as_str()))
            .unwrap_or(JsValue::NULL);
        db.prepare(
            "INSERT INTO visit_evidences (visit_id, type, source, occurred_at, lat, lng)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&[
            JsValue::from_str(&visit.id),
            JsValue::from_str(dto.evidence_type.as_str()),
            source_js,
            JsValue::from_str(&dto.occurred_at),
            lat_js,
            lng_js,
        ])?
        .run()
        .await?;
    }

    // Check if coupon was already issued today (KST) for this device & store (REQ-B9)
    let today_kst = to_kst_date_str(now);
    let count_row: Option<CountRow> = db
        .prepare(
            "SELECT COUNT(*) as cnt FROM coupons
             WHERE device_id = ?1 AND store_id = ?2 AND issued_kst_date = ?3",
        )
        .bind(&[
            JsValue::from_str(&device_id),
            JsValue::from_str(&store.id),
            JsValue::from_str(&today_kst),
        ])?
        .first(None)
        .await?;
    let already_issued_today = count_row.map(|r| r.cnt > 0).unwrap_or(false);

    let visit_created_at = parse_rfc3339(&visit.created_at).unwrap_or(now);
    let ctx = VerifyContext {
        visit_created_at,
        server_now: now,
        store_lat: store.lat,
        store_lng: store.lng,
        store_radius_m: store.radius_m as f64,
        already_issued_today,
    };

    let now_str = format_rfc3339(now);

    match evaluate_visit_verification(&ctx, &parsed_evidences) {
        VerifyOutcome::Verified { confidence } => {
            db.prepare(
                "UPDATE visits
                 SET status = 'VERIFIED', resolved_at = ?1, confidence = ?2, reject_reason = NULL
                 WHERE id = ?3",
            )
            .bind(&[
                JsValue::from_str(&now_str),
                JsValue::from_f64(confidence as f64),
                JsValue::from_str(&visit.id),
            ])?
            .run()
            .await?;

            // Issue coupon (REQ-B11: snapshot store name & benefit, 6-digit code, 7-day expiry)
            let coupon_id = new_id("cpn");
            let code6 = generate_coupon_code6();
            let expires_at = format_rfc3339(now + Duration::days(7));

            db.prepare(
                "INSERT INTO coupons (
                    id, visit_id, device_id, store_id, code6,
                    store_name_snapshot, benefit_snapshot, status,
                    issued_at, issued_kst_date, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ISSUED', ?8, ?9, ?10)",
            )
            .bind(&[
                JsValue::from_str(&coupon_id),
                JsValue::from_str(&visit.id),
                JsValue::from_str(&device_id),
                JsValue::from_str(&store.id),
                JsValue::from_str(&code6),
                JsValue::from_str(&store.name),
                JsValue::from_str(&store.benefit_text),
                JsValue::from_str(&now_str),
                JsValue::from_str(&today_kst),
                JsValue::from_str(&expires_at),
            ])?
            .run()
            .await?;

            visit.status = "VERIFIED".to_string();
            visit.resolved_at = Some(now_str.clone());
            visit.confidence = Some(confidence);
            visit.reject_reason = None;

            let coupon_row = CouponRow {
                id: coupon_id,
                visit_id: visit.id.clone(),
                device_id,
                store_id: store.id,
                code6,
                store_name_snapshot: store.name,
                benefit_snapshot: store.benefit_text,
                status: "ISSUED".to_string(),
                issued_at: now_str,
                issued_kst_date: today_kst,
                expires_at,
                redeemed_at: None,
                redeemed_kst_date: None,
                pin_fail_count: 0,
                locked_until: None,
            };

            Ok(Json(VerifyVisitResponse {
                visit: visit.to_summary(now),
                coupon: Some(coupon_row.into_dto(now)),
            }))
        }
        VerifyOutcome::Rejected { reason, message } => {
            let reason_str = serde_json::to_value(reason)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "REJECTED".to_string());

            db.prepare(
                "UPDATE visits
                 SET status = 'REJECTED', resolved_at = ?1, reject_reason = ?2
                 WHERE id = ?3",
            )
            .bind(&[
                JsValue::from_str(&now_str),
                JsValue::from_str(&reason_str),
                JsValue::from_str(&visit.id),
            ])?
            .run()
            .await?;

            Err(AppError::new(reason, message))
        }
    }
}

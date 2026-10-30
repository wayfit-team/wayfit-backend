use axum::{extract::State, Json};
use worker::wasm_bindgen::JsValue;

use crate::db::{IdRow, StoreRow};
use crate::error::{AppError, ErrorCode};
use crate::extractors::DeviceId;
use crate::ids::new_id;
use crate::schemas::stores::{CreateImpressionRequest, CreateImpressionResponse};
use crate::state::AppState;
use crate::time::{format_rfc3339, now_utc, to_kst_date_str};

/// 추천 카드 노출 기록 (REQ-B4: 기기·매장·트립 당 1회 중복 제거)
#[utoipa::path(
    post,
    path = "/v1/impressions",
    tag = "Vehicle (Pleos App)",
    request_body = CreateImpressionRequest,
    security(("device_id" = [])),
    responses(
        (status = 200, description = "노출 이벤트 기록 결과. 같은 기기·매장·트립의 중복 노출이면 recorded=false와 처음 기록된 impressionId를 준다", body = CreateImpressionResponse),
        (status = 400, description = "필수 파라미터 누락", body = crate::error::ErrorResponse),
        (status = 401, description = "X-Device-Id 헤더 누락", body = crate::error::ErrorResponse),
        (status = 403, description = "일시중지되었거나 구독이 끝난 매장 (STORE_INACTIVE)", body = crate::error::ErrorResponse),
        (status = 404, description = "존재하지 않는 매장", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn create_impression(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
    Json(req): Json<CreateImpressionRequest>,
) -> Result<Json<CreateImpressionResponse>, AppError> {
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
    let shown_at = format_rfc3339(now);
    let kst_date = to_kst_date_str(now);
    let imp_id = new_id("imp");

    let res = db
        .prepare(
            "INSERT OR IGNORE INTO impressions (id, device_id, store_id, trip_id, shown_at, kst_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&[
            JsValue::from_str(&imp_id),
            JsValue::from_str(&device_id),
            JsValue::from_str(&store.id),
            JsValue::from_str(req.trip_id.trim()),
            JsValue::from_str(&shown_at),
            JsValue::from_str(&kst_date),
        ])?
        .run()
        .await?;

    let changes = res.meta()?.and_then(|m| m.changes).unwrap_or(0);
    if changes > 0 {
        return Ok(Json(CreateImpressionResponse {
            recorded: true,
            impression_id: imp_id,
        }));
    }

    // REQ-B4: 중복 노출이면 처음 기록된 노출 ID를 돌려준다
    let existing: Option<IdRow> = db
        .prepare(
            "SELECT id FROM impressions
             WHERE device_id = ?1 AND store_id = ?2 AND trip_id = ?3",
        )
        .bind(&[
            JsValue::from_str(&device_id),
            JsValue::from_str(&store.id),
            JsValue::from_str(req.trip_id.trim()),
        ])?
        .first(None)
        .await?;

    Ok(Json(CreateImpressionResponse {
        recorded: false,
        impression_id: existing.map(|r| r.id).unwrap_or(imp_id),
    }))
}

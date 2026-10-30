use axum::{
    extract::State,
    Json,
};
use std::collections::HashSet;
use worker::wasm_bindgen::JsValue;

use crate::db::{StoreIdRow, StoreRow};
use crate::domain::corridor::{compute_bbox, evaluate_store_corridor, CorridorQuery};
use crate::domain::hours::is_store_open;
use crate::error::AppError;
use crate::extractors::{AppQuery, DeviceId};
use crate::schemas::stores::{
    NearbyStoreItem, NearbyStoresQuery, NearbyStoresResponse, StoreCategory,
};
use crate::state::AppState;
use crate::time::{now_utc, to_kst_date_str};

/// 경로상 매장 탐색 (REQ-B2, REQ-B3, EDGE-B9, NFR-B3)
#[utoipa::path(
    get,
    path = "/v1/stores/nearby",
    tag = "Vehicle (Pleos App)",
    params(NearbyStoresQuery),
    security(("device_id" = [])),
    responses(
        (status = 200, description = "앞으로 지나갈 경로상 추천 매장 목록 (우회 거리순)", body = NearbyStoresResponse),
        (status = 400, description = "잘못된 좌표/파라미터", body = crate::error::ErrorResponse),
        (status = 401, description = "X-Device-Id 헤더 누락", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_nearby_stores(
    State(state): State<AppState>,
    DeviceId(device_id): DeviceId,
    AppQuery(query): AppQuery<NearbyStoresQuery>,
) -> Result<Json<NearbyStoresResponse>, AppError> {
    if !(-90.0..=90.0).contains(&query.lat) || !(-180.0..=180.0).contains(&query.lng) {
        return Err(AppError::validation("유효하지 않은 위경도 좌표입니다"));
    }

    let limit = query.limit.unwrap_or(3).clamp(1, 10);
    let corridor_q = CorridorQuery {
        lat: query.lat,
        lng: query.lng,
        heading: query.heading,
        dest_lat: query.dest_lat,
        dest_lng: query.dest_lng,
    };

    let bbox = compute_bbox(&corridor_q);
    let db = state.db()?;
    let now = now_utc();
    let today_kst = to_kst_date_str(now);

    // 1. D1 bbox query for active, unpaused stores
    let stmt = db.prepare(
        "SELECT * FROM stores
         WHERE paused = 0
           AND subscription_active = 1
           AND lat BETWEEN ?1 AND ?2
           AND lng BETWEEN ?3 AND ?4",
    );
    let candidates: Vec<StoreRow> = stmt
        .bind(&[
            JsValue::from_f64(bbox.min_lat),
            JsValue::from_f64(bbox.max_lat),
            JsValue::from_f64(bbox.min_lng),
            JsValue::from_f64(bbox.max_lng),
        ])?
        .all()
        .await?
        .results::<StoreRow>()?;

    // 2. Fetch store IDs where this device already received a coupon today (REQ-B2, REQ-B9)
    let issued_today_rows: Vec<StoreIdRow> = db
        .prepare(
            "SELECT store_id FROM coupons
             WHERE device_id = ?1 AND issued_kst_date = ?2",
        )
        .bind(&[
            JsValue::from_str(&device_id),
            JsValue::from_str(&today_kst),
        ])?
        .all()
        .await?
        .results::<StoreIdRow>()?;

    let excluded_stores: HashSet<String> =
        issued_today_rows.into_iter().map(|r| r.store_id).collect();

    // 3. Filter by business hours and corridor geometry
    let mut matched: Vec<NearbyStoreItem> = candidates
        .into_iter()
        .filter(|s| !excluded_stores.contains(&s.id))
        .filter(|s| is_store_open(&s.open_hours_json, now))
        .filter_map(|s| {
            let m = evaluate_store_corridor(&corridor_q, s.lat, s.lng)?;
            Some(NearbyStoreItem {
                store_id: s.id,
                name: s.name,
                category: StoreCategory::from_db(&s.category),
                benefit_text: s.benefit_text,
                lat: s.lat,
                lng: s.lng,
                distance_m: m.distance_m,
                detour_m: m.detour_m,
            })
        })
        .collect();

    // 4. Sort by detour_m ASC, then distance_m ASC (REQ-B2)
    matched.sort_by(|a, b| {
        a.detour_m
            .cmp(&b.detour_m)
            .then_with(|| a.distance_m.cmp(&b.distance_m))
    });
    matched.truncate(limit);

    Ok(Json(NearbyStoresResponse { stores: matched }))
}

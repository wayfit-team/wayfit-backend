use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::collections::HashMap;
use worker::wasm_bindgen::JsValue;

use crate::db::{DateCountRow, StoreRow};
use crate::error::AppError;
use crate::extractors::KakaoUserId;
use crate::schemas::owner::{
    DailyStatItem, OwnerStoreResponse, OwnerStoresResponse, StoreStatsQuery, StoreStatsResponse,
    StoreStatsTotals, UpdateOwnerStoreRequest,
};
use crate::state::AppState;
use crate::time::{now_utc, recent_kst_dates};

/// 사장님 본인 소유 매장 목록 조회 (REQ-B20)
#[utoipa::path(
    get,
    path = "/v1/owner/stores",
    tag = "Store Owner (PWA /owner)",
    security(("kakao_user_id" = [])),
    responses(
        (status = 200, description = "내 카카오 계정에 등록된 매장 목록", body = OwnerStoresResponse),
        (status = 401, description = "X-Kakao-User-Id 헤더 누락", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_owner_stores(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
) -> Result<Json<OwnerStoresResponse>, AppError> {
    let db = state.db()?;
    let rows: Vec<StoreRow> = db
        .prepare("SELECT * FROM stores WHERE owner_kakao_id = ?1 ORDER BY created_at ASC")
        .bind(&[JsValue::from_str(&kakao_user_id)])?
        .all()
        .await?
        .results::<StoreRow>()?;

    let stores = rows.into_iter().map(|r| r.into_owner_dto()).collect();
    Ok(Json(OwnerStoresResponse { stores }))
}

/// 사장님 매장 정보·혜택·반경·PIN·일시중지 수정 (REQ-B20)
#[utoipa::path(
    patch,
    path = "/v1/owner/stores/{id}",
    tag = "Store Owner (PWA /owner)",
    params(("id" = String, Path, description = "수정할 매장 ID")),
    request_body = UpdateOwnerStoreRequest,
    security(("kakao_user_id" = [])),
    responses(
        (status = 200, description = "수정된 매장 상세 정보", body = OwnerStoreResponse),
        (status = 400, description = "혜택 40자 초과, 잘못된 반경(100/150/300 외), PIN 형식 오류", body = crate::error::ErrorResponse),
        (status = 403, description = "본인 소유 매장이 아님", body = crate::error::ErrorResponse),
        (status = 404, description = "존재하지 않는 매장", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn update_owner_store(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
    Path(store_id): Path<String>,
    Json(req): Json<UpdateOwnerStoreRequest>,
) -> Result<Json<OwnerStoreResponse>, AppError> {
    let db = state.db()?;
    let store: Option<StoreRow> = db
        .prepare("SELECT * FROM stores WHERE id = ?1")
        .bind(&[JsValue::from_str(&store_id)])?
        .first(None)
        .await?;

    let mut store = store.ok_or_else(|| AppError::not_found("매장을 찾을 수 없습니다"))?;
    if store.owner_kakao_id != kakao_user_id {
        return Err(AppError::forbidden("본인 소유의 매장만 수정할 수 있습니다"));
    }

    if let Some(name) = req.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation("매장 이름은 비워둘 수 없습니다"));
        }
        store.name = trimmed.to_string();
    }

    if let Some(cat) = req.category {
        store.category = cat.as_str().to_string();
    }

    if let Some(addr) = req.address {
        let trimmed = addr.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation("매장 주소는 비워둘 수 없습니다"));
        }
        store.address = trimmed.to_string();
    }

    if let Some(lat) = req.lat {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(AppError::validation("유효하지 않은 위도(lat)입니다"));
        }
        store.lat = lat;
    }

    if let Some(lng) = req.lng {
        if !(-180.0..=180.0).contains(&lng) {
            return Err(AppError::validation("유효하지 않은 경도(lng)입니다"));
        }
        store.lng = lng;
    }

    if let Some(benefit) = req.benefit_text {
        let trimmed = benefit.trim();
        let char_count = trimmed.chars().count();
        if char_count == 0 || char_count > 40 {
            return Err(AppError::validation(
                "혜택 문구(benefitText)는 1자 이상 40자 이하여야 합니다",
            ));
        }
        store.benefit_text = trimmed.to_string();
    }

    if let Some(radius_m) = req.radius_m {
        if !matches!(radius_m, 100 | 150 | 300) {
            return Err(AppError::validation(
                "인증 반경(radiusM)은 100, 150, 300(m) 중 하나여야 합니다",
            ));
        }
        store.radius_m = radius_m;
    }

    if let Some(open_hours) = req.open_hours {
        store.open_hours_json = serde_json::to_string(&open_hours)
            .map_err(|_| AppError::validation("영업시간 JSON 직렬화에 실패했습니다"))?;
    }

    if let Some(pin) = req.pin_code {
        let trimmed = pin.trim();
        if trimmed.len() != 4 || !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::validation(
                "매장 PIN 코드는 숫자 4자리여야 합니다",
            ));
        }
        store.pin_code = trimmed.to_string();
    }

    if let Some(paused) = req.paused {
        store.paused = if paused { 1 } else { 0 };
    }

    db.prepare(
        "UPDATE stores
         SET name = ?1,
             category = ?2,
             address = ?3,
             lat = ?4,
             lng = ?5,
             benefit_text = ?6,
             radius_m = ?7,
             open_hours_json = ?8,
             pin_code = ?9,
             paused = ?10
         WHERE id = ?11",
    )
    .bind(&[
        JsValue::from_str(&store.name),
        JsValue::from_str(&store.category),
        JsValue::from_str(&store.address),
        JsValue::from_f64(store.lat),
        JsValue::from_f64(store.lng),
        JsValue::from_str(&store.benefit_text),
        JsValue::from_f64(store.radius_m as f64),
        JsValue::from_str(&store.open_hours_json),
        JsValue::from_str(&store.pin_code),
        JsValue::from_f64(store.paused as f64),
        JsValue::from_str(&store.id),
    ])?
    .run()
    .await?;

    Ok(Json(OwnerStoreResponse {
        store: store.into_owner_dto(),
    }))
}

/// 사장님 매장 일자별 노출·방문 수락·인증·쿠폰 사용 통계 및 전환율 조회 (REQ-B21)
#[utoipa::path(
    get,
    path = "/v1/owner/stores/{id}/stats",
    tag = "Store Owner (PWA /owner)",
    params(
        ("id" = String, Path, description = "통계를 조회할 매장 ID"),
        StoreStatsQuery
    ),
    security(("kakao_user_id" = [])),
    responses(
        (status = 200, description = "최근 7일/30일 일자별 퍼널 통계 및 전환율", body = StoreStatsResponse),
        (status = 400, description = "days 파라미터 오류 (7 또는 30만 허용)", body = crate::error::ErrorResponse),
        (status = 403, description = "본인 소유 매장이 아님", body = crate::error::ErrorResponse),
        (status = 404, description = "존재하지 않는 매장", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn get_owner_store_stats(
    State(state): State<AppState>,
    KakaoUserId(kakao_user_id): KakaoUserId,
    Path(store_id): Path<String>,
    Query(query): Query<StoreStatsQuery>,
) -> Result<Json<StoreStatsResponse>, AppError> {
    let days = query.days.unwrap_or(7);
    if !matches!(days, 7 | 30) {
        return Err(AppError::validation(
            "조회 기간(days)은 7 또는 30만 지정할 수 있습니다",
        ));
    }

    let db = state.db()?;
    let store: Option<StoreRow> = db
        .prepare("SELECT * FROM stores WHERE id = ?1")
        .bind(&[JsValue::from_str(&store_id)])?
        .first(None)
        .await?;

    let store = store.ok_or_else(|| AppError::not_found("매장을 찾을 수 없습니다"))?;
    if store.owner_kakao_id != kakao_user_id {
        return Err(AppError::forbidden("본인 소유의 매장 통계만 조회할 수 있습니다"));
    }

    let dates = recent_kst_dates(now_utc(), days);
    let start_date = dates.first().cloned().unwrap_or_default();
    let end_date = dates.last().cloned().unwrap_or_default();

    // 1. Impressions per date
    let imp_rows: Vec<DateCountRow> = db
        .prepare(
            "SELECT kst_date, COUNT(*) as cnt FROM impressions
             WHERE store_id = ?1 AND kst_date BETWEEN ?2 AND ?3
             GROUP BY kst_date",
        )
        .bind(&[
            JsValue::from_str(&store_id),
            JsValue::from_str(&start_date),
            JsValue::from_str(&end_date),
        ])?
        .all()
        .await?
        .results::<DateCountRow>()?;

    // 2. Visits accepted (all created visits) per date
    let acc_rows: Vec<DateCountRow> = db
        .prepare(
            "SELECT kst_date, COUNT(*) as cnt FROM visits
             WHERE store_id = ?1 AND kst_date BETWEEN ?2 AND ?3
             GROUP BY kst_date",
        )
        .bind(&[
            JsValue::from_str(&store_id),
            JsValue::from_str(&start_date),
            JsValue::from_str(&end_date),
        ])?
        .all()
        .await?
        .results::<DateCountRow>()?;

    // 3. Visits verified per date
    let ver_rows: Vec<DateCountRow> = db
        .prepare(
            "SELECT kst_date, COUNT(*) as cnt FROM visits
             WHERE store_id = ?1 AND status = 'VERIFIED' AND kst_date BETWEEN ?2 AND ?3
             GROUP BY kst_date",
        )
        .bind(&[
            JsValue::from_str(&store_id),
            JsValue::from_str(&start_date),
            JsValue::from_str(&end_date),
        ])?
        .all()
        .await?
        .results::<DateCountRow>()?;

    // 4. Coupons redeemed per date
    let red_rows: Vec<DateCountRow> = db
        .prepare(
            "SELECT redeemed_kst_date as kst_date, COUNT(*) as cnt FROM coupons
             WHERE store_id = ?1 AND status = 'REDEEMED' AND redeemed_kst_date BETWEEN ?2 AND ?3
             GROUP BY redeemed_kst_date",
        )
        .bind(&[
            JsValue::from_str(&store_id),
            JsValue::from_str(&start_date),
            JsValue::from_str(&end_date),
        ])?
        .all()
        .await?
        .results::<DateCountRow>()?;

    let imp_map: HashMap<String, i64> = imp_rows.into_iter().map(|r| (r.kst_date, r.cnt)).collect();
    let acc_map: HashMap<String, i64> = acc_rows.into_iter().map(|r| (r.kst_date, r.cnt)).collect();
    let ver_map: HashMap<String, i64> = ver_rows.into_iter().map(|r| (r.kst_date, r.cnt)).collect();
    let red_map: HashMap<String, i64> = red_rows.into_iter().map(|r| (r.kst_date, r.cnt)).collect();

    let mut daily = Vec::with_capacity(dates.len());
    let mut total_imp = 0;
    let mut total_acc = 0;
    let mut total_ver = 0;
    let mut total_red = 0;

    for d in dates {
        let imp = *imp_map.get(&d).unwrap_or(&0);
        let acc = *acc_map.get(&d).unwrap_or(&0);
        let ver = *ver_map.get(&d).unwrap_or(&0);
        let red = *red_map.get(&d).unwrap_or(&0);

        total_imp += imp;
        total_acc += acc;
        total_ver += ver;
        total_red += red;

        daily.push(DailyStatItem {
            date: d,
            impressions: imp,
            visits_accepted: acc,
            visits_verified: ver,
            coupons_redeemed: red,
        });
    }

    let round4 = |num: i64, den: i64| -> f64 {
        if den <= 0 {
            0.0
        } else {
            ((num as f64 / den as f64) * 10000.0).round() / 10000.0
        }
    };

    let totals = StoreStatsTotals {
        impressions: total_imp,
        visits_accepted: total_acc,
        visits_verified: total_ver,
        coupons_redeemed: total_red,
        accept_rate: round4(total_acc, total_imp),
        verify_rate: round4(total_ver, total_acc),
        redeem_rate: round4(total_red, total_ver),
    };

    Ok(Json(StoreStatsResponse {
        store_id,
        days,
        totals,
        daily,
    }))
}

pub mod db;
pub mod domain;
pub mod error;
pub mod extractors;
pub mod ids;
pub mod routes;
pub mod schemas;
pub mod state;
pub mod time;

use axum::{
    body::Body,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::Serialize;
use tower_service::Service;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi, ToSchema,
};
use worker::{event, Context, Env, HttpRequest, Result as WorkerResult};

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    #[schema(example = "ok")]
    pub status: &'static str,
    #[schema(example = "wayfit-backend")]
    pub service: &'static str,
    #[schema(example = "0.1.0")]
    pub version: &'static str,
}

/// 헬스체크 엔드포인트
#[utoipa::path(
    get,
    path = "/v1/health",
    tag = "System",
    responses(
        (status = 200, description = "서버 정상 동작", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "wayfit-backend",
        version: env!("CARGO_PKG_VERSION"),
    })
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "device_id",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "X-Device-Id",
                    "Pleos 차량용 앱에서 생성한 기기 UUID (예: demo-car-01)",
                ))),
            );
            components.add_security_scheme(
                "kakao_user_id",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "X-Kakao-User-Id",
                    "카카오 OAuth 로그인 회원번호 (예: kakao_user_1001, 사장님 데모: kakao_owner_01)",
                ))),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "WayFit (웨이핏) Backend API",
        version = "0.1.0",
        description = "가는 길에, 웨이핏 — 경로 위 동네 가게 추천 & 차량 센서 기반 방문 인증·모바일 쿠폰 플랫폼 API (Cloudflare Workers + D1, 카카오 OAuth 단일화)"
    ),
    servers(
        (url = "https://wayfit-backend.dannyldj.workers.dev", description = "운영 (Cloudflare Workers)"),
        (url = "http://localhost:8787", description = "로컬 (wrangler dev)")
    ),
    modifiers(&SecurityAddon),
    paths(
        health_check,
        crate::routes::stores::get_nearby_stores,
        crate::routes::impressions::create_impression,
        crate::routes::visits::create_visit,
        crate::routes::visits::cancel_visit,
        crate::routes::visits::verify_visit,
        crate::routes::coupons::get_device_coupons,
        crate::routes::coupons::redeem_coupon,
        crate::routes::pairing::create_pairing_code,
        crate::routes::pairing::get_device_pairing_status,
        crate::routes::pairing::claim_pairing_code,
        crate::routes::me::get_my_devices,
        crate::routes::me::unlink_my_device,
        crate::routes::me::get_my_coupons,
        crate::routes::owner::get_owner_stores,
        crate::routes::owner::update_owner_store,
        crate::routes::owner::get_owner_store_stats,
    ),
    components(
        schemas(
            HealthResponse,
            crate::error::ErrorCode,
            crate::error::ErrorDetail,
            crate::error::ErrorResponse,
            crate::domain::hours::DayHours,
            crate::domain::verify::EvidenceType,
            crate::schemas::stores::StoreCategory,
            crate::schemas::stores::NearbyStoresQuery,
            crate::schemas::stores::NearbyStoreItem,
            crate::schemas::stores::NearbyStoresResponse,
            crate::schemas::stores::CreateImpressionRequest,
            crate::schemas::stores::CreateImpressionResponse,
            crate::schemas::visits::VisitStatus,
            crate::schemas::visits::CreateVisitRequest,
            crate::schemas::visits::VisitSummary,
            crate::schemas::visits::CreateVisitResponse,
            crate::schemas::visits::EvidenceDto,
            crate::schemas::visits::VerifyVisitRequest,
            crate::schemas::visits::VerifyVisitResponse,
            crate::schemas::coupons::CouponStatus,
            crate::schemas::coupons::CouponDto,
            crate::schemas::coupons::CouponListResponse,
            crate::schemas::coupons::RedeemCouponRequest,
            crate::schemas::coupons::RedeemCouponResponse,
            crate::schemas::pairing::CreatePairingCodeResponse,
            crate::schemas::pairing::DevicePairingStatusResponse,
            crate::schemas::pairing::ClaimPairingRequest,
            crate::schemas::pairing::ClaimPairingResponse,
            crate::schemas::pairing::LinkedDeviceItem,
            crate::schemas::pairing::MyDevicesResponse,
            crate::schemas::owner::OwnerStoreDto,
            crate::schemas::owner::OwnerStoresResponse,
            crate::schemas::owner::OwnerStoreResponse,
            crate::schemas::owner::UpdateOwnerStoreRequest,
            crate::schemas::owner::StoreStatsQuery,
            crate::schemas::owner::DailyStatItem,
            crate::schemas::owner::StoreStatsTotals,
            crate::schemas::owner::StoreStatsResponse,
        )
    ),
    tags(
        (name = "System", description = "헬스체크 및 시스템 상태"),
        (name = "Vehicle (Pleos App)", description = "Pleos 차량용 앱 API (X-Device-Id 헤더 사용)"),
        (name = "Driver Wallet (PWA)", description = "운전자 모바일 쿠폰 지갑 API (카카오 OAuth X-Kakao-User-Id 헤더 사용)"),
        (name = "Store Owner (PWA /owner)", description = "소상공인 사장님 웹 API (카카오 OAuth X-Kakao-User-Id 헤더 사용)")
    )
)]
pub struct ApiDoc;

async fn openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

async fn scalar_docs() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="ko">
  <head>
    <title>WayFit Backend API Reference</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>
    <script id="api-reference" data-url="/v1/openapi.json"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>"#,
    )
}

async fn cors_middleware(req: Request<Body>, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        let mut res = StatusCode::NO_CONTENT.into_response();
        add_cors_headers(res.headers_mut());
        return res;
    }
    let mut res = next.run(req).await;
    add_cors_headers(res.headers_mut());
    res
}

fn add_cors_headers(headers: &mut header::HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PATCH, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, Authorization, X-Device-Id, X-Kakao-User-Id"),
    );
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // System & OpenAPI Docs
        .route("/v1/health", get(health_check))
        .route("/v1/openapi.json", get(openapi_json))
        .route("/v1/docs", get(scalar_docs))
        // Vehicle (Pleos App) routes
        .route("/v1/stores/nearby", get(routes::stores::get_nearby_stores))
        .route(
            "/v1/impressions",
            post(routes::impressions::create_impression),
        )
        .route("/v1/visits", post(routes::visits::create_visit))
        .route("/v1/visits/{id}", delete(routes::visits::cancel_visit))
        .route("/v1/visits/{id}/verify", post(routes::visits::verify_visit))
        .route(
            "/v1/devices/me/coupons",
            get(routes::coupons::get_device_coupons),
        )
        .route(
            "/v1/pairing-codes",
            post(routes::pairing::create_pairing_code),
        )
        .route(
            "/v1/devices/me/pairing",
            get(routes::pairing::get_device_pairing_status),
        )
        // Driver Wallet (PWA) routes
        .route(
            "/v1/pairing/claim",
            post(routes::pairing::claim_pairing_code),
        )
        .route("/v1/me/devices", get(routes::me::get_my_devices))
        .route("/v1/me/devices/{id}", delete(routes::me::unlink_my_device))
        .route("/v1/me/coupons", get(routes::me::get_my_coupons))
        .route(
            "/v1/coupons/{id}/redeem",
            post(routes::coupons::redeem_coupon),
        )
        // Store Owner (PWA /owner) routes
        .route("/v1/owner/stores", get(routes::owner::get_owner_stores))
        .route(
            "/v1/owner/stores/{id}",
            patch(routes::owner::update_owner_store),
        )
        .route(
            "/v1/owner/stores/{id}/stats",
            get(routes::owner::get_owner_store_stats),
        )
        .layer(middleware::from_fn(cors_middleware))
        .with_state(state)
}

#[event(fetch)]
pub async fn fetch(req: HttpRequest, env: Env, _ctx: Context) -> WorkerResult<http::Response<Body>> {
    let state = AppState::new(env);
    let mut router = build_router(state);
    Ok(router.call(req).await.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_generates_all_paths() {
        let doc = ApiDoc::openapi();
        let json_str = doc.to_json().expect("OpenAPI JSON serialization");
        assert!(json_str.contains("/v1/stores/nearby"));
        assert!(json_str.contains("/v1/visits/{id}/verify"));
        assert!(json_str.contains("/v1/coupons/{id}/redeem"));
        assert!(json_str.contains("/v1/owner/stores/{id}/stats"));
        assert!(json_str.contains("X-Device-Id"));
        assert!(json_str.contains("X-Kakao-User-Id"));
    }

    #[test]
    fn test_query_params_documented_as_query() {
        let doc: serde_json::Value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let params = |path: &str| doc["paths"][path]["get"]["parameters"].as_array().unwrap().clone();
        let find = |ps: &[serde_json::Value], name: &str| {
            ps.iter().find(|p| p["name"] == name).cloned().unwrap()
        };

        let nearby = params("/v1/stores/nearby");
        for p in &nearby {
            assert_eq!(p["in"], "query", "{}", p["name"]);
        }
        assert_eq!(find(&nearby, "lat")["required"], true);
        assert_eq!(find(&nearby, "lng")["required"], true);
        for optional in ["heading", "destLat", "destLng", "limit"] {
            assert_ne!(find(&nearby, optional)["required"], true, "{}", optional);
        }

        let stats = params("/v1/owner/stores/{id}/stats");
        assert_eq!(find(&stats, "id")["in"], "path");
        assert_eq!(find(&stats, "days")["in"], "query");

        assert!(doc["servers"][0]["url"].as_str().unwrap().contains("workers.dev"));
    }
}

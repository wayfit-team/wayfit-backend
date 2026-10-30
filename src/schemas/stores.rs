use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoreCategory {
    Cafe,
    Food,
    Bakery,
    Etc,
}

impl StoreCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            StoreCategory::Cafe => "CAFE",
            StoreCategory::Food => "FOOD",
            StoreCategory::Bakery => "BAKERY",
            StoreCategory::Etc => "ETC",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "CAFE" => StoreCategory::Cafe,
            "FOOD" => StoreCategory::Food,
            "BAKERY" => StoreCategory::Bakery,
            _ => StoreCategory::Etc,
        }
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct NearbyStoresQuery {
    /// 현재 차량 위도 (WGS84)
    #[param(example = 37.2792)]
    pub lat: f64,
    /// 현재 차량 경도 (WGS84)
    #[param(example = 127.0437)]
    pub lng: f64,
    /// 진행 방향 각도 (0~360, 북쪽=0, 선택)
    #[param(example = 65.0)]
    pub heading: Option<f64>,
    /// 목적지 위도 (선택, 예: 광교호수공원 37.2865)
    #[param(example = 37.2865)]
    pub dest_lat: Option<f64>,
    /// 목적지 경도 (선택, 예: 광교호수공원 127.0650)
    #[param(example = 127.0650)]
    pub dest_lng: Option<f64>,
    /// 반환할 최대 매장 수 (기본 3, 최대 10)
    #[param(example = 3)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NearbyStoreItem {
    #[schema(example = "store_01")]
    pub store_id: String,
    #[schema(example = "호수길 로스터리")]
    pub name: String,
    pub category: StoreCategory,
    #[schema(example = "아메리카노 1잔 무료 증정")]
    pub benefit_text: String,
    #[schema(example = 37.2825)]
    pub lat: f64,
    #[schema(example = 127.0530)]
    pub lng: f64,
    /// 현재 위치로부터의 직선 거리 (m)
    #[schema(example = 890)]
    pub distance_m: i64,
    /// 경로 대비 예상 우회 거리 (m)
    #[schema(example = 45)]
    pub detour_m: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NearbyStoresResponse {
    pub stores: Vec<NearbyStoreItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateImpressionRequest {
    #[schema(example = "store_01")]
    pub store_id: String,
    #[schema(example = "trip_20261105_001")]
    pub trip_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateImpressionResponse {
    pub recorded: bool,
    pub impression_id: String,
}

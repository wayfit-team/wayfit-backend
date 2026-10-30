# WayFit Backend (`wayfit-backend`)

> **"가는 길에, 웨이핏"** — 경로 위 동네 가게를 추천하고, 들르면 차량 데이터로 방문을 인증해 모바일 쿠폰을 발급하는 소상공인 상생 광고 플랫폼 백엔드  
> **스택**: Cloudflare Workers + Rust (`workers-rs` + `axum` + `utoipa`) + Cloudflare D1 (SQLite)

---

## 1. 아키텍처 & 인증 위임 구조 (카카오 OAuth 단일화)

백엔드는 자체 로그인·비밀번호 해시·세션 테이블을 두지 않고, 외부 인증 및 클라이언트 생성 식별자 헤더를 사용해 핵심 비즈니스 로직(경로 코리도 탐색, 차량 센서 방문 인증, 쿠폰 발급/사용)에 집중합니다.

| 클라이언트 | 식별 헤더 | 설명 |
|---|---|---|
| **Pleos 차량 앱 (Kotlin)** | `X-Device-Id: <device_uuid>` | 앱 최초 실행 시 자체 생성한 기기 UUID (데모: `demo-car-01`) |
| **운전자 쿠폰 지갑 (PWA)** | `X-Kakao-User-Id: <kakao_id>` | 카카오 OAuth 로그인 후 전달하는 회원번호 (데모: `kakao_user_1001`) |
| **사장님 웹 (PWA `/owner`)** | `X-Kakao-User-Id: <kakao_id>` | 사장님 카카오 OAuth 회원번호 (`stores.owner_kakao_id` 매칭, 데모: `kakao_owner_01`) |

---

## 2. 팀원 간 API 명세 공유 (`/v1/docs` & `/v1/openapi.json`)

Rust DTO 및 핸들러에 선언된 `utoipa` 매크로로부터 **OpenAPI 3.1 명세와 Scalar 웹 문서 포털이 100% 자동 생성**됩니다.

- **인터랙티브 API 문서 (Scalar UI)**: `GET /v1/docs`
- **OpenAPI 3.1 JSON 스펙**: `GET /v1/openapi.json`
- **PWA 프론트엔드 TypeScript 타입 자동 생성**:
  ```bash
  npx openapi-typescript http://localhost:8787/v1/openapi.json -o src/types/api.d.ts
  ```

---

## 3. 디렉터리 구조

```text
wayfit-backend/
├── Cargo.toml                  # Rust 의존성 (worker, axum, utoipa, serde, chrono, uuid)
├── wrangler.toml               # Cloudflare Workers 및 D1 (wayfit-db) 바인딩 설정
├── migrations/
│   ├── 0001_init.sql           # D1 스키마 버전 관리 (7개 핵심 도메인 테이블 & 인덱스)
│   └── 0002_evidence_gear_park.sql # 방문 증거: 기어 P단·지오펜스 진입 추가 (PARKING_BRAKE_ON → GEAR_PARK)
├── scripts/
│   └── seed.sql                # 아주대 정문 → 광교호수공원 데모 매장 10곳 시드 데이터
└── src/
    ├── lib.rs                  # Worker 엔트리포인트, Axum 라우터, CORS, OpenAPI/Scalar 마운트
    ├── state.rs                # AppState (Cloudflare Env & D1 바인딩 래퍼)
    ├── db.rs                   # D1 테이블 Row 매핑 및 만료(EXPIRED) 변환 로직
    ├── error.rs                # AppError & 통일 JSON 에러 응답 ({ error: { code, message, details } })
    ├── extractors.rs           # DeviceId (X-Device-Id), KakaoUserId (X-Kakao-User-Id) 추출기
    ├── time.rs                 # UTC 타임스탬프 및 KST (UTC+9) 날짜 경계 계산 (NFR-B6)
    ├── ids.rs                  # UUID, 6자리 쿠폰 코드(code6), 8자리 페어링 코드 생성기
    ├── domain/                 # 순수 도메인 로직 (cargo test 단위 테스트 대상)
    │   ├── corridor.rs         # 직선 경로 코리도(1km), 진행률(0~1), 우회 거리, 방향(±60°) 계산
    │   ├── verify.rs           # 방문 증거 판정 상태 머신, 신뢰도 점수(60~100), 시계 오차 보정
    │   └── hours.rs            # KST 요일별 영업시간 판정
    ├── schemas/                # 요청/응답 DTO 및 OpenAPI ToSchema 정의
    │   ├── stores.rs
    │   ├── visits.rs
    │   ├── coupons.rs
    │   ├── pairing.rs
    │   └── owner.rs
    └── routes/                 # 15개 비즈니스 API 핸들러
        ├── stores.rs           # GET /v1/stores/nearby
        ├── impressions.rs      # POST /v1/impressions
        ├── visits.rs           # POST /v1/visits, DELETE /v1/visits/{id}, POST /v1/visits/{id}/verify
        ├── coupons.rs          # GET /v1/devices/me/coupons, POST /v1/coupons/{id}/redeem
        ├── pairing.rs          # POST /v1/pairing-codes, GET /v1/devices/me/pairing, POST /v1/pairing/claim
        ├── me.rs               # GET /v1/me/devices, DELETE /v1/me/devices/{id}, GET /v1/me/coupons
        └── owner.rs            # GET·PATCH /v1/owner/stores, GET /v1/owner/stores/{id}/stats
```

---

## 4. 전체 API 엔드포인트 요약 (15개 비즈니스 API + 시스템 3개)

| 분류 | 메서드 · 경로 | 식별 헤더 | 설명 |
|---|---|---|---|
| **System** | `GET /v1/health` | 없음 | 서버 상태 헬스체크 |
| **System** | `GET /v1/docs` | 없음 | Scalar 인터랙티브 API 문서 포털 |
| **System** | `GET /v1/openapi.json` | 없음 | OpenAPI 3.1 JSON 명세 |
| **차량 앱** | `GET /v1/stores/nearby` | `X-Device-Id` | 현재 위치·목적지 기반 경로상 매장 탐색 (`REQ-B2, B3`) |
| **차량 앱** | `POST /v1/impressions` | `X-Device-Id` | 추천 카드 노출 기록 (트립 내 중복 제거, `REQ-B4`) |
| **차량 앱** | `POST /v1/visits` | `X-Device-Id` | "들를게요" 방문 생성 (`ACCEPTED`, 이전 진행 중 방문 자동 취소, `REQ-B5`) |
| **차량 앱** | `DELETE /v1/visits/{id}` | `X-Device-Id` | 진행 중 방문 취소 (`REQ-B6`) |
| **차량 앱** | `POST /v1/visits/{id}/verify` | `X-Device-Id` | 차량 센서 증거 제출·판정·쿠폰 즉시 발급 (멱등, `REQ-B8~B11`) |
| **차량 앱** | `GET /v1/devices/me/coupons` | `X-Device-Id` | 차량 기기에 귀속된 쿠폰 목록 조회 (`REQ-B12`) |
| **차량 앱** | `POST /v1/pairing-codes` | `X-Device-Id` | 8자리 QR 페어링 코드 발급 (10분 유효, `REQ-B15`) |
| **차량 앱** | `GET /v1/devices/me/pairing` | `X-Device-Id` | 현재 차량에 연결된 카카오 계정(폰) 수 조회 (`REQ-B17`) |
| **운전자 PWA** | `POST /v1/pairing/claim` | `X-Kakao-User-Id` | 8자리 코드 제출 → 내 카카오 계정에 차량 기기 연결 (`REQ-B16`) |
| **운전자 PWA** | `GET /v1/me/devices` | `X-Kakao-User-Id` | 내 계정에 연결된 차량 기기 목록 조회 (`REQ-B18`) |
| **운전자 PWA** | `DELETE /v1/me/devices/{id}` | `X-Kakao-User-Id` | 특정 차량 기기 연결 해제 (`REQ-B18`) |
| **운전자 PWA** | `GET /v1/me/coupons` | `X-Kakao-User-Id` | 연결된 모든 차량의 쿠폰 통합 조회 (`REQ-B12, B14`) |
| **운전자 PWA** | `POST /v1/coupons/{id}/redeem` | `X-Kakao-User-Id` | 매장 4자리 PIN으로 쿠폰 사용 처리 (5회 오류 시 15분 잠금, `REQ-B13`) |
| **사장님 웹** | `GET /v1/owner/stores` | `X-Kakao-User-Id` | 사장님 본인 소유 매장 목록 조회 (`REQ-B20`) |
| **사장님 웹** | `PATCH /v1/owner/stores/{id}` | `X-Kakao-User-Id` | 매장 혜택(40자)·인증 반경(100/150/300m)·PIN·일시중지 수정 (`REQ-B20`) |
| **사장님 웹** | `GET /v1/owner/stores/{id}/stats` | `X-Kakao-User-Id` | 최근 7일/30일 일자별 노출·수락·인증·사용 및 전환율 통계 (`REQ-B21`) |

---

## 5. 로컬 개발 및 배포 가이드

```bash
# 1. Wasm 타겟 추가 (최초 1회)
rustup target add wasm32-unknown-unknown

# 2. 로컬 D1 스키마 마이그레이션 및 데모 시드(아주대→광교호수공원 10곳) 적용
npx wrangler d1 migrations apply wayfit-db --local
npx wrangler d1 execute wayfit-db --local --file=./scripts/seed.sql

# 3. 도메인 로직 단위 테스트 실행
cargo test --lib

# 4. 로컬 개발 서버 실행 (http://localhost:8787/v1/docs)
npx wrangler dev

# 5. Cloudflare 운영(Remote) D1 생성 및 배포
npx wrangler d1 create wayfit-db
# -> 출력된 database_id를 wrangler.toml에 반영 후:
npx wrangler d1 migrations apply wayfit-db --remote
npx wrangler d1 execute wayfit-db --remote --file=./scripts/seed.sql
npx wrangler deploy
```

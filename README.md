# WayFit Backend (`wayfit-backend`)

> **"가는 길에, 웨이핏"** — 경로 위 동네 가게를 추천하고, 들르면 차량 데이터로 방문을 인증해 모바일 쿠폰을 발급하는 소상공인 상생 광고 플랫폼 백엔드  
> **스택**: Cloudflare Workers + Rust (`workers-rs` + `axum` + `utoipa`) + Cloudflare D1 (SQLite)

---

## 1. 아키텍처 & 인증 (카카오 로그인 단일화)

비밀번호는 다루지 않습니다. 사람(운전자·사장님)은 카카오 로그인으로, 차량은 최초 실행 시 발급받은 기기 토큰으로 인증합니다. 클라이언트가 보낸 식별자를 그대로 믿지 않고, 서버가 발급한 토큰을 해시로 조회해 사용자·기기를 결정합니다.

| 클라이언트 | 인증 헤더 | 발급 |
|---|---|---|
| **Pleos 차량 앱 (Kotlin)** | `Authorization: Bearer wfd_…` | 최초 실행 시 `POST /v1/devices` → `deviceId`·`deviceToken` (토큰은 이 응답에서 한 번만 내려옴) |
| **운전자 쿠폰 지갑 (PWA)** | `Authorization: Bearer wfs_…` | 카카오 로그인 → `POST /v1/auth/kakao` → 30일 세션 토큰 |
| **사장님 웹 (PWA `/owner`)** | `Authorization: Bearer wfs_…` | 운전자와 같은 카카오 로그인. 매장은 `stores.owner_kakao_id`(카카오 회원번호)로 매칭 |

### 카카오 로그인 흐름
1. PWA가 `state`를 만들어 `https://kauth.kakao.com/oauth/authorize`로 이동 (`redirect_uri` = PWA의 `/auth/kakao/callback`)
2. 카카오가 PWA 콜백으로 `code`·`state`를 돌려주면 PWA가 `state`를 확인하고 `POST /v1/auth/kakao {code, redirectUri}`
3. 서버가 Client Secret으로 카카오 토큰을 받아 `/v2/user/me`에서 회원번호를 확인한 뒤 자체 세션 토큰 발급. 카카오 토큰은 저장하지 않음
4. 로그아웃은 `POST /v1/auth/logout` (세션 폐기)

- 토큰 원문은 저장하지 않고 SHA-256 해시만 D1에 저장 (`sessions`, `devices`)
- `redirectUri`는 `KAKAO_REDIRECT_URIS` 허용 목록과 정확히 일치해야 함

### 운영 설정
| 항목 | 위치 |
|---|---|
| `KAKAO_REST_API_KEY` | `wrangler.toml` `[vars]` — 카카오 콘솔 앱 키의 REST API 키 |
| `KAKAO_REDIRECT_URIS` | `wrangler.toml` `[vars]` — 카카오 콘솔에 등록한 Redirect URI와 동일하게 (쉼표 구분) |
| `KAKAO_CLIENT_SECRET` | `npx wrangler secret put KAKAO_CLIENT_SECRET` — 카카오 콘솔에서 Client Secret을 **사용함**으로 둘 것 |
| 로컬 | `.dev.vars.example`을 `.dev.vars`로 복사해 채움 |
| 사장님 매장 연결 | `stores.owner_kakao_id`를 사장님의 실제 카카오 회원번호(숫자)로 설정. 시드의 `kakao_owner_01`은 로그인으로 매칭되지 않음 |

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
│   ├── 0002_evidence_gear_park.sql # 방문 증거: 기어 P단·지오펜스 진입 추가 (PARKING_BRAKE_ON → GEAR_PARK)
│   └── 0003_auth_sessions_devices.sql  # 카카오 로그인 세션·차량 기기 토큰 (해시 저장)
├── scripts/
│   └── seed.sql                # 아주대 정문 → 광교호수공원 데모 매장 10곳 시드 데이터
└── src/
    ├── lib.rs                  # Worker 엔트리포인트, Axum 라우터, CORS, OpenAPI/Scalar 마운트
    ├── state.rs                # AppState (Cloudflare Env & D1 바인딩 래퍼)
    ├── db.rs                   # D1 테이블 Row 매핑 및 만료(EXPIRED) 변환 로직
    ├── error.rs                # AppError & 통일 JSON 에러 응답 ({ error: { code, message, details } })
    ├── extractors.rs           # DeviceId (기기 토큰), KakaoUserId (세션 토큰) 추출기, AppJson·AppQuery
    ├── time.rs                 # UTC 타임스탬프 및 KST (UTC+9) 날짜 경계 계산 (NFR-B6)
    ├── ids.rs                  # UUID, 6자리 쿠폰 코드(code6), 8자리 숫자 페어링 코드 생성기
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
    └── routes/                 # 비즈니스 API 핸들러
        ├── auth.rs             # POST /v1/auth/kakao, POST /v1/auth/logout, POST /v1/devices
        ├── stores.rs           # GET /v1/stores/nearby
        ├── impressions.rs      # POST /v1/impressions
        ├── visits.rs           # POST /v1/visits, DELETE /v1/visits/{id}, POST /v1/visits/{id}/verify
        ├── coupons.rs          # GET /v1/devices/me/coupons, POST /v1/coupons/{id}/redeem
        ├── pairing.rs          # POST /v1/pairing-codes, GET /v1/devices/me/pairing, POST /v1/pairing/claim
        ├── me.rs               # GET /v1/me/devices, DELETE /v1/me/devices/{id}, GET /v1/me/coupons
        └── owner.rs            # GET·PATCH /v1/owner/stores, GET /v1/owner/stores/{id}/stats
```

---

## 4. 전체 API 엔드포인트 요약 (비즈니스 API 18개 + 시스템 3개)

| 분류 | 메서드 · 경로 | 인증 | 설명 |
|---|---|---|---|
| **System** | `GET /v1/health` | 없음 | 서버 상태 헬스체크 |
| **System** | `GET /v1/docs` | 없음 | Scalar 인터랙티브 API 문서 포털 |
| **System** | `GET /v1/openapi.json` | 없음 | OpenAPI 3.1 JSON 명세 |
| **Auth** | `POST /v1/auth/kakao` | 없음 | 카카오 인가 코드 → 30일 세션 토큰 발급 |
| **Auth** | `POST /v1/auth/logout` | 세션 토큰 | 현재 세션 폐기 |
| **차량 앱** | `POST /v1/devices` | 없음 | 기기 등록 → 기기 ID·기기 토큰 발급 (최초 1회) |
| **차량 앱** | `GET /v1/stores/nearby` | 기기 토큰 | 현재 위치·목적지 기반 경로상 매장 탐색 (`REQ-B2, B3`) |
| **차량 앱** | `POST /v1/impressions` | 기기 토큰 | 추천 카드 노출 기록 (트립 내 중복 제거, `REQ-B4`) |
| **차량 앱** | `POST /v1/visits` | 기기 토큰 | "들를게요" 방문 생성 (`ACCEPTED`, 이전 진행 중 방문 자동 취소, `REQ-B5`) |
| **차량 앱** | `DELETE /v1/visits/{id}` | 기기 토큰 | 진행 중 방문 취소 (`REQ-B6`) |
| **차량 앱** | `POST /v1/visits/{id}/verify` | 기기 토큰 | 차량 센서 증거(위치·지오펜스 진입·기어 P단 등) 제출·판정·쿠폰 즉시 발급 (멱등, `REQ-B8~B11`) |
| **차량 앱** | `GET /v1/devices/me/coupons` | 기기 토큰 | 차량 기기에 귀속된 쿠폰 목록 조회 (`REQ-B12`) |
| **차량 앱** | `POST /v1/pairing-codes` | 기기 토큰 | 8자리 숫자 QR 페어링 코드 발급 (10분 유효, `REQ-B15`) |
| **차량 앱** | `GET /v1/devices/me/pairing` | 기기 토큰 | 현재 차량에 연결된 카카오 계정(폰) 수 조회 (`REQ-B17`) |
| **운전자 PWA** | `POST /v1/pairing/claim` | 세션 토큰 | 8자리 숫자 코드 제출 → 내 카카오 계정에 차량 기기 연결 (`REQ-B16`) |
| **운전자 PWA** | `GET /v1/me/devices` | 세션 토큰 | 내 계정에 연결된 차량 기기 목록 조회 (`REQ-B18`) |
| **운전자 PWA** | `DELETE /v1/me/devices/{id}` | 세션 토큰 | 특정 차량 기기 연결 해제 (`REQ-B18`) |
| **운전자 PWA** | `GET /v1/me/coupons` | 세션 토큰 | 연결된 모든 차량의 쿠폰 통합 조회 (`REQ-B12, B14`) |
| **운전자 PWA** | `POST /v1/coupons/{id}/redeem` | 세션 토큰 | 매장 4자리 PIN으로 쿠폰 사용 처리 (5회 오류 시 15분 잠금, `REQ-B13`) |
| **사장님 웹** | `GET /v1/owner/stores` | 세션 토큰 | 사장님 본인 소유 매장 목록 조회 (`REQ-B20`) |
| **사장님 웹** | `PATCH /v1/owner/stores/{id}` | 세션 토큰 | 매장 혜택(40자)·인증 반경(100/150/300m)·PIN·일시중지 수정 (`REQ-B20`) |
| **사장님 웹** | `GET /v1/owner/stores/{id}/stats` | 세션 토큰 | 최근 7일/30일 일자별 노출·수락·인증·사용 및 전환율 통계 (`REQ-B21`) |

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

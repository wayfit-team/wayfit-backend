-- 카카오 로그인 세션 · 차량 기기 토큰 (헤더 값을 그대로 믿던 X-Kakao-User-Id / X-Device-Id 대체)
-- 토큰 원문은 저장하지 않고 SHA-256 해시만 저장한다

-- 1. 카카오 로그인 세션 (운전자 쿠폰 지갑 · 사장님 웹 공통)
CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY,          -- 'wfs_' 세션 토큰의 SHA-256 (hex)
  kakao_user_id TEXT NOT NULL,          -- 카카오 회원번호 (서버가 카카오 API로 확인한 값)
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE INDEX idx_sessions_kakao_user ON sessions(kakao_user_id);

-- 2. 차량 기기 (Pleos 앱 최초 실행 시 등록)
CREATE TABLE devices (
  id TEXT PRIMARY KEY,                  -- 서버가 발급한 기기 ID ('dev_...')
  token_hash TEXT NOT NULL UNIQUE,      -- 'wfd_' 기기 토큰의 SHA-256 (hex)
  created_at TEXT NOT NULL
);

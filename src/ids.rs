use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Prefix of Kakao login session tokens issued to the PWA
pub const SESSION_TOKEN_PREFIX: &str = "wfs_";
/// Prefix of vehicle device tokens issued to the Pleos app
pub const DEVICE_TOKEN_PREFIX: &str = "wfd_";

/// Generate a prefixed UUID (e.g., "visit_...", "cpn_...", "imp_...")
pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

/// Generate a 6-digit numeric display code for coupons (REQ-B11)
pub fn generate_coupon_code6() -> String {
    let mut buf = [0u8; 4];
    let _ = getrandom::getrandom(&mut buf);
    let num = u32::from_le_bytes(buf) % 1_000_000;
    format!("{:06}", num)
}

/// Generate an 8-digit numeric pairing code (REQ-B15).
/// 차 화면의 코드를 폰 숫자 키패드로 옮겨 입력하기 쉽도록 숫자만 쓴다.
pub fn generate_pairing_code8() -> String {
    let mut buf = [0u8; 8];
    // 난수 생성 실패 시 0으로 채워진 예측 가능한 코드가 나오지 않도록 실패를 그대로 드러낸다
    getrandom::getrandom(&mut buf).expect("secure random source unavailable");
    let num = u64::from_le_bytes(buf) % 100_000_000;
    format!("{:08}", num)
}

/// Generate an opaque bearer token: prefix + 256-bit random hex (e.g., "wfs_3f9a...")
pub fn new_token(prefix: &str) -> String {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).expect("secure random source unavailable");
    format!("{}{}", prefix, to_hex(&buf))
}

/// SHA-256 hex digest stored in place of the token (tokens are 256-bit random, so no slow hash needed)
pub fn token_hash(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generators() {
        let id = new_id("visit");
        assert!(id.starts_with("visit_"));
        let code6 = generate_coupon_code6();
        assert_eq!(code6.len(), 6);
        assert!(code6.chars().all(|c| c.is_ascii_digit()));
        let code8 = generate_pairing_code8();
        assert_eq!(code8.len(), 8);
        assert!(code8.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_tokens() {
        let token = new_token(SESSION_TOKEN_PREFIX);
        assert!(token.starts_with("wfs_"));
        assert_eq!(token.len(), 4 + 64);
        assert_ne!(token, new_token(SESSION_TOKEN_PREFIX));

        let hash = token_hash(&token);
        assert_eq!(hash.len(), 64);
        assert_eq!(hash, token_hash(&token));
        assert_ne!(hash, token_hash(&new_token(SESSION_TOKEN_PREFIX)));
        // SHA-256("abc")
        assert_eq!(
            token_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

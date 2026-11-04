use chrono::{DateTime, NaiveDateTime, SubsecRound, Utc};

/// Current UTC time truncated to microseconds, so a value survives a
/// database round trip and RFC 3339 formatting without changing.
pub fn now() -> NaiveDateTime {
    Utc::now().naive_utc().trunc_subsecs(6)
}

/// `2026-11-04T08:30:00.123456Z` — the protocol's canonical timestamp form.
pub fn fmt_date(date: &NaiveDateTime) -> String {
    date.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

pub fn fmt_opt_date(date: &Option<NaiveDateTime>) -> Option<String> {
    date.as_ref().map(fmt_date)
}

/// Accepts any RFC 3339 timestamp and normalises it to naive UTC.
pub fn parse_date(raw: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(raw.trim()).ok().map(|d| d.naive_utc().trunc_subsecs(6))
}

pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.len() <= 254
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Validates the shape of a protocol EncString (`1.<b64 iv>.<b64 ciphertext>`).
/// The server cannot decrypt it, but rejecting garbage early keeps clients honest.
pub fn is_enc_string(value: &str, max_len: usize) -> bool {
    use data_encoding::BASE64;
    if value.len() > max_len {
        return false;
    }
    let mut parts = value.split('.');
    let (Some("1"), Some(iv), Some(ct), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let iv_ok = BASE64.decode(iv.as_bytes()).map(|b| b.len() == 12).unwrap_or(false);
    // AES-GCM output always carries a 16-byte tag; only check the encoding cheaply for large values.
    let ct_ok = ct.len() >= 24
        && ct.len() % 4 == 0
        && ct.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=');
    iv_ok && ct_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_round_trip_is_exact() {
        let now = now();
        assert_eq!(parse_date(&fmt_date(&now)), Some(now));
    }

    #[test]
    fn enc_string_shape() {
        assert!(is_enc_string("1.AAAAAAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAAAA", 100));
        assert!(!is_enc_string("2.AAAAAAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAAAA", 100));
        assert!(!is_enc_string("1.AAAA.AAAAAAAAAAAAAAAAAAAAAAAA", 100));
        assert!(!is_enc_string("plain text", 100));
    }

    #[test]
    fn email_validation() {
        assert!(is_valid_email("me@example.com"));
        assert!(!is_valid_email("me@localhost"));
        assert!(!is_valid_email("no-at-sign"));
        assert!(!is_valid_email("me @example.com"));
    }
}

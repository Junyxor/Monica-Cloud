//! Token issuing and request guards (vaultwarden's `auth.rs` counterpart).

use std::{
    net::{IpAddr, SocketAddr},
    sync::OnceLock,
};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::{
    CONFIG,
    api::AppState,
    db::models::{Device, User},
    err,
    error::{ApiResult, Error},
};

const ISSUER: &str = "monica-cloud";
const TYP_ACCESS: &str = "access";
const TYP_SEND_FILE: &str = "send-file";
const SEND_FILE_TOKEN_SECS: i64 = 300;

static KEYS: OnceLock<(EncodingKey, DecodingKey)> = OnceLock::new();

/// Load the HMAC signing key from `DATA_FOLDER/jwt.key`, creating it on first start.
pub fn init_keys() -> std::io::Result<()> {
    let path = CONFIG.data_folder.join("jwt.key");
    let secret = match std::fs::read(&path) {
        Ok(bytes) if bytes.len() >= 32 => bytes,
        _ => {
            let bytes = data_encoding::HEXLOWER.encode(&crate::crypto::random_bytes::<64>()).into_bytes();
            write_private(&path, &bytes)?;
            bytes
        }
    };
    let _ = KEYS.set((EncodingKey::from_secret(&secret), DecodingKey::from_secret(&secret)));
    Ok(())
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

fn keys() -> &'static (EncodingKey, DecodingKey) {
    KEYS.get().expect("auth::init_keys must run before serving requests")
}

fn validation() -> Validation {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[ISSUER]);
    validation.leeway = 30;
    validation
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessClaims {
    iss: String,
    typ: String,
    sub: String,
    dev: String,
    stamp: String,
    iat: i64,
    exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SendFileClaims {
    iss: String,
    typ: String,
    sub: String,
    iat: i64,
    exp: i64,
}

pub fn issue_access_token(user: &User, device: &Device) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = AccessClaims {
        iss: ISSUER.into(),
        typ: TYP_ACCESS.into(),
        sub: user.uuid.clone(),
        dev: device.uuid.clone(),
        stamp: user.security_stamp.clone(),
        iat: now,
        exp: now + CONFIG.access_token_ttl_secs,
    };
    encode(&Header::new(Algorithm::HS256), &claims, &keys().0).expect("HS256 signing cannot fail")
}

pub fn issue_send_file_token(send_uuid: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = SendFileClaims {
        iss: ISSUER.into(),
        typ: TYP_SEND_FILE.into(),
        sub: send_uuid.into(),
        iat: now,
        exp: now + SEND_FILE_TOKEN_SECS,
    };
    encode(&Header::new(Algorithm::HS256), &claims, &keys().0).expect("HS256 signing cannot fail")
}

/// Returns the Send id the download token was issued for.
pub fn verify_send_file_token(token: &str) -> ApiResult<String> {
    let data = decode::<SendFileClaims>(token, &keys().1, &validation())
        .map_err(|_| Error::Unauthorized("download link expired".into()))?;
    if data.claims.typ != TYP_SEND_FILE {
        err!(Unauthorized, "invalid download token");
    }
    Ok(data.claims.sub)
}

/// The caller's IP: the configured proxy header when present, else the socket peer.
pub struct ClientIp(pub String);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(name) = &CONFIG.ip_header
            && let Some(value) = parts.headers.get(name.as_str()).and_then(|v| v.to_str().ok())
            && let Some(ip) = value.split(',').next().and_then(|first| first.trim().parse::<IpAddr>().ok())
        {
            return Ok(ClientIp(ip.to_string()));
        }
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|ConnectInfo(addr)| addr.ip().to_string());
        Ok(ClientIp(peer.unwrap_or_default()))
    }
}

/// An authenticated user with a live device session.
pub struct Auth {
    pub user: User,
    pub device_uuid: String,
    pub device_name: String,
    pub ip: String,
}

impl FromRequestParts<AppState> for Auth {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let ClientIp(ip) = ClientIp::from_request_parts(parts, state).await?;
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| Error::Unauthorized("missing bearer token".into()))?
            .trim()
            .to_string();

        let claims = decode::<AccessClaims>(&token, &keys().1, &validation())
            .map_err(|_| Error::Unauthorized("invalid or expired token".into()))?
            .claims;
        if claims.typ != TYP_ACCESS {
            err!(Unauthorized, "invalid token type");
        }

        let (user, device_name) = state
            .db
            .run(move |conn| {
                let user = User::find(conn, &claims.sub)?;
                let device = Device::find(conn, &claims.dev, &claims.sub)?;
                Ok((user.map(|u| (u, claims.stamp, claims.dev)), device.map(|d| d.name)))
            })
            .await?;

        let Some((user, stamp, device_uuid)) = user else {
            err!(Unauthorized, "account no longer exists");
        };
        let Some(device_name) = device_name.filter(|_| user.security_stamp == stamp) else {
            err!(Unauthorized, "session was revoked, please log in again");
        };
        if !user.enabled {
            err!(Forbidden, "account is disabled");
        }
        Ok(Auth {
            user,
            device_uuid,
            device_name,
            ip,
        })
    }
}

/// An authenticated owner or admin.
pub struct AdminAuth(pub Auth);

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth = Auth::from_request_parts(parts, state).await?;
        if !auth.user.is_admin() {
            err!(Forbidden, "administrator role required");
        }
        Ok(AdminAuth(auth))
    }
}

//! A single error type for every handler. It renders the JSON envelope from
//! the protocol (`{ "error": "...", "message": "..." }`) and never leaks
//! internal details to clients.

use axum::{
    Json,
    extract::{FromRequest, Request, rejection::JsonRejection},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[derive(Debug)]
pub enum Error {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    /// Optimistic concurrency failure; carries the server's current object.
    Conflict(String, Option<Value>),
    PayloadTooLarge(String),
    RateLimited,
    Internal(String),
}

pub type ApiResult<T> = Result<T, Error>;
pub type JsonResult = ApiResult<Json<Value>>;

/// `err!(BadRequest, "message")` returns early with the given error.
#[macro_export]
macro_rules! err {
    (Conflict, $msg:expr, $current:expr) => {
        return Err($crate::error::Error::Conflict($msg.into(), Some($current)))
    };
    (RateLimited) => {
        return Err($crate::error::Error::RateLimited)
    };
    ($kind:ident, $msg:expr) => {
        return Err($crate::error::Error::$kind($msg.into()))
    };
}

impl Error {
    fn parts(&self) -> (StatusCode, &'static str, String) {
        match self {
            Error::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m.clone()),
            Error::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m.clone()),
            Error::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m.clone()),
            Error::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m.clone()),
            Error::Conflict(m, _) => (StatusCode::CONFLICT, "conflict", m.clone()),
            Error::PayloadTooLarge(m) => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large", m.clone()),
            Error::RateLimited => {
                (StatusCode::TOO_MANY_REQUESTS, "rate_limited", "too many attempts, try again later".into())
            }
            Error::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal server error".into()),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Internal(m) => write!(f, "internal: {m}"),
            other => {
                let (status, code, message) = other.parts();
                write!(f, "{status} {code}: {message}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        if let Error::Internal(detail) = &self {
            tracing::error!("{detail}");
        }
        let (status, code, message) = self.parts();
        let mut body = json!({ "error": code, "message": message });
        if let Error::Conflict(_, Some(current)) = &self {
            body["current"] = current.clone();
        }
        let mut response = (status, Json(body)).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer realm=\"Monica Cloud\""));
        }
        response
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Self {
        match e {
            diesel::result::Error::NotFound => Error::NotFound("not found".into()),
            other => Error::Internal(format!("database: {other}")),
        }
    }
}

impl From<diesel::r2d2::PoolError> for Error {
    fn from(e: diesel::r2d2::PoolError) -> Self {
        Error::Internal(format!("database pool: {e}"))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Internal(format!("io: {e}"))
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(e: tokio::task::JoinError) -> Self {
        Error::Internal(format!("task: {e}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Internal(format!("json: {e}"))
    }
}

/// `Json<T>` with rejections rendered in the protocol's error envelope.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ApiJson(value)),
            Err(rejection) => Err(match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => Error::PayloadTooLarge("request body is too large".into()),
                _ => Error::BadRequest(json_rejection_message(&rejection)),
            }),
        }
    }
}

fn json_rejection_message(rejection: &JsonRejection) -> String {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => "expected Content-Type: application/json".into(),
        other => other.body_text(),
    }
}

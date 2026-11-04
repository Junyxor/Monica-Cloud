mod admin;
mod core;
mod identity;
mod web;

use std::{sync::Arc, time::Instant};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, header},
};
use tower_http::{set_header::SetResponseHeaderLayer, trace::TraceLayer};

use crate::{CONFIG, db::Db, error::Error, ratelimit::RateLimiter, storage::Storage};

pub use self::core::purge_expired_sends;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub storage: Storage,
    pub login_limiter: Arc<RateLimiter>,
    pub started_at: Instant,
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .merge(identity::routes())
        .merge(core::routes())
        .merge(admin::routes())
        .fallback(|| async { Error::NotFound("unknown API endpoint".into()) })
        // JSON bodies only; blob uploads stream through `Body` and enforce their own limits.
        .layer(DefaultBodyLimit::max(CONFIG.json_limit_mb * 1024 * 1024));

    Router::new()
        .nest("/api/v1", api)
        .merge(web::routes())
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("same-origin")))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

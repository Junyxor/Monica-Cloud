//! Full and incremental sync (protocol §4.2).

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::Duration;
use serde::Deserialize;
use serde_json::json;

use crate::{
    CONFIG,
    api::AppState,
    auth::Auth,
    db::models::{Folder, Item, Send, Tombstone, User},
    error::{Error, JsonResult},
    util::{fmt_date, now, parse_date},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/sync", get(sync))
}

#[derive(Deserialize)]
struct SyncQuery {
    since: Option<String>,
}

async fn sync(State(state): State<AppState>, auth: Auth, Query(query): Query<SyncQuery>) -> JsonResult {
    let requested = match query.since.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => {
            Some(parse_date(raw).ok_or_else(|| Error::BadRequest("since must be an RFC 3339 timestamp".into()))?)
        }
        None => None,
    };
    let uuid = auth.user.uuid;

    let body = state
        .db
        .read(move |conn| {
            // Re-read inside the snapshot so the revision matches the rows we return.
            let user =
                User::find(conn, &uuid)?.ok_or_else(|| Error::Unauthorized("account no longer exists".into()))?;
            let tombstone_horizon = now() - Duration::days(CONFIG.tombstone_retention_days);
            // Fall back to a full sync when tombstones may have been purged, or when the
            // client's cursor is from the future (restored server, different account).
            let since = requested.filter(|s| *s >= tombstone_horizon && *s <= user.revision_date);

            let folders = Folder::changed_since(conn, &uuid, since)?;
            let items = Item::changed_since_json(conn, &uuid, since)?;
            let sends = Send::changed_since(conn, &uuid, since)?;
            let deleted = match since {
                Some(since) => Tombstone::since(conn, &uuid, since)?,
                None => Vec::new(),
            };

            Ok(json!({
                "full": since.is_none(),
                "revisionDate": fmt_date(&user.revision_date),
                "profile": user.profile_json(),
                "folders": folders.iter().map(Folder::to_json).collect::<Vec<_>>(),
                "items": items,
                "sends": sends.iter().map(Send::to_json).collect::<Vec<_>>(),
                "deleted": deleted.iter().map(Tombstone::to_json).collect::<Vec<_>>(),
            }))
        })
        .await?;
    Ok(Json(body))
}

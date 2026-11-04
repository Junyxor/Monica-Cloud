use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use serde_json::json;

use crate::{
    api::AppState,
    auth::Auth,
    db::models::{Device, Event},
    error::{Error, JsonResult},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/devices", get(list_devices)).route("/devices/{id}", delete(revoke_device))
}

async fn list_devices(State(state): State<AppState>, auth: Auth) -> JsonResult {
    let uuid = auth.user.uuid;
    let devices = state.db.run(move |conn| Device::for_user(conn, &uuid)).await?;
    let current = auth.device_uuid;
    Ok(Json(json!({ "data": devices.iter().map(|d| d.to_json(Some(&current))).collect::<Vec<_>>() })))
}

/// Revoking deletes the session row, which also invalidates its access token immediately.
async fn revoke_device(State(state): State<AppState>, auth: Auth, Path(id): Path<String>) -> JsonResult {
    state
        .db
        .write(move |conn| {
            let device =
                Device::find(conn, &id, &auth.user.uuid)?.ok_or_else(|| Error::NotFound("device not found".into()))?;
            Device::delete(conn, &device.uuid)?;
            Event::new(Some(&auth.user.uuid), &auth.user.email, "device.revoke", &device.name, &auth.ip)
                .device(&auth.device_name)
                .insert(conn)?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({})))
}

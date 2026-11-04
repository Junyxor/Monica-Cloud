use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, put},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    api::AppState,
    auth::Auth,
    db::models::{Event, Folder, Item, OBJECT_FOLDER, Tombstone, User},
    err,
    error::{ApiJson, Error, JsonResult},
    util::is_enc_string,
};

pub const FOLDER_NAME_MAX: usize = 4096;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/folders", get(list_folders).post(create_folder))
        .route("/folders/{id}", put(update_folder).delete(delete_folder))
}

#[derive(Deserialize)]
struct FolderData {
    name: String,
}

impl FolderData {
    fn validate(&self) -> crate::error::ApiResult<()> {
        if !is_enc_string(&self.name, FOLDER_NAME_MAX) {
            err!(BadRequest, "name is not a v1 EncString");
        }
        Ok(())
    }
}

async fn list_folders(State(state): State<AppState>, auth: Auth) -> JsonResult {
    let uuid = auth.user.uuid;
    let folders = state.db.run(move |conn| Folder::changed_since(conn, &uuid, None)).await?;
    Ok(Json(json!({ "data": folders.iter().map(Folder::to_json).collect::<Vec<_>>() })))
}

async fn create_folder(State(state): State<AppState>, auth: Auth, ApiJson(data): ApiJson<FolderData>) -> JsonResult {
    data.validate()?;
    let folder = state
        .db
        .write(move |conn| {
            let revision = User::next_revision(conn, &auth.user.uuid)?;
            let folder = Folder::new(auth.user.uuid.clone(), data.name, revision);
            folder.insert(conn)?;
            Event::new(Some(&auth.user.uuid), &auth.user.email, "folder.create", &folder.uuid, &auth.ip)
                .device(&auth.device_name)
                .insert(conn)?;
            Ok(folder)
        })
        .await?;
    Ok(Json(folder.to_json()))
}

async fn update_folder(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<String>,
    ApiJson(data): ApiJson<FolderData>,
) -> JsonResult {
    data.validate()?;
    let folder = state
        .db
        .write(move |conn| {
            let mut folder =
                Folder::find(conn, &id, &auth.user.uuid)?.ok_or_else(|| Error::NotFound("folder not found".into()))?;
            folder.name = data.name;
            folder.revision_date = User::next_revision(conn, &auth.user.uuid)?;
            folder.save(conn)?;
            Ok(folder)
        })
        .await?;
    Ok(Json(folder.to_json()))
}

async fn delete_folder(State(state): State<AppState>, auth: Auth, Path(id): Path<String>) -> JsonResult {
    state
        .db
        .write(move |conn| {
            let folder =
                Folder::find(conn, &id, &auth.user.uuid)?.ok_or_else(|| Error::NotFound("folder not found".into()))?;
            let revision = User::next_revision(conn, &auth.user.uuid)?;
            // Detach items explicitly so their revision moves and incremental sync picks them up.
            for mut item in Item::in_folder(conn, &folder.uuid)? {
                item.folder_uuid = None;
                item.revision_date = revision;
                item.save(conn)?;
            }
            Folder::delete(conn, &folder.uuid)?;
            Tombstone::record(conn, &auth.user.uuid, OBJECT_FOLDER, &folder.uuid, revision)?;
            Event::new(Some(&auth.user.uuid), &auth.user.email, "folder.delete", &folder.uuid, &auth.ip)
                .device(&auth.device_name)
                .insert(conn)?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({})))
}

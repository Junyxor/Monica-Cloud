//! Item CRUD, bulk import and bulk actions (protocol §4.3).

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    api::AppState,
    auth::Auth,
    db::{
        Conn,
        models::{Attachment, Event, Folder, ITEM_TYPE_MAX, ITEM_TYPE_MIN, Item, OBJECT_ITEM, Tombstone, User},
    },
    err,
    error::{ApiJson, ApiResult, Error, JsonResult},
    util::{is_enc_string, parse_date},
};

const ITEM_KEY_MAX: usize = 512;
const ITEM_DATA_MAX: usize = 4 * 1024 * 1024;
const IMPORT_ITEMS_MAX: usize = 10_000;
const IMPORT_FOLDERS_MAX: usize = 2_000;
const BULK_MAX: usize = 1_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/items", post(create_item))
        .route("/items/import", post(import_items))
        .route("/items/bulk", post(bulk_action))
        .route("/items/{id}", get(get_item).put(update_item).delete(delete_item))
        .route("/items/{id}/trash", post(trash_item))
        .route("/items/{id}/restore", post(restore_item))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemData {
    #[serde(rename = "type")]
    atype: i32,
    folder_id: Option<String>,
    #[serde(default)]
    favorite: bool,
    key: String,
    data: String,
    #[serde(default)]
    archived: bool,
    last_known_revision_date: Option<String>,
    /// Import only: client-side reference echoed back to map local ids.
    #[serde(rename = "ref")]
    reference: Option<String>,
    /// Import only: refers to a folder created in the same import.
    folder_ref: Option<String>,
}

impl ItemData {
    fn validate(&self) -> ApiResult<()> {
        if !(ITEM_TYPE_MIN..=ITEM_TYPE_MAX).contains(&self.atype) {
            err!(BadRequest, format!("type must be between {ITEM_TYPE_MIN} and {ITEM_TYPE_MAX}"));
        }
        if !is_enc_string(&self.key, ITEM_KEY_MAX) {
            err!(BadRequest, "key is not a v1 EncString");
        }
        if !is_enc_string(&self.data, ITEM_DATA_MAX) {
            err!(BadRequest, "data is not a v1 EncString or is larger than 4 MiB");
        }
        Ok(())
    }

    /// Copy client fields onto `item`. `folder` must already be ownership-checked.
    fn apply(&self, item: &mut Item, folder: Option<String>, revision: NaiveDateTime) {
        item.atype = self.atype;
        item.folder_uuid = folder;
        item.favorite = self.favorite;
        item.akey = self.key.clone();
        item.data = self.data.clone();
        item.archived_at = match (self.archived, item.archived_at) {
            (true, Some(existing)) => Some(existing),
            (true, None) => Some(revision),
            (false, _) => None,
        };
        item.revision_date = revision;
    }
}

fn check_folder(conn: &mut Conn, folder_id: Option<&String>, user_uuid: &str) -> ApiResult<Option<String>> {
    match folder_id {
        Some(id) => match Folder::find(conn, id, user_uuid)? {
            Some(folder) => Ok(Some(folder.uuid)),
            None => err!(BadRequest, "folder does not exist"),
        },
        None => Ok(None),
    }
}

fn find_item(conn: &mut Conn, id: &str, user_uuid: &str) -> ApiResult<Item> {
    Item::find(conn, id, user_uuid)?.ok_or_else(|| Error::NotFound("item not found".into()))
}

fn event(conn: &mut Conn, auth: &AuthInfo, action: &str, target: &str) -> ApiResult<()> {
    Event::new(Some(&auth.user_uuid), &auth.email, action, target, &auth.ip).device(&auth.device_name).insert(conn)
}

/// The parts of [`Auth`] that closures need to own.
struct AuthInfo {
    user_uuid: String,
    email: String,
    ip: String,
    device_name: String,
}

impl From<Auth> for AuthInfo {
    fn from(auth: Auth) -> Self {
        Self {
            user_uuid: auth.user.uuid,
            email: auth.user.email,
            ip: auth.ip,
            device_name: auth.device_name,
        }
    }
}

async fn create_item(State(state): State<AppState>, auth: Auth, ApiJson(data): ApiJson<ItemData>) -> JsonResult {
    data.validate()?;
    let auth = AuthInfo::from(auth);
    let item = state
        .db
        .write(move |conn| {
            let folder = check_folder(conn, data.folder_id.as_ref(), &auth.user_uuid)?;
            let revision = User::next_revision(conn, &auth.user_uuid)?;
            let mut item = Item::new(auth.user_uuid.clone(), revision);
            data.apply(&mut item, folder, revision);
            item.insert(conn)?;
            event(conn, &auth, "item.create", &item.uuid)?;
            Ok(item.to_json(&[]))
        })
        .await?;
    Ok(Json(item))
}

#[derive(Deserialize)]
struct ImportFolder {
    #[serde(rename = "ref")]
    reference: String,
    name: String,
}

#[derive(Deserialize)]
struct ImportData {
    #[serde(default)]
    folders: Vec<ImportFolder>,
    items: Vec<ItemData>,
}

/// First upload from a client: many folders and items in one transaction and one revision.
async fn import_items(State(state): State<AppState>, auth: Auth, ApiJson(data): ApiJson<ImportData>) -> JsonResult {
    if data.items.len() > IMPORT_ITEMS_MAX || data.folders.len() > IMPORT_FOLDERS_MAX {
        err!(
            BadRequest,
            format!("import is limited to {IMPORT_ITEMS_MAX} items and {IMPORT_FOLDERS_MAX} folders per request")
        );
    }
    for folder in &data.folders {
        if !is_enc_string(&folder.name, super::folders::FOLDER_NAME_MAX) {
            err!(BadRequest, "folder name is not a v1 EncString");
        }
    }
    for item in &data.items {
        item.validate()?;
    }

    let auth = AuthInfo::from(auth);
    let body = state
        .db
        .write(move |conn| {
            let revision = User::next_revision(conn, &auth.user_uuid)?;
            let mut folder_ids: HashMap<String, String> = HashMap::new();
            let mut folders_out = Vec::with_capacity(data.folders.len());
            for import in data.folders {
                let folder = Folder::new(auth.user_uuid.clone(), import.name, revision);
                folder.insert(conn)?;
                folders_out.push(json!({ "ref": import.reference, "folder": folder.to_json() }));
                folder_ids.insert(import.reference, folder.uuid);
            }

            let mut items_out = Vec::with_capacity(data.items.len());
            for import in &data.items {
                let folder = match &import.folder_ref {
                    Some(reference) => Some(
                        folder_ids
                            .get(reference)
                            .cloned()
                            .ok_or_else(|| Error::BadRequest(format!("unknown folderRef {reference:?}")))?,
                    ),
                    None => check_folder(conn, import.folder_id.as_ref(), &auth.user_uuid)?,
                };
                let mut item = Item::new(auth.user_uuid.clone(), revision);
                import.apply(&mut item, folder, revision);
                item.insert(conn)?;
                let mut out = item.to_json(&[]);
                out["ref"] = json!(import.reference);
                items_out.push(out);
            }
            event(conn, &auth, "items.import", &format!("{} items", items_out.len()))?;
            Ok(json!({ "folders": folders_out, "items": items_out }))
        })
        .await?;
    Ok(Json(body))
}

async fn get_item(State(state): State<AppState>, auth: Auth, Path(id): Path<String>) -> JsonResult {
    let user_uuid = auth.user.uuid;
    let item = state.db.read(move |conn| find_item(conn, &id, &user_uuid)?.to_json_full(conn)).await?;
    Ok(Json(item))
}

async fn update_item(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<String>,
    ApiJson(data): ApiJson<ItemData>,
) -> JsonResult {
    data.validate()?;
    let last_known = match &data.last_known_revision_date {
        Some(raw) => Some(parse_date(raw).ok_or_else(|| Error::BadRequest("invalid lastKnownRevisionDate".into()))?),
        None => None,
    };
    let auth = AuthInfo::from(auth);
    let item = state
        .db
        .write(move |conn| {
            let mut item = find_item(conn, &id, &auth.user_uuid)?;
            if let Some(last_known) = last_known
                && last_known != item.revision_date
            {
                err!(Conflict, "item was modified on another device", item.to_json_full(conn)?);
            }
            let folder = check_folder(conn, data.folder_id.as_ref(), &auth.user_uuid)?;
            let revision = User::next_revision(conn, &auth.user_uuid)?;
            data.apply(&mut item, folder, revision);
            item.save(conn)?;
            event(conn, &auth, "item.update", &item.uuid)?;
            item.to_json_full(conn)
        })
        .await?;
    Ok(Json(item))
}

/// Shared by single and bulk endpoints. Returns blob ids to delete after commit.
fn apply_action(
    conn: &mut Conn,
    item: &mut Item,
    action: &str,
    folder: &Option<String>,
    revision: NaiveDateTime,
) -> ApiResult<Vec<String>> {
    match action {
        "trash" => {
            if item.deleted_at.is_none() {
                item.deleted_at = Some(revision);
            }
        }
        "restore" => item.deleted_at = None,
        "move" => item.folder_uuid = folder.clone(),
        "delete" => {
            let blobs = Attachment::blob_ids_for_items(conn, std::slice::from_ref(&item.uuid))?;
            Item::delete(conn, &item.uuid)?;
            Tombstone::record(conn, &item.user_uuid, OBJECT_ITEM, &item.uuid, revision)?;
            return Ok(blobs);
        }
        _ => err!(BadRequest, "action must be trash, restore, delete or move"),
    }
    item.revision_date = revision;
    item.save(conn)?;
    Ok(Vec::new())
}

async fn single_action(
    state: AppState,
    auth: Auth,
    id: String,
    action: &'static str,
) -> ApiResult<(Value, Vec<String>)> {
    let auth = AuthInfo::from(auth);
    state
        .db
        .write(move |conn| {
            let mut item = find_item(conn, &id, &auth.user_uuid)?;
            let revision = User::next_revision(conn, &auth.user_uuid)?;
            let blobs = apply_action(conn, &mut item, action, &None, revision)?;
            event(conn, &auth, &format!("item.{action}"), &item.uuid)?;
            let body = if action == "delete" {
                json!({})
            } else {
                item.to_json_full(conn)?
            };
            Ok((body, blobs))
        })
        .await
}

async fn trash_item(State(state): State<AppState>, auth: Auth, Path(id): Path<String>) -> JsonResult {
    Ok(Json(single_action(state, auth, id, "trash").await?.0))
}

async fn restore_item(State(state): State<AppState>, auth: Auth, Path(id): Path<String>) -> JsonResult {
    Ok(Json(single_action(state, auth, id, "restore").await?.0))
}

async fn delete_item(State(state): State<AppState>, auth: Auth, Path(id): Path<String>) -> JsonResult {
    let storage = state.storage.clone();
    let (body, blobs) = single_action(state, auth, id, "delete").await?;
    storage.delete_many(blobs).await;
    Ok(Json(body))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkData {
    action: String,
    ids: Vec<String>,
    folder_id: Option<String>,
}

async fn bulk_action(State(state): State<AppState>, auth: Auth, ApiJson(data): ApiJson<BulkData>) -> JsonResult {
    if data.ids.is_empty() || data.ids.len() > BULK_MAX {
        err!(BadRequest, format!("ids must contain 1-{BULK_MAX} entries"));
    }
    if !matches!(data.action.as_str(), "trash" | "restore" | "delete" | "move") {
        err!(BadRequest, "action must be trash, restore, delete or move");
    }
    let auth = AuthInfo::from(auth);
    let storage = state.storage.clone();
    let (count, blobs) = state
        .db
        .write(move |conn| {
            let folder = if data.action == "move" {
                check_folder(conn, data.folder_id.as_ref(), &auth.user_uuid)?
            } else {
                None
            };
            let mut items = Item::find_many(conn, &data.ids, &auth.user_uuid)?;
            let revision = User::next_revision(conn, &auth.user_uuid)?;
            let mut blobs = Vec::new();
            for item in &mut items {
                blobs.extend(apply_action(conn, item, &data.action, &folder, revision)?);
            }
            event(conn, &auth, &format!("items.bulk_{}", data.action), &format!("{} items", items.len()))?;
            Ok((items.len(), blobs))
        })
        .await?;
    storage.delete_many(blobs).await;
    Ok(Json(json!({ "affected": count })))
}

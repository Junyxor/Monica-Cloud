use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::{
    db::{Conn, schema::attachments},
    error::ApiResult,
    util::{fmt_date, new_uuid, now},
};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset, Identifiable)]
#[diesel(table_name = attachments, primary_key(uuid))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Attachment {
    pub uuid: String,
    pub item_uuid: String,
    pub user_uuid: String,
    pub file_name: String,
    pub akey: String,
    pub size: i64,
    pub blob_id: String,
    pub uploaded: bool,
    pub created_at: NaiveDateTime,
}

impl Attachment {
    pub fn new(item_uuid: String, user_uuid: String, file_name: String, akey: String, size: i64) -> Self {
        Self {
            uuid: new_uuid(),
            item_uuid,
            user_uuid,
            file_name,
            akey,
            size,
            blob_id: crate::storage::new_blob_id(),
            uploaded: false,
            created_at: now(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.uuid,
            "fileName": self.file_name,
            "key": self.akey,
            "size": self.size,
            "creationDate": fmt_date(&self.created_at),
        })
    }

    pub fn insert(&self, conn: &mut Conn) -> ApiResult<()> {
        diesel::insert_into(attachments::table).values(self).execute(conn)?;
        Ok(())
    }

    pub fn mark_uploaded(conn: &mut Conn, uuid: &str) -> ApiResult<()> {
        diesel::update(attachments::table.find(uuid)).set(attachments::uploaded.eq(true)).execute(conn)?;
        Ok(())
    }

    pub fn delete(conn: &mut Conn, uuid: &str) -> ApiResult<()> {
        diesel::delete(attachments::table.find(uuid)).execute(conn)?;
        Ok(())
    }

    pub fn find(conn: &mut Conn, uuid: &str, item_uuid: &str) -> ApiResult<Option<Self>> {
        Ok(attachments::table
            .filter(attachments::uuid.eq(uuid))
            .filter(attachments::item_uuid.eq(item_uuid))
            .select(Attachment::as_select())
            .first(conn)
            .optional()?)
    }

    pub fn uploaded_for_item(conn: &mut Conn, item_uuid: &str) -> ApiResult<Vec<Self>> {
        Ok(attachments::table
            .filter(attachments::item_uuid.eq(item_uuid))
            .filter(attachments::uploaded.eq(true))
            .order(attachments::created_at.asc())
            .select(Attachment::as_select())
            .load(conn)?)
    }

    /// Blob ids of every attachment (uploaded or pending) of the given items.
    pub fn blob_ids_for_items(conn: &mut Conn, item_uuids: &[String]) -> ApiResult<Vec<String>> {
        Ok(attachments::table
            .filter(attachments::item_uuid.eq_any(item_uuids))
            .select(attachments::blob_id)
            .load(conn)?)
    }

    pub fn blob_ids_for_user(conn: &mut Conn, user_uuid: &str) -> ApiResult<Vec<String>> {
        Ok(attachments::table.filter(attachments::user_uuid.eq(user_uuid)).select(attachments::blob_id).load(conn)?)
    }

    /// Remove registrations that were never uploaded; returns their blob ids.
    pub fn purge_stale_pending(conn: &mut Conn, older_than: NaiveDateTime) -> ApiResult<Vec<String>> {
        let stale =
            attachments::table.filter(attachments::uploaded.eq(false)).filter(attachments::created_at.lt(older_than));
        let blobs: Vec<String> = stale.select(attachments::blob_id).load(conn)?;
        diesel::delete(stale).execute(conn)?;
        Ok(blobs)
    }

    pub fn count_all(conn: &mut Conn) -> ApiResult<(i64, i64)> {
        use diesel::{dsl::sql, sql_types::BigInt};
        Ok(attachments::table
            .filter(attachments::uploaded.eq(true))
            .select((sql::<BigInt>("COUNT(*)"), sql::<BigInt>("COALESCE(SUM(size), 0)")))
            .first(conn)?)
    }
}

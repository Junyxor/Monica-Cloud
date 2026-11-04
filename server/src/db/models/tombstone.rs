use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::{
    db::{Conn, schema::tombstones},
    error::ApiResult,
    util::{fmt_date, new_uuid},
};

pub const OBJECT_ITEM: &str = "item";
pub const OBJECT_FOLDER: &str = "folder";
pub const OBJECT_SEND: &str = "send";

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = tombstones)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tombstone {
    pub uuid: String,
    pub user_uuid: String,
    pub object_type: String,
    pub object_uuid: String,
    pub deleted_at: NaiveDateTime,
}

impl Tombstone {
    /// Record a permanent deletion at the transaction's revision.
    pub fn record(
        conn: &mut Conn,
        user_uuid: &str,
        object_type: &str,
        object_uuid: &str,
        revision: NaiveDateTime,
    ) -> ApiResult<()> {
        let row = Tombstone {
            uuid: new_uuid(),
            user_uuid: user_uuid.into(),
            object_type: object_type.into(),
            object_uuid: object_uuid.into(),
            deleted_at: revision,
        };
        diesel::insert_into(tombstones::table).values(&row).execute(conn)?;
        Ok(())
    }

    pub fn to_json(&self) -> Value {
        json!({ "type": self.object_type, "id": self.object_uuid, "deletedAt": fmt_date(&self.deleted_at) })
    }

    pub fn since(conn: &mut Conn, user_uuid: &str, since: NaiveDateTime) -> ApiResult<Vec<Self>> {
        Ok(tombstones::table
            .filter(tombstones::user_uuid.eq(user_uuid))
            .filter(tombstones::deleted_at.gt(since))
            .order(tombstones::deleted_at.asc())
            .select(Tombstone::as_select())
            .load(conn)?)
    }

    pub fn purge_older_than(conn: &mut Conn, cutoff: NaiveDateTime) -> ApiResult<usize> {
        Ok(diesel::delete(tombstones::table.filter(tombstones::deleted_at.lt(cutoff))).execute(conn)?)
    }
}

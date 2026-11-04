use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::{
    db::{Conn, schema::folders},
    error::ApiResult,
    util::{fmt_date, new_uuid, now},
};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, AsChangeset, Identifiable)]
#[diesel(table_name = folders, primary_key(uuid))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Folder {
    pub uuid: String,
    pub user_uuid: String,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub revision_date: NaiveDateTime,
}

impl Folder {
    pub fn new(user_uuid: String, name: String, revision: NaiveDateTime) -> Self {
        Self {
            uuid: new_uuid(),
            user_uuid,
            name,
            created_at: now(),
            revision_date: revision,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.uuid,
            "name": self.name,
            "creationDate": fmt_date(&self.created_at),
            "revisionDate": fmt_date(&self.revision_date),
        })
    }

    pub fn insert(&self, conn: &mut Conn) -> ApiResult<()> {
        diesel::insert_into(folders::table).values(self).execute(conn)?;
        Ok(())
    }

    pub fn save(&self, conn: &mut Conn) -> ApiResult<()> {
        diesel::update(folders::table.find(&self.uuid)).set(self).execute(conn)?;
        Ok(())
    }

    pub fn delete(conn: &mut Conn, uuid: &str) -> ApiResult<()> {
        diesel::delete(folders::table.find(uuid)).execute(conn)?;
        Ok(())
    }

    pub fn find(conn: &mut Conn, uuid: &str, user_uuid: &str) -> ApiResult<Option<Self>> {
        Ok(folders::table
            .filter(folders::uuid.eq(uuid))
            .filter(folders::user_uuid.eq(user_uuid))
            .select(Folder::as_select())
            .first(conn)
            .optional()?)
    }

    pub fn changed_since(conn: &mut Conn, user_uuid: &str, since: Option<NaiveDateTime>) -> ApiResult<Vec<Self>> {
        let mut query = folders::table.filter(folders::user_uuid.eq(user_uuid)).into_boxed();
        if let Some(since) = since {
            query = query.filter(folders::revision_date.gt(since));
        }
        Ok(query.order(folders::created_at.asc()).select(Folder::as_select()).load(conn)?)
    }
}

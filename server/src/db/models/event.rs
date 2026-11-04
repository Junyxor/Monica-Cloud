use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::{
    db::{Conn, schema::events},
    error::ApiResult,
    util::{fmt_date, new_uuid, now},
};

/// Audit log entry. Targets are object ids, never decrypted content.
#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = events)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Event {
    pub uuid: String,
    pub user_uuid: Option<String>,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub ip: String,
    pub device_name: Option<String>,
    pub created_at: NaiveDateTime,
}

impl Event {
    pub fn new(user_uuid: Option<&str>, actor: &str, action: &str, target: &str, ip: &str) -> Self {
        Self {
            uuid: new_uuid(),
            user_uuid: user_uuid.map(Into::into),
            actor: actor.into(),
            action: action.into(),
            target: target.into(),
            ip: ip.into(),
            device_name: None,
            created_at: now(),
        }
    }

    pub fn device(mut self, name: &str) -> Self {
        self.device_name = Some(name.into());
        self
    }

    pub fn insert(&self, conn: &mut Conn) -> ApiResult<()> {
        diesel::insert_into(events::table).values(self).execute(conn)?;
        Ok(())
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.uuid,
            "userId": self.user_uuid,
            "actor": self.actor,
            "action": self.action,
            "target": self.target,
            "ip": self.ip,
            "deviceName": self.device_name,
            "date": fmt_date(&self.created_at),
        })
    }

    pub fn page(
        conn: &mut Conn,
        user_uuid: Option<&str>,
        before: Option<NaiveDateTime>,
        limit: i64,
    ) -> ApiResult<Vec<Self>> {
        let mut query = events::table.into_boxed();
        if let Some(user_uuid) = user_uuid {
            query = query.filter(events::user_uuid.eq(user_uuid.to_string()));
        }
        if let Some(before) = before {
            query = query.filter(events::created_at.lt(before));
        }
        Ok(query.order(events::created_at.desc()).limit(limit).select(Event::as_select()).load(conn)?)
    }

    pub fn purge_older_than(conn: &mut Conn, cutoff: NaiveDateTime) -> ApiResult<usize> {
        Ok(diesel::delete(events::table.filter(events::created_at.lt(cutoff))).execute(conn)?)
    }
}

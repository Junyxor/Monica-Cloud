use diesel::prelude::*;

use crate::{
    config::RuntimeSettings,
    db::{Conn, schema::server_settings},
    error::ApiResult,
};

const KEY: &str = "runtime";

/// Persists [`RuntimeSettings`] as one JSON row, like vaultwarden's `config.json` override.
pub struct ServerSettings;

impl ServerSettings {
    /// Load stored overrides into the in-memory cache, if any exist.
    pub fn load(conn: &mut Conn) -> ApiResult<()> {
        let stored: Option<String> = server_settings::table
            .filter(server_settings::key.eq(KEY))
            .select(server_settings::value)
            .first(conn)
            .optional()?;
        if let Some(raw) = stored {
            match serde_json::from_str::<RuntimeSettings>(&raw) {
                Ok(settings) => RuntimeSettings::replace(settings),
                Err(e) => tracing::warn!("ignoring unreadable stored settings: {e}"),
            }
        }
        Ok(())
    }

    pub fn store(conn: &mut Conn, settings: &RuntimeSettings) -> ApiResult<()> {
        let raw = serde_json::to_string(settings)?;
        diesel::insert_into(server_settings::table)
            .values((server_settings::key.eq(KEY), server_settings::value.eq(&raw)))
            .on_conflict(server_settings::key)
            .do_update()
            .set(server_settings::value.eq(&raw))
            .execute(conn)?;
        Ok(())
    }
}

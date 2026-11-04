//! Authenticated vault endpoints (vaultwarden's `api/core`).

mod accounts;
mod attachments;
mod devices;
mod folders;
mod items;
mod sends;
mod sync;

use axum::Router;

use super::AppState;

pub use accounts::EventQuery;
pub use sends::purge_expired_sends;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(accounts::routes())
        .merge(sync::routes())
        .merge(items::routes())
        .merge(folders::routes())
        .merge(attachments::routes())
        .merge(sends::routes())
        .merge(devices::routes())
}

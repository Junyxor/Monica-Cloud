-- Monica Cloud initial schema (protocol v1, see docs/protocol.zh-CN.md).
--
-- The server is zero-knowledge: it stores account metadata, opaque client
-- ciphertext and blob references. Nothing in this schema is decrypted server-side.

CREATE TABLE users (
    uuid            TEXT    NOT NULL PRIMARY KEY,
    email           TEXT    NOT NULL UNIQUE,
    name            TEXT    NOT NULL,
    role            TEXT    NOT NULL DEFAULT 'user',      -- owner | admin | user
    enabled         BOOLEAN NOT NULL DEFAULT 1,
    -- argon2id(authHash); the master password never reaches the server
    password_hash   TEXT    NOT NULL,
    kdf_type        INTEGER NOT NULL DEFAULT 0,
    kdf_iterations  INTEGER NOT NULL,
    -- userKey wrapped by the client-derived wrapKey
    akey            TEXT    NOT NULL,
    security_stamp  TEXT    NOT NULL,
    revision_date   TIMESTAMP NOT NULL,
    quota_bytes     BIGINT,
    created_at      TIMESTAMP NOT NULL,
    updated_at      TIMESTAMP NOT NULL,
    last_login_at   TIMESTAMP
);

CREATE TABLE devices (
    uuid                TEXT    NOT NULL PRIMARY KEY,
    user_uuid           TEXT    NOT NULL REFERENCES users (uuid) ON DELETE CASCADE,
    identifier          TEXT    NOT NULL,
    name                TEXT    NOT NULL,
    dtype               TEXT    NOT NULL,                 -- android | web | desktop | cli
    app_version         TEXT    NOT NULL DEFAULT '',
    refresh_token_hash  TEXT,
    created_at          TIMESTAMP NOT NULL,
    updated_at          TIMESTAMP NOT NULL,
    last_seen_at        TIMESTAMP NOT NULL,
    last_ip             TEXT    NOT NULL DEFAULT '',
    UNIQUE (user_uuid, identifier)
);
CREATE UNIQUE INDEX idx_devices_refresh ON devices (refresh_token_hash);

CREATE TABLE folders (
    uuid           TEXT NOT NULL PRIMARY KEY,
    user_uuid      TEXT NOT NULL REFERENCES users (uuid) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    created_at     TIMESTAMP NOT NULL,
    revision_date  TIMESTAMP NOT NULL
);
CREATE INDEX idx_folders_user ON folders (user_uuid, revision_date);

CREATE TABLE items (
    uuid           TEXT    NOT NULL PRIMARY KEY,
    user_uuid      TEXT    NOT NULL REFERENCES users (uuid) ON DELETE CASCADE,
    folder_uuid    TEXT    REFERENCES folders (uuid) ON DELETE SET NULL,
    atype          INTEGER NOT NULL,
    favorite       BOOLEAN NOT NULL DEFAULT 0,
    akey           TEXT    NOT NULL,
    data           TEXT    NOT NULL,
    created_at     TIMESTAMP NOT NULL,
    revision_date  TIMESTAMP NOT NULL,
    deleted_at     TIMESTAMP,
    archived_at    TIMESTAMP
);
CREATE INDEX idx_items_user ON items (user_uuid, revision_date);
CREATE INDEX idx_items_folder ON items (folder_uuid);

CREATE TABLE attachments (
    uuid        TEXT    NOT NULL PRIMARY KEY,
    item_uuid   TEXT    NOT NULL REFERENCES items (uuid) ON DELETE CASCADE,
    user_uuid   TEXT    NOT NULL REFERENCES users (uuid) ON DELETE CASCADE,
    file_name   TEXT    NOT NULL,
    akey        TEXT    NOT NULL,
    size        BIGINT  NOT NULL,
    blob_id     TEXT    NOT NULL,
    uploaded    BOOLEAN NOT NULL DEFAULT 0,
    created_at  TIMESTAMP NOT NULL
);
CREATE INDEX idx_attachments_item ON attachments (item_uuid);
CREATE INDEX idx_attachments_user ON attachments (user_uuid);

CREATE TABLE sends (
    uuid              TEXT    NOT NULL PRIMARY KEY,
    user_uuid         TEXT    NOT NULL REFERENCES users (uuid) ON DELETE CASCADE,
    access_id         TEXT    NOT NULL UNIQUE,
    atype             INTEGER NOT NULL,                  -- 0 = text, 1 = file
    akey              TEXT    NOT NULL,                  -- sendKey wrapped by userKey
    name              TEXT    NOT NULL,                  -- EncString(contentKey)
    notes             TEXT,
    text              TEXT,
    file_name         TEXT,
    size_bytes        BIGINT,
    blob_id           TEXT,
    uploaded          BOOLEAN NOT NULL DEFAULT 0,
    password_hash     TEXT,
    max_access_count  INTEGER,
    access_count      INTEGER NOT NULL DEFAULT 0,
    disabled          BOOLEAN NOT NULL DEFAULT 0,
    hide_email        BOOLEAN NOT NULL DEFAULT 0,
    expiration_at     TIMESTAMP,
    deletion_at       TIMESTAMP NOT NULL,
    created_at        TIMESTAMP NOT NULL,
    revision_date     TIMESTAMP NOT NULL
);
CREATE INDEX idx_sends_user ON sends (user_uuid, revision_date);
CREATE INDEX idx_sends_deletion ON sends (deletion_at);

-- Permanent deletions, so incremental sync can tell clients what to drop.
CREATE TABLE tombstones (
    uuid         TEXT NOT NULL PRIMARY KEY,
    user_uuid    TEXT NOT NULL REFERENCES users (uuid) ON DELETE CASCADE,
    object_type  TEXT NOT NULL,                          -- item | folder | send
    object_uuid  TEXT NOT NULL,
    deleted_at   TIMESTAMP NOT NULL
);
CREATE INDEX idx_tombstones_user ON tombstones (user_uuid, deleted_at);

CREATE TABLE events (
    uuid         TEXT NOT NULL PRIMARY KEY,
    user_uuid    TEXT REFERENCES users (uuid) ON DELETE SET NULL,
    actor        TEXT NOT NULL,
    action       TEXT NOT NULL,
    target       TEXT NOT NULL DEFAULT '',
    ip           TEXT NOT NULL DEFAULT '',
    device_name  TEXT,
    created_at   TIMESTAMP NOT NULL
);
CREATE INDEX idx_events_user ON events (user_uuid, created_at);
CREATE INDEX idx_events_created ON events (created_at);

-- Runtime settings changed from the admin panel; they override environment defaults.
CREATE TABLE server_settings (
    key    TEXT NOT NULL PRIMARY KEY,
    value  TEXT NOT NULL
);

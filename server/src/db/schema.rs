// Mirrors migrations/sqlite. Keep the two in sync by hand (as vaultwarden does).

diesel::table! {
    users (uuid) {
        uuid -> Text,
        email -> Text,
        name -> Text,
        role -> Text,
        enabled -> Bool,
        password_hash -> Text,
        kdf_type -> Integer,
        kdf_iterations -> Integer,
        akey -> Text,
        security_stamp -> Text,
        revision_date -> Timestamp,
        quota_bytes -> Nullable<BigInt>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        last_login_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    devices (uuid) {
        uuid -> Text,
        user_uuid -> Text,
        identifier -> Text,
        name -> Text,
        dtype -> Text,
        app_version -> Text,
        refresh_token_hash -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        last_seen_at -> Timestamp,
        last_ip -> Text,
    }
}

diesel::table! {
    folders (uuid) {
        uuid -> Text,
        user_uuid -> Text,
        name -> Text,
        created_at -> Timestamp,
        revision_date -> Timestamp,
    }
}

diesel::table! {
    items (uuid) {
        uuid -> Text,
        user_uuid -> Text,
        folder_uuid -> Nullable<Text>,
        atype -> Integer,
        favorite -> Bool,
        akey -> Text,
        data -> Text,
        created_at -> Timestamp,
        revision_date -> Timestamp,
        deleted_at -> Nullable<Timestamp>,
        archived_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    attachments (uuid) {
        uuid -> Text,
        item_uuid -> Text,
        user_uuid -> Text,
        file_name -> Text,
        akey -> Text,
        size -> BigInt,
        blob_id -> Text,
        uploaded -> Bool,
        created_at -> Timestamp,
    }
}

diesel::table! {
    sends (uuid) {
        uuid -> Text,
        user_uuid -> Text,
        access_id -> Text,
        atype -> Integer,
        akey -> Text,
        name -> Text,
        notes -> Nullable<Text>,
        text -> Nullable<Text>,
        file_name -> Nullable<Text>,
        size_bytes -> Nullable<BigInt>,
        blob_id -> Nullable<Text>,
        uploaded -> Bool,
        password_hash -> Nullable<Text>,
        max_access_count -> Nullable<Integer>,
        access_count -> Integer,
        disabled -> Bool,
        hide_email -> Bool,
        expiration_at -> Nullable<Timestamp>,
        deletion_at -> Timestamp,
        created_at -> Timestamp,
        revision_date -> Timestamp,
    }
}

diesel::table! {
    tombstones (uuid) {
        uuid -> Text,
        user_uuid -> Text,
        object_type -> Text,
        object_uuid -> Text,
        deleted_at -> Timestamp,
    }
}

diesel::table! {
    events (uuid) {
        uuid -> Text,
        user_uuid -> Nullable<Text>,
        actor -> Text,
        action -> Text,
        target -> Text,
        ip -> Text,
        device_name -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    server_settings (key) {
        key -> Text,
        value -> Text,
    }
}

diesel::joinable!(devices -> users (user_uuid));
diesel::joinable!(folders -> users (user_uuid));
diesel::joinable!(items -> users (user_uuid));
diesel::joinable!(attachments -> items (item_uuid));
diesel::joinable!(sends -> users (user_uuid));
diesel::joinable!(tombstones -> users (user_uuid));

diesel::allow_tables_to_appear_in_same_query!(
    users,
    devices,
    folders,
    items,
    attachments,
    sends,
    tombstones,
    events,
    server_settings,
);

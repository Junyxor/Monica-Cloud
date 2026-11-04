//! Content-addressed-by-id blob store on the local filesystem.
//!
//! Uploads are streamed to `tmp/`, fsynced and then renamed into
//! `blobs/<2 hex>/<id>`, so a blob is either fully present or absent.
//! The bytes are client ciphertext; the server never inspects them.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use axum::body::Body;
use futures_util::StreamExt;
use tokio::{fs, io::AsyncWriteExt};
use tokio_util::io::ReaderStream;

use crate::{
    err,
    error::{ApiResult, Error},
};

pub fn new_blob_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[derive(Clone)]
pub struct Storage {
    blobs: PathBuf,
    tmp: PathBuf,
}

impl Storage {
    pub fn new(blobs: PathBuf, tmp: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&blobs)?;
        std::fs::create_dir_all(&tmp)?;
        Ok(Self {
            blobs,
            tmp,
        })
    }

    fn blob_path(&self, blob_id: &str) -> ApiResult<PathBuf> {
        if blob_id.len() != 32 || !blob_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Internal(format!("invalid blob id {blob_id:?}")));
        }
        Ok(self.blobs.join(&blob_id[..2]).join(blob_id))
    }

    /// Stream `body` into the blob `blob_id`. The body must be exactly `expected` bytes.
    pub async fn store(&self, blob_id: &str, body: Body, expected: i64) -> ApiResult<()> {
        let tmp = self.tmp.join(new_blob_id());
        let result = self.store_inner(&tmp, blob_id, body, expected).await;
        if result.is_err() {
            let _ = fs::remove_file(&tmp).await;
        }
        result
    }

    async fn store_inner(&self, tmp: &PathBuf, blob_id: &str, body: Body, expected: i64) -> ApiResult<()> {
        let target = self.blob_path(blob_id)?;
        let mut file = fs::File::create(tmp).await?;
        let mut written: i64 = 0;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Error::BadRequest(format!("upload interrupted: {e}")))?;
            written += chunk.len() as i64;
            if written > expected {
                err!(BadRequest, "upload is larger than the declared size");
            }
            file.write_all(&chunk).await?;
        }
        if written != expected {
            err!(BadRequest, format!("upload has {written} bytes but {expected} were declared"));
        }
        file.sync_all().await?;
        drop(file);

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::rename(tmp, &target).await?;
        Ok(())
    }

    /// Open a blob for streaming. Returns the body and its length.
    pub async fn open(&self, blob_id: &str) -> ApiResult<(Body, u64)> {
        let path = self.blob_path(blob_id)?;
        let file = match fs::File::open(&path).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => err!(NotFound, "file content is missing"),
            Err(e) => return Err(e.into()),
        };
        let len = file.metadata().await?.len();
        Ok((Body::from_stream(ReaderStream::new(file)), len))
    }

    pub async fn delete(&self, blob_id: &str) {
        let Ok(path) = self.blob_path(blob_id) else {
            return;
        };
        if let Err(e) = fs::remove_file(&path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("could not delete blob {blob_id}: {e}");
        }
    }

    pub async fn delete_many(&self, blob_ids: Vec<String>) {
        for blob_id in blob_ids {
            self.delete(&blob_id).await;
        }
    }

    /// Remove abandoned partial uploads.
    pub async fn cleanup_tmp(&self, max_age: Duration) -> std::io::Result<usize> {
        let mut removed = 0;
        let mut entries = fs::read_dir(&self.tmp).await?;
        while let Some(entry) = entries.next_entry().await? {
            let modified = entry.metadata().await?.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if modified.elapsed().unwrap_or_default() > max_age && fs::remove_file(entry.path()).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

//! MinioHarness: manages a MinIO container for object-store-backed tests.
//!
//! This harness uses the `testcontainers` ecosystem so that Tier 4+ tests run
//! against a real S3-compatible container rather than a hand-rolled Docker
//! wrapper. The harness starts one container per test suite and exposes a
//! configured `object_store::ObjectStore` for catalog tests.

use std::sync::Arc;
use std::time::Duration;

use futures::TryStreamExt;
use hmac::{Hmac, Mac};
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use sha2::{Digest, Sha256};
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::minio::MinIO;

/// MinIO container harness for S3-compatible object store tests.
pub struct MinioHarness {
    container: ContainerAsync<MinIO>,
    /// HTTP endpoint for the MinIO S3 API, e.g. `http://127.0.0.1:49382`.
    pub endpoint: String,
    /// Root access key used by the test container.
    pub access_key: String,
    /// Root secret key used by the test container.
    pub secret_key: String,
    /// Bucket assigned to this harness.
    pub bucket: String,
}

/// Default MinIO credentials used in the test container.
const MINIO_ACCESS_KEY: &str = "minioadmin";
const MINIO_SECRET_KEY: &str = "minioadmin";

impl MinioHarness {
    /// Start a MinIO container and create the test bucket.
    pub async fn start(bucket: &str) -> Result<Self, MinioHarnessError> {
        let container = MinIO::default()
            .start()
            .await
            .map_err(|e| MinioHarnessError::Docker(format!("failed to start container: {e}")))?;

        let port = container
            .get_host_port_ipv4(9000)
            .await
            .map_err(|e| MinioHarnessError::Docker(format!("failed to resolve port: {e}")))?;
        let host = container
            .get_host()
            .await
            .map_err(|e| MinioHarnessError::Docker(format!("failed to resolve host: {e}")))?;

        let harness = Self {
            container,
            endpoint: format!("http://{host}:{port}"),
            access_key: MINIO_ACCESS_KEY.to_string(),
            secret_key: MINIO_SECRET_KEY.to_string(),
            bucket: bucket.to_string(),
        };

        harness.wait_for_ready(Duration::from_secs(30)).await?;
        harness.create_bucket().await?;
        Ok(harness)
    }

    /// Build an object-store client bound to the harness bucket.
    pub fn object_store(&self) -> Arc<dyn ObjectStore> {
        let s3 = AmazonS3Builder::new()
            .with_endpoint(&self.endpoint)
            .with_bucket_name(&self.bucket)
            .with_access_key_id(&self.access_key)
            .with_secret_access_key(&self.secret_key)
            .with_region("us-east-1")
            .with_allow_http(true)
            .build()
            .expect("failed to build S3 client for MinIO");
        Arc::new(s3)
    }

    /// Open options for the harness bucket and the given object-store prefix.
    pub fn open_options(&self, prefix: &str) -> rocklake_catalog::OpenOptions {
        rocklake_catalog::OpenOptions {
            object_store: self.object_store(),
            path: ObjectPath::from(prefix),
            encryption: None,
        }
    }

    /// Put a raw object into the harness bucket.
    pub async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), MinioHarnessError> {
        let store = self.object_store();
        let payload: object_store::PutPayload = data.to_vec().into();
        store
            .put(&ObjectPath::from(key), payload)
            .await
            .map_err(|e| MinioHarnessError::ObjectStore(e.to_string()))?;
        Ok(())
    }

    /// List object keys under a prefix.
    pub async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, MinioHarnessError> {
        let store = self.object_store();
        let object_prefix = ObjectPath::from(prefix);
        let objects = store
            .list(Some(&object_prefix))
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| MinioHarnessError::ObjectStore(e.to_string()))?;
        Ok(objects
            .into_iter()
            .map(|object| object.location.to_string())
            .collect())
    }

    /// Delete an object from the harness bucket.
    pub async fn delete_object(&self, key: &str) -> Result<(), MinioHarnessError> {
        let store = self.object_store();
        store
            .delete(&ObjectPath::from(key))
            .await
            .map_err(|e| MinioHarnessError::ObjectStore(e.to_string()))?;
        Ok(())
    }

    /// Create the test bucket via a signed S3 `PUT Bucket` request.
    pub async fn create_bucket(&self) -> Result<(), MinioHarnessError> {
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let host = self
            .endpoint
            .strip_prefix("http://")
            .unwrap_or(&self.endpoint)
            .to_string();
        let payload_hash = format!("{:x}", Sha256::digest(b""));

        let canonical_headers = format!(
            "host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "PUT\n/{bucket}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
            bucket = self.bucket,
        );
        let scope = format!("{date_stamp}/us-east-1/s3/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{:x}",
            Sha256::digest(canonical_request.as_bytes())
        );
        let signing_key = derive_signing_key(&self.secret_key, &date_stamp)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&signing_key)
            .map_err(|e| MinioHarnessError::BucketCreate(format!("hmac init failed: {e}")))?;
        mac.update(string_to_sign.as_bytes());
        let signature = format!("{:x}", mac.finalize().into_bytes());
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key,
        );

        let client = reqwest::Client::new();
        let resp = client
            .put(format!("{}/{}", self.endpoint, self.bucket))
            .header("x-amz-date", amz_date)
            .header("x-amz-content-sha256", payload_hash)
            .header("Authorization", authorization)
            .send()
            .await
            .map_err(|e| MinioHarnessError::BucketCreate(e.to_string()))?;

        if resp.status().is_success() || resp.status().as_u16() == 409 {
            Ok(())
        } else {
            Err(MinioHarnessError::BucketCreate(format!(
                "unexpected status: {}",
                resp.status()
            )))
        }
    }

    /// Wait until MinIO's health endpoint responds.
    async fn wait_for_ready(&self, timeout: Duration) -> Result<(), MinioHarnessError> {
        let start = std::time::Instant::now();
        let client = reqwest::Client::new();
        loop {
            if start.elapsed() > timeout {
                return Err(MinioHarnessError::Timeout(
                    "MinIO did not become ready in time".into(),
                ));
            }
            match client
                .get(format!("{}/minio/health/live", self.endpoint))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                _ => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
    }
}

impl Drop for MinioHarness {
    fn drop(&mut self) {
        let _ = &self.container;
    }
}

/// Errors from the MinIO harness.
#[derive(Debug, thiserror::Error)]
pub enum MinioHarnessError {
    #[error("docker error: {0}")]
    Docker(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("bucket creation failed: {0}")]
    BucketCreate(String),
    #[error("object store operation failed: {0}")]
    ObjectStore(String),
}

fn derive_signing_key(secret_key: &str, date_stamp: &str) -> Result<Vec<u8>, MinioHarnessError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(format!("AWS4{secret_key}").as_bytes())
        .map_err(|e| MinioHarnessError::BucketCreate(format!("hmac init failed: {e}")))?;
    mac.update(date_stamp.as_bytes());
    let date_key = mac.finalize().into_bytes().to_vec();

    let mut mac = Hmac::<Sha256>::new_from_slice(&date_key)
        .map_err(|e| MinioHarnessError::BucketCreate(format!("hmac init failed: {e}")))?;
    mac.update(b"us-east-1");
    let region_key = mac.finalize().into_bytes().to_vec();

    let mut mac = Hmac::<Sha256>::new_from_slice(&region_key)
        .map_err(|e| MinioHarnessError::BucketCreate(format!("hmac init failed: {e}")))?;
    mac.update(b"s3");
    let service_key = mac.finalize().into_bytes().to_vec();

    let mut mac = Hmac::<Sha256>::new_from_slice(&service_key)
        .map_err(|e| MinioHarnessError::BucketCreate(format!("hmac init failed: {e}")))?;
    mac.update(b"aws4_request");
    Ok(mac.finalize().into_bytes().to_vec())
}

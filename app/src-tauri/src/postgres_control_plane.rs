use sha2::{Digest, Sha256};
use tokio_postgres::{Client, NoTls};

#[derive(Clone, Debug)]
pub struct UploadControlPlaneEvent<'a> {
    pub owner_id: &'a str,
    pub message_id: i32,
    pub storage_channel_id: i64,
    pub file_name: &'a str,
    pub size_bytes: i64,
}

#[derive(Clone, Debug)]
pub struct BeginUploadRequest<'a> {
    pub owner_id: &'a str,
    pub idempotency_key: &'a str,
    pub request_fingerprint: &'a str,
    pub file_name: &'a str,
    pub size_bytes: i64,
    pub folder_id: Option<i64>,
    pub source_ref: &'a str,
    pub transport_mode: &'a str,
    pub target: &'a str,
    pub staging_node_id: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingUpload {
    pub tenant_id: String,
    pub job_id: String,
    pub correlation_id: String,
    pub attempt_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedUpload {
    pub tenant_id: String,
    pub job_id: String,
    pub correlation_id: String,
    pub asset_id: String,
    pub receipt: crate::telegram_transport::TelegramUploadReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeginUploadDecision {
    Proceed(PendingUpload),
    InProgress {
        job_id: String,
        correlation_id: String,
    },
    Completed(CompletedUpload),
    ResumeFinalize(PendingUpload),
    CompensationRequired {
        job_id: String,
        correlation_id: String,
    },
    NeedsReconciliation {
        job_id: String,
        correlation_id: String,
    },
    Terminal {
        job_id: String,
        correlation_id: String,
        status: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UploadReplayAction {
    Acquire,
    InProgress,
    Completed,
    ResumeFinalize,
    CompensationRequired,
    NeedsReconciliation,
    Terminal,
    Conflict,
}

pub(crate) fn classify_upload_replay(
    status: &str,
    fingerprint_matches: bool,
    lease_active: bool,
) -> UploadReplayAction {
    if !fingerprint_matches {
        return UploadReplayAction::Conflict;
    }
    match status {
        "finalized" | "succeeded" => UploadReplayAction::Completed,
        "pending" | "queued" | "retry_wait" => UploadReplayAction::Acquire,
        "running" if lease_active => UploadReplayAction::InProgress,
        "running" => UploadReplayAction::NeedsReconciliation,
        "telegram_succeeded" if lease_active => UploadReplayAction::InProgress,
        "telegram_succeeded" => UploadReplayAction::ResumeFinalize,
        "compensation_pending" => UploadReplayAction::CompensationRequired,
        "failed" | "compensated" | "cancelled" => UploadReplayAction::Terminal,
        _ => UploadReplayAction::Terminal,
    }
}

pub fn upload_request_fingerprint(
    file_name: &str,
    size_bytes: i64,
    folder_id: Option<i64>,
    content_sha256: &str,
    source_ref: &str,
    transport_mode: &str,
    target: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"telegram-drive-upload-v2\0");
    for value in [
        file_name,
        content_sha256,
        source_ref,
        transport_mode,
        target,
    ] {
        hasher.update(value.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(size_bytes.to_be_bytes());
    hasher.update(folder_id.unwrap_or(i64::MIN).to_be_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Debug)]
pub struct PostgresControlPlane {
    config: Option<tokio_postgres::Config>,
    host: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeRoleFlags {
    identities_match: bool,
    is_superuser: bool,
    bypasses_rls: bool,
    owns_control_tables: bool,
    has_memberships: bool,
}

impl RuntimeRoleFlags {
    fn is_safe(self) -> bool {
        self.identities_match
            && !self.is_superuser
            && !self.bypasses_rls
            && !self.owns_control_tables
            && !self.has_memberships
    }
}

fn is_loopback_postgres_host(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

impl PostgresControlPlane {
    pub fn from_env() -> Result<Self, String> {
        let mode = std::env::var("SAAS_DATABASE_MODE").unwrap_or_else(|_| "sqlite".into());
        if mode.eq_ignore_ascii_case("sqlite") {
            return Ok(Self {
                config: None,
                host: None,
            });
        }
        if !mode.eq_ignore_ascii_case("postgres") {
            return Err(format!("unsupported SAAS_DATABASE_MODE: {mode}"));
        }
        let host = required_env("POSTGRES_HOST")?;
        if !is_loopback_postgres_host(&host) {
            return Err(
                "remote PostgreSQL requires a TLS connector with CA validation; NoTls is allowed only for loopback"
                    .into(),
            );
        }
        let mut config = tokio_postgres::Config::new();
        config
            .host(&host)
            .port(
                required_env("POSTGRES_PORT")?
                    .parse()
                    .map_err(|_| "invalid POSTGRES_PORT")?,
            )
            .dbname(&required_env("POSTGRES_DB")?)
            .user(&required_env("POSTGRES_APP_USER")?)
            .password(required_env("POSTGRES_APP_PASSWORD")?);
        Ok(Self {
            config: Some(config),
            host: Some(host),
        })
    }

    pub fn enabled(&self) -> bool {
        self.config.is_some()
    }

    pub(crate) async fn connect_checked(&self) -> Result<Client, String> {
        let Some(config) = &self.config else {
            return Err("PostgreSQL control plane is disabled".into());
        };
        let host = self.host.as_deref().unwrap_or_default();
        if !is_loopback_postgres_host(host) {
            return Err(
                "remote PostgreSQL requires a TLS connector with CA validation; NoTls is allowed only for loopback"
                    .into(),
            );
        }
        let (client, connection) = config.connect(NoTls).await.map_err(redacted_pg_error)?;
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                log::error!("PostgreSQL control-plane connection failed: {}", error);
            }
        });
        verify_runtime_role(&client).await?;
        Ok(client)
    }

    pub async fn record_upload(&self, event: UploadControlPlaneEvent<'_>) -> Result<(), String> {
        let Some(_) = &self.config else {
            return Ok(());
        };
        let mut client = self.connect_checked().await?;
        let candidate_tenant_id = deterministic_uuid("tenant", event.owner_id);
        let candidate_asset_id = deterministic_uuid(
            "asset",
            &format!(
                "{}:{}:{}",
                event.owner_id, event.storage_channel_id, event.message_id
            ),
        );
        let media_kind = media_kind(event.file_name);
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        let tenant_id: String = tx
            .query_one(
                "SELECT resolve_legacy_tenant($1,$2::text::uuid,$3)::text",
                &[&event.owner_id, &candidate_tenant_id, &event.owner_id],
            )
            .await
            .map_err(redacted_pg_error)?
            .get(0);
        tx.execute(
            "SELECT set_config('app.tenant_id', $1, true)",
            &[&tenant_id],
        )
        .await
        .map_err(redacted_pg_error)?;
        let asset_id: String = tx
            .query_one(
                "INSERT INTO assets(id,tenant_id,telegram_message_id,storage_channel_id,file_name,media_kind,size_bytes,status) VALUES ($1::text::uuid,$2::text::uuid,$3,$4,$5,$6,$7,'ready') ON CONFLICT (storage_channel_id,telegram_message_id) DO UPDATE SET file_name=EXCLUDED.file_name,size_bytes=EXCLUDED.size_bytes,status='ready' RETURNING id::text",
                &[&candidate_asset_id, &tenant_id, &(event.message_id as i64), &event.storage_channel_id, &event.file_name, &media_kind, &event.size_bytes],
            )
            .await
            .map_err(redacted_pg_error)?
            .get(0);
        let candidate_job_id = deterministic_uuid("upload-job", &asset_id);
        let correlation_id = deterministic_uuid("upload-correlation", &asset_id);
        let transfer_idempotency =
            format!("telegram:{}:{}", event.storage_channel_id, event.message_id);
        let job_id: String = tx
            .query_one(
                "INSERT INTO transfer_jobs(id,tenant_id,asset_id,direction,idempotency_key,status,bytes_total,bytes_transferred,attempt_count,correlation_id) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,'upload',$4,'succeeded',$5,$5,1,$6::text::uuid) ON CONFLICT (tenant_id,direction,idempotency_key) DO UPDATE SET asset_id=EXCLUDED.asset_id,status='succeeded',bytes_total=EXCLUDED.bytes_total,bytes_transferred=EXCLUDED.bytes_transferred,updated_at=now() RETURNING id::text",
                &[&candidate_job_id, &tenant_id, &asset_id, &transfer_idempotency, &event.size_bytes, &correlation_id],
            )
            .await
            .map_err(redacted_pg_error)?
            .get(0);
        for (kind, quantity) in [
            ("asset_stored", event.size_bytes),
            ("upload_bytes", event.size_bytes),
        ] {
            tx.execute(
                "INSERT INTO usage_ledger(id,tenant_id,asset_id,transfer_job_id,event_type,quantity,idempotency_key,correlation_id) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5,$6,$7,$8::text::uuid) ON CONFLICT (tenant_id,event_type,idempotency_key) DO NOTHING",
                &[&deterministic_uuid(kind, &asset_id), &tenant_id, &asset_id, &job_id, &kind, &quantity, &asset_id, &correlation_id],
            ).await.map_err(redacted_pg_error)?;
        }
        tx.execute(
            "INSERT INTO audit_events(id,tenant_id,action,target_type,target_id,correlation_id,metadata) VALUES ($1::text::uuid,$2::text::uuid,'asset.uploaded','asset',$3,$4::text::uuid,jsonb_build_object('message_id',$5::bigint,'file_name',$6::text,'size_bytes',$7::bigint)) ON CONFLICT (id) DO NOTHING",
            &[&deterministic_uuid("audit-upload", &asset_id), &tenant_id, &asset_id, &correlation_id, &(event.message_id as i64), &event.file_name, &event.size_bytes],
        ).await.map_err(redacted_pg_error)?;
        tx.commit().await.map_err(redacted_pg_error)
    }
}

fn required_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} must be set for PostgreSQL mode"))
}

async fn verify_runtime_role(client: &Client) -> Result<(), String> {
    let control_tables = vec![
        "tenants",
        "assets",
        "transfer_jobs",
        "usage_ledger",
        "audit_events",
    ];
    let row = client
        .query_one(
            "SELECT current_user = session_user, bool_or(r.rolsuper), bool_or(r.rolbypassrls), bool_or(EXISTS (SELECT 1 FROM pg_auth_members m WHERE m.member = r.oid)), bool_or(EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname = ANY($1::text[]) AND c.relowner = r.oid)) FROM pg_roles r WHERE r.rolname IN (current_user, session_user)",
            &[&control_tables],
        )
        .await
        .map_err(redacted_pg_error)?;
    let flags = RuntimeRoleFlags {
        identities_match: row.get(0),
        is_superuser: row.get(1),
        bypasses_rls: row.get(2),
        has_memberships: row.get(3),
        owns_control_tables: row.get(4),
    };
    if !flags.is_safe() {
        return Err("PostgreSQL runtime role failed safety checks".into());
    }
    Ok(())
}

pub(crate) fn deterministic_uuid(namespace: &str, value: &str) -> String {
    let digest = Sha256::digest(format!("telegram-drive:{namespace}:{value}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", bytes[0],bytes[1],bytes[2],bytes[3],bytes[4],bytes[5],bytes[6],bytes[7],bytes[8],bytes[9],bytes[10],bytes[11],bytes[12],bytes[13],bytes[14],bytes[15])
}

pub(crate) fn media_kind(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" => "image",
        "mp4" | "mkv" | "mov" | "webm" | "avi" => "video",
        "mp3" | "wav" | "flac" | "m4a" | "ogg" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" => "document",
        _ => "other",
    }
}

pub(crate) fn redacted_pg_error(error: tokio_postgres::Error) -> String {
    if let Some(db_error) = error.as_db_error() {
        return format!(
            "PostgreSQL control-plane operation failed (closed={}, code={})",
            error.is_closed(),
            db_error.code().code()
        );
    }
    format!(
        "PostgreSQL control-plane operation failed (closed={})",
        error.is_closed()
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_ids_are_stable_and_distinct() {
        assert_eq!(
            deterministic_uuid("tenant", "tenant:a"),
            deterministic_uuid("tenant", "tenant:a")
        );
        assert_ne!(
            deterministic_uuid("tenant", "tenant:a"),
            deterministic_uuid("tenant", "tenant:b")
        );
    }
    #[test]
    fn classifies_media_without_trusting_mime_input() {
        assert_eq!(media_kind("clip.MP4"), "video");
        assert_eq!(media_kind("photo.webp"), "image");
        assert_eq!(media_kind("payload.bin"), "other");
    }

    #[test]
    fn upload_request_fingerprint_is_stable_and_semantically_sensitive() {
        let fingerprint = |size, digest, mode: &str, target: &str| {
            upload_request_fingerprint(
                "movie.mp4",
                size,
                Some(7),
                digest,
                "saga-staging/movie.upload",
                mode,
                target,
            )
        };
        let first = fingerprint(42, "abcd", "bot", "channel:-1007");
        assert_eq!(first, fingerprint(42, "abcd", "bot", "channel:-1007"));
        assert_ne!(first, fingerprint(43, "abcd", "bot", "channel:-1007"));
        assert_ne!(first, fingerprint(42, "efgh", "bot", "channel:-1007"));
        assert_ne!(first, fingerprint(42, "abcd", "user", "channel:-1007"));
        assert_ne!(first, fingerprint(42, "abcd", "bot", "channel:-1008"));
    }
    #[test]
    fn active_upload_lease_blocks_duplicate_worker() {
        assert_eq!(
            classify_upload_replay("running", true, true),
            UploadReplayAction::InProgress
        );
        assert_eq!(
            classify_upload_replay("running", true, false),
            UploadReplayAction::NeedsReconciliation
        );
        assert_eq!(
            classify_upload_replay("finalized", true, false),
            UploadReplayAction::Completed
        );
        assert_eq!(
            classify_upload_replay("telegram_succeeded", true, true),
            UploadReplayAction::InProgress
        );
        assert_eq!(
            classify_upload_replay("telegram_succeeded", true, false),
            UploadReplayAction::ResumeFinalize
        );
        assert_eq!(
            classify_upload_replay("failed", true, false),
            UploadReplayAction::Terminal
        );
        assert_eq!(
            classify_upload_replay("queued", false, false),
            UploadReplayAction::Conflict
        );
    }

    #[test]
    fn postgres_no_tls_is_restricted_to_loopback_hosts() {
        assert!(is_loopback_postgres_host("127.0.0.1"));
        assert!(is_loopback_postgres_host("localhost"));
        assert!(is_loopback_postgres_host("::1"));
        assert!(!is_loopback_postgres_host("db.internal"));
        assert!(!is_loopback_postgres_host("192.168.1.10"));
        assert!(!is_loopback_postgres_host("https://127.0.0.1"));
    }

    #[test]
    fn runtime_role_flags_fail_closed() {
        assert!(RuntimeRoleFlags {
            identities_match: true,
            is_superuser: false,
            bypasses_rls: false,
            owns_control_tables: false,
            has_memberships: false,
        }
        .is_safe());
        assert!(!RuntimeRoleFlags {
            identities_match: true,
            is_superuser: true,
            bypasses_rls: false,
            owns_control_tables: false,
            has_memberships: false,
        }
        .is_safe());
        assert!(!RuntimeRoleFlags {
            identities_match: true,
            is_superuser: false,
            bypasses_rls: true,
            owns_control_tables: false,
            has_memberships: false,
        }
        .is_safe());
        assert!(!RuntimeRoleFlags {
            identities_match: true,
            is_superuser: false,
            bypasses_rls: false,
            owns_control_tables: true,
            has_memberships: false,
        }
        .is_safe());
        assert!(!RuntimeRoleFlags {
            identities_match: true,
            is_superuser: false,
            bypasses_rls: false,
            owns_control_tables: false,
            has_memberships: true,
        }
        .is_safe());
        assert!(!RuntimeRoleFlags {
            identities_match: false,
            is_superuser: false,
            bypasses_rls: false,
            owns_control_tables: false,
            has_memberships: false,
        }
        .is_safe());
    }

    #[tokio::test]
    #[ignore = "requires local PostgreSQL credentials from .env"]
    async fn postgres_upload_roundtrip_is_scoped_and_idempotent() {
        let control_plane = PostgresControlPlane::from_env().expect("postgres config");
        assert!(control_plane.enabled());
        let run_id = uuid::Uuid::new_v4().simple().to_string();
        let owner_id = format!("tenant:postgres-control-plane-{run_id}");
        let existing_tenant_id = deterministic_uuid("preexisting-tenant", &owner_id);
        assert_ne!(
            existing_tenant_id,
            deterministic_uuid("tenant", &owner_id),
            "fixture must exercise an existing non-candidate tenant mapping"
        );
        let message_id = 2_140_000_000 + (uuid::Uuid::new_v4().as_u128() % 1_000_000) as i32;
        let event = UploadControlPlaneEvent {
            owner_id: &owner_id,
            message_id,
            storage_channel_id: -1_000_000_000_999,
            file_name: "integration-test.mp4",
            size_bytes: 4096,
        };

        let mut client = control_plane.connect_checked().await.expect("connect");
        let resolved: String = client
            .query_one(
                "SELECT resolve_legacy_tenant($1,$2::text::uuid,$3)::text",
                &[&owner_id, &existing_tenant_id, &owner_id],
            )
            .await
            .expect("seed existing legacy mapping")
            .get(0);
        assert_eq!(resolved, existing_tenant_id);

        control_plane
            .record_upload(event.clone())
            .await
            .expect("first write");
        control_plane
            .record_upload(event.clone())
            .await
            .expect("idempotent replay");

        let tx = client.transaction().await.expect("transaction");
        tx.execute(
            "SELECT set_config('app.tenant_id', $1, true)",
            &[&existing_tenant_id],
        )
        .await
        .expect("scope");
        let asset_id: String = tx
            .query_one(
                "SELECT id::text FROM assets WHERE storage_channel_id=$1 AND telegram_message_id=$2",
                &[&event.storage_channel_id, &(event.message_id as i64)],
            )
            .await
            .expect("asset id")
            .get(0);
        let ledger_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM usage_ledger WHERE asset_id=$1::text::uuid",
                &[&asset_id],
            )
            .await
            .expect("ledger count")
            .get(0);
        let job_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM transfer_jobs WHERE asset_id=$1::text::uuid",
                &[&asset_id],
            )
            .await
            .expect("job count")
            .get(0);
        assert_eq!(ledger_count, 2);
        assert_eq!(job_count, 1);
        tx.execute("DELETE FROM audit_events WHERE target_id=$1", &[&asset_id])
            .await
            .expect("audit cleanup");
        tx.execute(
            "DELETE FROM usage_ledger WHERE asset_id=$1::text::uuid",
            &[&asset_id],
        )
        .await
        .expect("ledger cleanup");
        tx.execute(
            "DELETE FROM transfer_jobs WHERE asset_id=$1::text::uuid",
            &[&asset_id],
        )
        .await
        .expect("job cleanup");
        tx.execute("DELETE FROM assets WHERE id=$1::text::uuid", &[&asset_id])
            .await
            .expect("asset cleanup");
        tx.execute(
            "DELETE FROM tenants WHERE id=$1::text::uuid",
            &[&existing_tenant_id],
        )
        .await
        .expect("tenant cleanup");
        tx.commit().await.expect("cleanup commit");
    }
}

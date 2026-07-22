use crate::asset_locator::AssetLocatorRecord;
use crate::postgres_control_plane::{deterministic_uuid, redacted_pg_error, PostgresControlPlane};

#[derive(Clone, Debug)]
pub struct DownloadAccountingContext {
    pub tenant_id: String,
    pub asset_id: String,
    pub job_id: String,
    pub correlation_id: String,
    pub attempt_token: String,
}

impl PostgresControlPlane {
    pub async fn begin_download(
        &self,
        owner_id: &str,
        locator: &AssetLocatorRecord,
        request_id: &str,
        expected_bytes: u64,
    ) -> Result<Option<DownloadAccountingContext>, String> {
        if !self.enabled() {
            return Ok(None);
        }
        if owner_id.trim().is_empty() || request_id.trim().is_empty() {
            return Err("DOWNLOAD_ACCOUNTING_CONTEXT_INVALID".to_string());
        }
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        let candidate_tenant_id = deterministic_uuid("tenant", owner_id);
        let tenant_id: String = tx
            .query_one(
                "SELECT resolve_legacy_tenant($1,$2::text::uuid,$3)::text",
                &[&owner_id, &candidate_tenant_id, &owner_id],
            )
            .await
            .map_err(redacted_pg_error)?
            .get(0);
        tx.execute("SELECT set_config('app.tenant_id',$1,true)", &[&tenant_id])
            .await
            .map_err(redacted_pg_error)?;
        let asset_id: String = tx
            .query_opt(
                "SELECT id::text FROM assets WHERE tenant_id=$1::text::uuid AND storage_channel_id=$2 AND telegram_message_id=$3 AND COALESCE(storage_peer_kind,$4)=$4 AND status='ready' AND deleted_at IS NULL",
                &[&tenant_id, &locator.storage_peer_id, &(locator.message_id as i64), &locator.storage_peer_kind],
            )
            .await
            .map_err(redacted_pg_error)?
            .ok_or_else(|| "DOWNLOAD_ASSET_NOT_AUTHORIZED".to_string())?
            .get(0);
        let job_id = deterministic_uuid("download-job", &format!("{tenant_id}:{request_id}"));
        let correlation_id = deterministic_uuid("download-correlation", &job_id);
        let attempt_token = uuid::Uuid::new_v4().to_string();
        let expected_bytes = i64::try_from(expected_bytes).map_err(|_| "DOWNLOAD_SIZE_INVALID")?;
        tx.execute(
            "INSERT INTO transfer_jobs(id,tenant_id,asset_id,direction,idempotency_key,status,bytes_total,bytes_transferred,attempt_count,correlation_id,attempt_token,lease_owner,lease_expires_at) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,'download',$4,'running',$5,0,1,$6::text::uuid,$7::text::uuid,'response-stream',now()+interval '15 minutes') ON CONFLICT(tenant_id,direction,idempotency_key) DO NOTHING",
            &[&job_id, &tenant_id, &asset_id, &request_id, &expected_bytes, &correlation_id, &attempt_token],
        )
        .await
        .map_err(redacted_pg_error)?;
        let row = tx
            .query_one(
                "SELECT id::text,correlation_id::text,attempt_token::text,status FROM transfer_jobs WHERE tenant_id=$1::text::uuid AND direction='download' AND idempotency_key=$2 FOR UPDATE",
                &[&tenant_id, &request_id],
            )
            .await
            .map_err(redacted_pg_error)?;
        let status: String = row.get(3);
        if status != "running" {
            return Err("DOWNLOAD_IDEMPOTENCY_CONFLICT".to_string());
        }
        tx.execute(
            "INSERT INTO audit_events(id,tenant_id,action,target_type,target_id,correlation_id,metadata) VALUES($1::text::uuid,$2::text::uuid,'asset.download.started','asset',$3,$4::text::uuid,jsonb_build_object('message_id',$5::bigint,'storage_peer_id',$6::bigint)) ON CONFLICT(id) DO NOTHING",
            &[&deterministic_uuid("download-start", &job_id), &tenant_id, &asset_id, &correlation_id, &(locator.message_id as i64), &locator.storage_peer_id],
        )
        .await
        .map_err(redacted_pg_error)?;
        let result = DownloadAccountingContext {
            tenant_id,
            asset_id,
            job_id: row.get(0),
            correlation_id: row.get(1),
            attempt_token: row.get(2),
        };
        tx.commit().await.map_err(redacted_pg_error)?;
        Ok(Some(result))
    }

    pub async fn checkpoint_download(
        &self,
        context: &DownloadAccountingContext,
        sequence: u64,
        delta_bytes: usize,
    ) -> Result<(), String> {
        if delta_bytes == 0 || !self.enabled() {
            return Ok(());
        }
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        tx.execute(
            "SELECT set_config('app.tenant_id',$1,true)",
            &[&context.tenant_id],
        )
        .await
        .map_err(redacted_pg_error)?;
        let delta = i64::try_from(delta_bytes).map_err(|_| "DOWNLOAD_DELTA_INVALID")?;
        let event_key = format!("download:{}:checkpoint:{sequence}", context.job_id);
        let inserted = tx
            .execute(
                "INSERT INTO usage_ledger(id,tenant_id,asset_id,transfer_job_id,event_type,quantity,idempotency_key,correlation_id,metadata) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,'download_bytes',$5,$6,$7::text::uuid,jsonb_build_object('sequence',$8::bigint)) ON CONFLICT(tenant_id,event_type,idempotency_key) DO NOTHING",
                &[&deterministic_uuid("download-ledger", &event_key), &context.tenant_id, &context.asset_id, &context.job_id, &delta, &event_key, &context.correlation_id, &(sequence as i64)],
            )
            .await
            .map_err(redacted_pg_error)?;
        if inserted == 1 {
            let updated = tx
                .execute(
                    "UPDATE transfer_jobs SET bytes_transferred=bytes_transferred+$1,lease_expires_at=now()+interval '15 minutes',updated_at=now() WHERE id=$2::text::uuid AND tenant_id=$3::text::uuid AND attempt_token=$4::text::uuid AND status='running'",
                    &[&delta, &context.job_id, &context.tenant_id, &context.attempt_token],
                )
                .await
                .map_err(redacted_pg_error)?;
            if updated != 1 {
                return Err("DOWNLOAD_ACCOUNTING_FENCE_REJECTED".to_string());
            }
        }
        tx.commit().await.map_err(redacted_pg_error)
    }

    pub async fn finish_download(
        &self,
        context: &DownloadAccountingContext,
        error_code: Option<&str>,
    ) -> Result<(), String> {
        if !self.enabled() {
            return Ok(());
        }
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        tx.execute(
            "SELECT set_config('app.tenant_id',$1,true)",
            &[&context.tenant_id],
        )
        .await
        .map_err(redacted_pg_error)?;
        let status = if error_code.is_some() {
            "failed"
        } else {
            "succeeded"
        };
        let updated = tx
            .execute(
                "UPDATE transfer_jobs SET status=$1,error_code=$2,completed_at=now(),lease_owner=NULL,lease_expires_at=NULL,updated_at=now() WHERE id=$3::text::uuid AND tenant_id=$4::text::uuid AND attempt_token=$5::text::uuid AND status='running'",
                &[&status, &error_code, &context.job_id, &context.tenant_id, &context.attempt_token],
            )
            .await
            .map_err(redacted_pg_error)?;
        if updated != 1 {
            return Err("DOWNLOAD_ACCOUNTING_FENCE_REJECTED".to_string());
        }
        tx.commit().await.map_err(redacted_pg_error)
    }
}

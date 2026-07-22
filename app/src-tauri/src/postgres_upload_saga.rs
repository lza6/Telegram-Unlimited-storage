use crate::postgres_control_plane::{
    classify_upload_replay, deterministic_uuid, media_kind, redacted_pg_error, BeginUploadDecision,
    BeginUploadRequest, CompletedUpload, PendingUpload, PostgresControlPlane, UploadReplayAction,
};
use crate::telegram_transport::TelegramUploadReceipt;
use tokio_postgres::Row;

#[derive(Clone)]
pub struct ClaimedUploadRecovery {
    pub pending: PendingUpload,
    pub status: String,
    pub receipt: Option<TelegramUploadReceipt>,
    pub folder_id: Option<i64>,
    pub source_ref: String,
    pub transport_mode: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadRecoveryState {
    pub status: String,
    pub attempt_token: Option<String>,
    pub compensation_status: String,
    pub lease_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SagaNodeCredentials {
    pub node_id: String,
    node_token: String,
}

impl SagaNodeCredentials {
    pub fn from_env(data_dir: &std::path::Path) -> Result<Self, String> {
        let node_id = saga_node_id(data_dir)?;
        let node_token =
            std::env::var("SAGA_NODE_TOKEN").map_err(|_| "SAGA_NODE_TOKEN_REQUIRED".to_string())?;
        Self::new(node_id, node_token)
    }

    fn new(node_id: String, node_token: String) -> Result<Self, String> {
        if !valid_staging_node_id(&node_id) {
            return Err("SAGA_NODE_ID_INVALID".to_string());
        }
        validate_saga_node_token(&node_token)?;
        Ok(Self {
            node_id,
            node_token,
        })
    }

    pub fn token(&self) -> &str {
        &self.node_token
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadRecoveryReleaseOutcome {
    Complete,
    Retry,
    ManualReview,
}

impl UploadRecoveryReleaseOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Retry => "retry",
            Self::ManualReview => "manual_review",
        }
    }
}
impl PostgresControlPlane {
    pub async fn begin_upload(
        &self,
        request: BeginUploadRequest<'_>,
    ) -> Result<BeginUploadDecision, String> {
        validate_begin_request(&request)?;
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        let candidate_tenant_id = deterministic_uuid("tenant", request.owner_id);
        let tenant_id: String = tx
            .query_one(
                "SELECT resolve_legacy_tenant($1,$2::text::uuid,$3)::text",
                &[&request.owner_id, &candidate_tenant_id, &request.owner_id],
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

        let candidate_job_id = deterministic_uuid(
            "upload-job",
            &format!("{}:{}", tenant_id, request.idempotency_key),
        );
        let correlation_id = deterministic_uuid(
            "upload-correlation",
            &format!("{}:{}", tenant_id, request.idempotency_key),
        );
        tx.execute(
            "INSERT INTO transfer_jobs(id,tenant_id,direction,idempotency_key,status,bytes_total,bytes_transferred,attempt_count,correlation_id,request_fingerprint,source_file_name,requested_folder_id,compensation_status,saga_version,request_spec,max_attempts) VALUES ($1::text::uuid,$2::text::uuid,'upload',$3,'pending',$4,0,0,$5::text::uuid,$6,$7,$8,'none',1,jsonb_build_object('source_ref',$9::text,'transport_mode',$10::text,'target',$11::text,'staging_node_id',$12::text),8) ON CONFLICT (tenant_id,direction,idempotency_key) DO NOTHING",
            &[&candidate_job_id, &tenant_id, &request.idempotency_key, &request.size_bytes, &correlation_id, &request.request_fingerprint, &request.file_name, &request.folder_id, &request.source_ref, &request.transport_mode, &request.target, &request.staging_node_id],
        )
        .await
        .map_err(redacted_pg_error)?;

        let row = tx
            .query_one(
                "SELECT id::text,status,request_fingerprint,correlation_id::text,(lease_expires_at IS NOT NULL AND lease_expires_at > now()),asset_id::text,telegram_message_id,storage_peer_id,storage_peer_kind,telegram_file_id,telegram_file_name,telegram_file_size,telegram_mime_type,request_spec->>'staging_node_id',attempt_count,max_attempts,uploader_bot_id,bot_pool_index FROM transfer_jobs WHERE tenant_id=$1::text::uuid AND direction='upload' AND idempotency_key=$2 FOR UPDATE",
                &[&tenant_id, &request.idempotency_key],
            )
            .await
            .map_err(redacted_pg_error)?;
        let job_id: String = row.get(0);
        let status: String = row.get(1);
        let stored_fingerprint: Option<String> = row.get(2);
        let stored_correlation_id: String = row.get(3);
        let lease_active: bool = row.get(4);
        let fingerprint_matches =
            stored_fingerprint.as_deref() == Some(request.request_fingerprint);
        if !fingerprint_matches {
            return Err("IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_REQUEST".into());
        }
        let stored_staging_node_id: Option<String> = row.get(13);
        if matches!(
            status.as_str(),
            "pending"
                | "queued"
                | "retry_wait"
                | "running"
                | "telegram_succeeded"
                | "compensation_pending"
        ) && stored_staging_node_id.as_deref() != Some(request.staging_node_id)
        {
            tx.commit().await.map_err(redacted_pg_error)?;
            return Ok(BeginUploadDecision::NeedsReconciliation {
                job_id,
                correlation_id: stored_correlation_id,
            });
        }
        let attempt_count: i32 = row.get(14);
        let max_attempts: Option<i32> = row.get(15);
        if matches!(status.as_str(), "pending" | "queued" | "retry_wait")
            && attempt_count >= max_attempts.unwrap_or(8)
        {
            tx.execute(
                "UPDATE transfer_jobs SET status='failed',error_code='UPLOAD_RETRY_EXHAUSTED',completed_at=now(),updated_at=now() WHERE id=$1::text::uuid",
                &[&job_id],
            )
            .await
            .map_err(redacted_pg_error)?;
            tx.commit().await.map_err(redacted_pg_error)?;
            return Ok(BeginUploadDecision::Terminal {
                job_id,
                correlation_id: stored_correlation_id,
                status: "failed".to_string(),
            });
        }
        let action = classify_upload_replay(&status, true, lease_active);
        match action {
            UploadReplayAction::Conflict => {
                Err("IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_REQUEST".into())
            }
            UploadReplayAction::InProgress => {
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::InProgress {
                    job_id,
                    correlation_id: stored_correlation_id,
                })
            }
            UploadReplayAction::Completed => {
                let completed = completed_upload_from_begin_row(
                    &tenant_id,
                    &job_id,
                    &stored_correlation_id,
                    &row,
                )?;
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::Completed(completed))
            }
            UploadReplayAction::ResumeFinalize => {
                let attempt_token = uuid::Uuid::new_v4().to_string();
                let lease_owner = format!(
                    "node:{}:request:{}",
                    request.staging_node_id,
                    &stored_correlation_id[..12]
                );
                let updated = tx
                    .execute(
                        "UPDATE transfer_jobs SET attempt_token=$1::text::uuid,lease_owner=$2,lease_expires_at=now()+interval '5 minutes',updated_at=now() WHERE id=$3::text::uuid AND status='telegram_succeeded'",
                        &[&attempt_token, &lease_owner, &job_id],
                    )
                    .await
                    .map_err(redacted_pg_error)?;
                if updated != 1 {
                    return Err("UPLOAD_SAGA_FINALIZE_CLAIM_FAILED".into());
                }
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::ResumeFinalize(PendingUpload {
                    tenant_id,
                    job_id,
                    correlation_id: stored_correlation_id,
                    attempt_token,
                }))
            }
            UploadReplayAction::CompensationRequired => {
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::CompensationRequired {
                    job_id,
                    correlation_id: stored_correlation_id,
                })
            }
            UploadReplayAction::NeedsReconciliation => {
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::NeedsReconciliation {
                    job_id,
                    correlation_id: stored_correlation_id,
                })
            }
            UploadReplayAction::Terminal => {
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::Terminal {
                    job_id,
                    correlation_id: stored_correlation_id,
                    status,
                })
            }
            UploadReplayAction::Acquire => {
                let attempt_token = uuid::Uuid::new_v4().to_string();
                let lease_owner = format!(
                    "node:{}:request:{}",
                    request.staging_node_id,
                    &stored_correlation_id[..12]
                );
                let updated = tx
                    .execute(
                        "UPDATE transfer_jobs SET status='running',attempt_token=$1::text::uuid,lease_owner=$2,lease_expires_at=now()+interval '5 minutes',attempt_count=attempt_count+1,next_attempt_at=NULL,error_code=NULL,error_message=NULL,compensation_status='none',compensation_error=NULL,updated_at=now() WHERE id=$3::text::uuid AND status IN ('pending','queued','retry_wait')",
                        &[&attempt_token, &lease_owner, &job_id],
                    )
                    .await
                    .map_err(redacted_pg_error)?;
                if updated != 1 {
                    return Err("UPLOAD_SAGA_ACQUIRE_FAILED".into());
                }
                tx.commit().await.map_err(redacted_pg_error)?;
                Ok(BeginUploadDecision::Proceed(PendingUpload {
                    tenant_id,
                    job_id,
                    correlation_id: stored_correlation_id,
                    attempt_token,
                }))
            }
        }
    }

    pub async fn record_upload_receipt(
        &self,
        pending: &PendingUpload,
        receipt: &TelegramUploadReceipt,
    ) -> Result<(), String> {
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        set_tenant_scope(&tx, &pending.tenant_id).await?;
        let file_size = i64::try_from(receipt.file_size).map_err(|_| "UPLOAD_SIZE_INVALID")?;
        let bot_pool_index = receipt.bot_pool_index.map(|value| value as i32);
        let updated = tx
            .execute(
                "UPDATE transfer_jobs SET status='telegram_succeeded',telegram_message_id=$1,storage_peer_id=$2,storage_peer_kind=$3,telegram_file_id=$4,telegram_file_name=$5,telegram_file_size=$6,telegram_mime_type=$7,uploader_bot_id=$8,bot_pool_index=$9,bytes_transferred=$6,receipt_recorded_at=now(),lease_expires_at=now()+interval '5 minutes',updated_at=now() WHERE id=$10::text::uuid AND tenant_id=$11::text::uuid AND attempt_token=$12::text::uuid AND status='running'",
                &[&(receipt.message_id as i64), &receipt.storage_peer_id, &receipt.storage_peer_kind, &receipt.telegram_file_id, &receipt.file_name, &file_size, &receipt.mime_type, &receipt.uploader_bot_id, &bot_pool_index, &pending.job_id, &pending.tenant_id, &pending.attempt_token],
            )
            .await
            .map_err(redacted_pg_error)?;
        if updated != 1 {
            return Err("UPLOAD_SAGA_FENCE_REJECTED".into());
        }
        tx.commit().await.map_err(redacted_pg_error)
    }

    pub async fn finalize_upload(
        &self,
        pending: &PendingUpload,
    ) -> Result<CompletedUpload, String> {
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        set_tenant_scope(&tx, &pending.tenant_id).await?;
        let row = tx
            .query_one(
                "SELECT status,source_file_name,bytes_total,telegram_message_id,storage_peer_id,storage_peer_kind,telegram_file_id,telegram_file_name,telegram_file_size,telegram_mime_type,asset_id::text,request_spec->>'transport_mode',uploader_bot_id,bot_pool_index FROM transfer_jobs WHERE id=$1::text::uuid AND tenant_id=$2::text::uuid AND attempt_token=$3::text::uuid FOR UPDATE",
                &[&pending.job_id, &pending.tenant_id, &pending.attempt_token],
            )
            .await
            .map_err(redacted_pg_error)?;
        let status: String = row.get(0);
        if status == "finalized" {
            let completed = completed_upload_from_finalize_row(pending, &row)?;
            tx.commit().await.map_err(redacted_pg_error)?;
            return Ok(completed);
        }
        if status != "telegram_succeeded" {
            return Err("UPLOAD_SAGA_NOT_READY_TO_FINALIZE".into());
        }

        let source_file_name: String = row.get(1);
        let bytes_total: i64 = row.get(2);
        let message_id: i64 = required_value(&row, 3, "telegram_message_id")?;
        let storage_peer_id: i64 = required_value(&row, 4, "storage_peer_id")?;
        let storage_peer_kind: String = required_value(&row, 5, "storage_peer_kind")?;
        let telegram_file_id: Option<String> = row.get(6);
        let file_name = row.get::<_, Option<String>>(7).unwrap_or(source_file_name);
        let file_size = row.get::<_, Option<i64>>(8).unwrap_or(bytes_total);
        let mime_type = row
            .get::<_, Option<String>>(9)
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let transport_mode = row.get::<_, Option<String>>(11).unwrap_or_else(|| {
            if telegram_file_id.is_some() {
                "bot".to_string()
            } else {
                "user".to_string()
            }
        });
        let uploader_bot_id: Option<String> = row.get(12);
        let bot_pool_index: Option<i32> = row.get(13);
        let candidate_asset_id = deterministic_uuid(
            "asset",
            &format!("{}:{}:{}", pending.tenant_id, storage_peer_id, message_id),
        );
        let asset_id: String = tx
            .query_one(
                "INSERT INTO assets(id,tenant_id,telegram_message_id,telegram_file_id,storage_channel_id,storage_peer_kind,transport_mode,uploader_bot_id,bot_pool_index,file_name,mime_type,media_kind,size_bytes,status) VALUES ($1::text::uuid,$2::text::uuid,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'ready') ON CONFLICT (storage_channel_id,telegram_message_id) DO UPDATE SET telegram_file_id=EXCLUDED.telegram_file_id,storage_peer_kind=EXCLUDED.storage_peer_kind,transport_mode=EXCLUDED.transport_mode,uploader_bot_id=EXCLUDED.uploader_bot_id,bot_pool_index=EXCLUDED.bot_pool_index,file_name=EXCLUDED.file_name,mime_type=EXCLUDED.mime_type,size_bytes=EXCLUDED.size_bytes,status='ready' RETURNING id::text",
                &[&candidate_asset_id, &pending.tenant_id, &message_id, &telegram_file_id, &storage_peer_id, &storage_peer_kind, &transport_mode, &uploader_bot_id, &bot_pool_index, &file_name, &mime_type, &media_kind(&file_name), &file_size],
            )
            .await
            .map_err(redacted_pg_error)?
            .get(0);
        for (event_type, quantity) in [("asset_stored", file_size), ("upload_bytes", file_size)] {
            tx.execute(
                "INSERT INTO usage_ledger(id,tenant_id,asset_id,transfer_job_id,event_type,quantity,idempotency_key,correlation_id) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5,$6,$7,$8::text::uuid) ON CONFLICT (tenant_id,event_type,idempotency_key) DO NOTHING",
                &[&deterministic_uuid(event_type, &pending.job_id), &pending.tenant_id, &asset_id, &pending.job_id, &event_type, &quantity, &pending.job_id, &pending.correlation_id],
            )
            .await
            .map_err(redacted_pg_error)?;
        }
        tx.execute(
            "INSERT INTO audit_events(id,tenant_id,action,target_type,target_id,correlation_id,metadata) VALUES ($1::text::uuid,$2::text::uuid,'asset.uploaded','asset',$3,$4::text::uuid,jsonb_build_object('message_id',$5::bigint,'storage_peer_id',$6::bigint,'storage_peer_kind',$7::text,'file_name',$8::text,'size_bytes',$9::bigint)) ON CONFLICT (id) DO NOTHING",
            &[&deterministic_uuid("audit-upload", &pending.job_id), &pending.tenant_id, &asset_id, &pending.correlation_id, &message_id, &storage_peer_id, &storage_peer_kind, &file_name, &file_size],
        )
        .await
        .map_err(redacted_pg_error)?;
        let updated = tx
            .execute(
                "UPDATE transfer_jobs SET asset_id=$1::text::uuid,status='finalized',bytes_transferred=$2,finalized_at=now(),completed_at=now(),lease_owner=NULL,lease_expires_at=NULL,updated_at=now() WHERE id=$3::text::uuid AND attempt_token=$4::text::uuid AND status='telegram_succeeded'",
                &[&asset_id, &file_size, &pending.job_id, &pending.attempt_token],
            )
            .await
            .map_err(redacted_pg_error)?;
        if updated != 1 {
            return Err("UPLOAD_SAGA_FINALIZE_FENCE_REJECTED".into());
        }
        tx.commit().await.map_err(redacted_pg_error)?;
        Ok(CompletedUpload {
            tenant_id: pending.tenant_id.clone(),
            job_id: pending.job_id.clone(),
            correlation_id: pending.correlation_id.clone(),
            asset_id,
            receipt: TelegramUploadReceipt {
                message_id: message_id as i32,
                telegram_file_id,
                file_name,
                file_size: file_size.max(0) as u64,
                mime_type,
                storage_peer_id,
                storage_peer_kind,
                bot_pool_index: bot_pool_index.map(|value| value as u32),
                uploader_bot_id,
            },
        })
    }

    pub async fn renew_upload_lease(&self, pending: &PendingUpload) -> Result<(), String> {
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        set_tenant_scope(&tx, &pending.tenant_id).await?;
        let updated = tx
            .execute(
                "UPDATE transfer_jobs SET lease_expires_at=now()+interval '5 minutes',updated_at=now() WHERE id=$1::text::uuid AND tenant_id=$2::text::uuid AND attempt_token=$3::text::uuid AND status='running'",
                &[&pending.job_id, &pending.tenant_id, &pending.attempt_token],
            )
            .await
            .map_err(redacted_pg_error)?;
        if updated != 1 {
            return Err("UPLOAD_SAGA_HEARTBEAT_FENCE_REJECTED".into());
        }
        tx.commit().await.map_err(redacted_pg_error)
    }
    pub async fn mark_upload_retryable(
        &self,
        pending: &PendingUpload,
        error_code: &str,
    ) -> Result<(), String> {
        self.update_upload_state(
            pending,
            "pending",
            "none",
            Some(error_code),
            None,
            &["running"],
        )
        .await
    }

    pub async fn mark_upload_failed(
        &self,
        pending: &PendingUpload,
        error_code: &str,
    ) -> Result<(), String> {
        self.update_upload_state(
            pending,
            "failed",
            "none",
            Some(error_code),
            None,
            &["running"],
        )
        .await
    }

    pub async fn mark_compensation_pending(
        &self,
        pending: &PendingUpload,
        error_code: &str,
        compensation_error: Option<&str>,
    ) -> Result<(), String> {
        self.update_upload_state(
            pending,
            "compensation_pending",
            if compensation_error.is_some() {
                "reconcile"
            } else {
                "pending"
            },
            Some(error_code),
            compensation_error,
            &["running", "telegram_succeeded", "compensation_pending"],
        )
        .await
    }

    pub async fn mark_compensation_delete_confirmed(
        &self,
        pending: &PendingUpload,
    ) -> Result<(), String> {
        self.update_upload_state(
            pending,
            "compensation_pending",
            "deleted",
            None,
            None,
            &["compensation_pending"],
        )
        .await
    }
    pub async fn mark_compensated(&self, pending: &PendingUpload) -> Result<(), String> {
        self.update_upload_state(
            pending,
            "compensated",
            "deleted",
            None,
            None,
            &["telegram_succeeded", "compensation_pending"],
        )
        .await
    }

    pub async fn claim_upload_recovery(
        &self,
        staging_node_id: &str,
        node_token: &str,
        limit: i32,
    ) -> Result<Vec<ClaimedUploadRecovery>, String> {
        if !valid_staging_node_id(staging_node_id)
            || validate_saga_node_token(node_token).is_err()
            || !(1..=100).contains(&limit)
        {
            return Err("UPLOAD_RECOVERY_CLAIM_INVALID".to_string());
        }
        let client = self.connect_checked().await?;
        let rows = client
            .query(
                "SELECT tenant_id::text,job_id::text,correlation_id::text,status,attempt_token::text,telegram_message_id,storage_peer_id,storage_peer_kind,telegram_file_id,telegram_file_name,telegram_file_size,telegram_mime_type,receipt_recorded_at,requested_folder_id,source_ref,transport_mode,target FROM claim_upload_saga_recovery($1,$2,$3)",
                &[&staging_node_id, &node_token, &limit],
            )
            .await
            .map_err(redacted_pg_error)?;
        rows.into_iter()
            .map(|row| {
                let tenant_id: String = row.get(0);
                let job_id: String = row.get(1);
                let correlation_id: String = row.get(2);
                let status: String = row.get(3);
                let attempt_token: String = row.get(4);
                let message_id: Option<i64> = row.get(5);
                let storage_peer_id: Option<i64> = row.get(6);
                let storage_peer_kind: Option<String> = row.get(7);
                let telegram_file_id: Option<String> = row.get(8);
                let file_name: Option<String> = row.get(9);
                let file_size: Option<i64> = row.get(10);
                let mime_type: Option<String> = row.get(11);
                let receipt = match (message_id, storage_peer_id, storage_peer_kind, file_size) {
                    (
                        Some(message_id),
                        Some(storage_peer_id),
                        Some(storage_peer_kind),
                        Some(file_size),
                    ) => Some(TelegramUploadReceipt {
                        message_id: i32::try_from(message_id)
                            .map_err(|_| "UPLOAD_RECOVERY_MESSAGE_ID_INVALID".to_string())?,
                        telegram_file_id,
                        file_name: file_name.unwrap_or_else(|| "upload.bin".to_string()),
                        file_size: file_size.max(0) as u64,
                        mime_type: mime_type
                            .unwrap_or_else(|| "application/octet-stream".to_string()),
                        storage_peer_id,
                        storage_peer_kind,
                        bot_pool_index: None,
                        uploader_bot_id: None,
                    }),
                    _ => None,
                };
                Ok(ClaimedUploadRecovery {
                    pending: PendingUpload {
                        tenant_id,
                        job_id,
                        correlation_id,
                        attempt_token,
                    },
                    status,
                    receipt,
                    folder_id: row.get(13),
                    source_ref: row.get(14),
                    transport_mode: row.get(15),
                    target: row.get(16),
                })
            })
            .collect()
    }

    pub async fn renew_upload_recovery_lease(
        &self,
        credentials: &SagaNodeCredentials,
        pending: &PendingUpload,
        lease_seconds: i32,
    ) -> Result<(), String> {
        validate_recovery_lease_request(credentials, pending, lease_seconds)?;
        let client = self.connect_checked().await?;
        client
            .query_one(
                "SELECT renew_upload_saga_recovery($1,$2,$3::text::uuid,$4::text::uuid,$5)",
                &[
                    &credentials.node_id,
                    &credentials.node_token,
                    &pending.job_id,
                    &pending.attempt_token,
                    &lease_seconds,
                ],
            )
            .await
            .map_err(redacted_pg_error)?;
        Ok(())
    }

    pub async fn release_upload_recovery(
        &self,
        credentials: &SagaNodeCredentials,
        pending: &PendingUpload,
        outcome: UploadRecoveryReleaseOutcome,
        error: Option<&str>,
    ) -> Result<(), String> {
        validate_recovery_identity(credentials, pending)?;
        let error = error.map(|value| value.chars().take(4000).collect::<String>());
        let client = self.connect_checked().await?;
        client
            .query_one(
                "SELECT release_upload_saga_recovery($1,$2,$3::text::uuid,$4::text::uuid,$5,$6)",
                &[
                    &credentials.node_id,
                    &credentials.node_token,
                    &pending.job_id,
                    &pending.attempt_token,
                    &outcome.as_str(),
                    &error,
                ],
            )
            .await
            .map_err(redacted_pg_error)?;
        Ok(())
    }
    pub async fn upload_recovery_state(
        &self,
        tenant_id: &str,
        job_id: &str,
    ) -> Result<Option<UploadRecoveryState>, String> {
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        set_tenant_scope(&tx, tenant_id).await?;
        let row = tx
            .query_opt(
                "SELECT status,attempt_token::text,compensation_status,(lease_expires_at IS NOT NULL AND lease_expires_at > now()) FROM transfer_jobs WHERE tenant_id=$1::text::uuid AND id=$2::text::uuid AND saga_version=1 AND direction='upload'",
                &[&tenant_id, &job_id],
            )
            .await
            .map_err(redacted_pg_error)?;
        tx.commit().await.map_err(redacted_pg_error)?;
        Ok(row.map(|row| UploadRecoveryState {
            status: row.get(0),
            attempt_token: row.get(1),
            compensation_status: row.get(2),
            lease_active: row.get(3),
        }))
    }
    async fn update_upload_state(
        &self,
        pending: &PendingUpload,
        status: &str,
        compensation_status: &str,
        error_code: Option<&str>,
        compensation_error: Option<&str>,
        allowed_statuses: &[&str],
    ) -> Result<(), String> {
        let mut client = self.connect_checked().await?;
        let tx = client.transaction().await.map_err(redacted_pg_error)?;
        set_tenant_scope(&tx, &pending.tenant_id).await?;
        let updated = tx
            .execute(
                "UPDATE transfer_jobs SET status=$1,compensation_status=$2,error_code=$3,compensation_error=$4,next_attempt_at=CASE WHEN $1='compensation_pending' THEN now() ELSE next_attempt_at END,completed_at=CASE WHEN $1 IN ('failed','compensated') THEN now() ELSE completed_at END,compensated_at=CASE WHEN $1='compensated' THEN now() ELSE compensated_at END,lease_owner=CASE WHEN $1='compensation_pending' AND lease_owner LIKE 'recovery:%' THEN lease_owner ELSE NULL END,lease_expires_at=CASE WHEN $1='compensation_pending' AND lease_owner LIKE 'recovery:%' THEN lease_expires_at ELSE NULL END,updated_at=now() WHERE id=$5::text::uuid AND tenant_id=$6::text::uuid AND attempt_token=$7::text::uuid AND status=ANY($8::text[])",
                &[&status, &compensation_status, &error_code, &compensation_error, &pending.job_id, &pending.tenant_id, &pending.attempt_token, &allowed_statuses],
            )
            .await
            .map_err(redacted_pg_error)?;
        if updated != 1 {
            return Err("UPLOAD_SAGA_STATE_FENCE_REJECTED".into());
        }
        tx.commit().await.map_err(redacted_pg_error)
    }
}

pub fn saga_node_id(data_dir: &std::path::Path) -> Result<String, String> {
    if let Ok(configured) = std::env::var("SAGA_NODE_ID") {
        let configured = configured.trim();
        if valid_staging_node_id(configured) {
            return Ok(configured.to_string());
        }
        return Err("SAGA_NODE_ID_INVALID".to_string());
    }
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "local".to_string());
    let canonical = std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest: String = hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let sanitized: String = host
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    let node_id = format!("{}:{}", sanitized.trim_matches('-'), digest);
    if valid_staging_node_id(&node_id) {
        Ok(node_id)
    } else {
        Err("SAGA_NODE_ID_INVALID".to_string())
    }
}
fn validate_saga_node_token(value: &str) -> Result<(), String> {
    if (32..=4096).contains(&value.len()) {
        Ok(())
    } else {
        Err("SAGA_NODE_TOKEN_INVALID".to_string())
    }
}

fn validate_recovery_identity(
    credentials: &SagaNodeCredentials,
    pending: &PendingUpload,
) -> Result<(), String> {
    if !valid_staging_node_id(&credentials.node_id)
        || validate_saga_node_token(&credentials.node_token).is_err()
        || uuid::Uuid::parse_str(&pending.job_id).is_err()
        || uuid::Uuid::parse_str(&pending.attempt_token).is_err()
    {
        return Err("UPLOAD_RECOVERY_LEASE_INVALID".to_string());
    }
    Ok(())
}

fn validate_recovery_lease_request(
    credentials: &SagaNodeCredentials,
    pending: &PendingUpload,
    lease_seconds: i32,
) -> Result<(), String> {
    validate_recovery_identity(credentials, pending)?;
    if !(30..=900).contains(&lease_seconds) {
        return Err("UPLOAD_RECOVERY_LEASE_INVALID".to_string());
    }
    Ok(())
}
fn valid_staging_node_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}
pub struct UploadRecoveryLeaseHeartbeat {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    ownership_lost: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl UploadRecoveryLeaseHeartbeat {
    pub fn ownership_lost(&self) -> bool {
        self.ownership_lost
            .load(std::sync::atomic::Ordering::Acquire)
    }
}

impl Drop for UploadRecoveryLeaseHeartbeat {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

pub fn spawn_upload_recovery_lease_heartbeat(
    control_plane: PostgresControlPlane,
    credentials: SagaNodeCredentials,
    pending: PendingUpload,
) -> UploadRecoveryLeaseHeartbeat {
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let ownership_lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let lost_in_task = ownership_lost.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = &mut stopped => break,
                _ = interval.tick() => {
                    if let Err(error) = control_plane
                        .renew_upload_recovery_lease(&credentials, &pending, 300)
                        .await
                    {
                        lost_in_task.store(true, std::sync::atomic::Ordering::Release);
                        log::warn!("upload Saga recovery lease heartbeat stopped: {error}");
                        break;
                    }
                }
            }
        }
    });
    UploadRecoveryLeaseHeartbeat {
        stop: Some(stop),
        ownership_lost,
    }
}
pub struct UploadLeaseHeartbeat {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    ownership_lost: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl UploadLeaseHeartbeat {
    pub fn ownership_lost(&self) -> bool {
        self.ownership_lost
            .load(std::sync::atomic::Ordering::Acquire)
    }
}

impl Drop for UploadLeaseHeartbeat {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

pub fn spawn_upload_lease_heartbeat(
    control_plane: PostgresControlPlane,
    pending: PendingUpload,
) -> UploadLeaseHeartbeat {
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let ownership_lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let lost_in_task = ownership_lost.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = &mut stopped => break,
                _ = interval.tick() => {
                    if let Err(error) = control_plane.renew_upload_lease(&pending).await {
                        lost_in_task.store(true, std::sync::atomic::Ordering::Release);
                        log::warn!("upload Saga lease heartbeat stopped: {error}");
                        break;
                    }
                }
            }
        }
    });
    UploadLeaseHeartbeat {
        stop: Some(stop),
        ownership_lost,
    }
}
fn validate_begin_request(request: &BeginUploadRequest<'_>) -> Result<(), String> {
    if request.idempotency_key.trim().is_empty() || request.idempotency_key.len() > 200 {
        return Err("IDEMPOTENCY_KEY_INVALID".into());
    }
    if request.request_fingerprint.len() != 64
        || !request
            .request_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("REQUEST_FINGERPRINT_INVALID".into());
    }
    if request.size_bytes < 0 || request.file_name.trim().is_empty() {
        return Err("UPLOAD_REQUEST_INVALID".into());
    }
    if request.staging_node_id.trim().is_empty()
        || request.staging_node_id.len() > 128
        || !request
            .staging_node_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("STAGING_NODE_ID_INVALID".into());
    }
    Ok(())
}

async fn set_tenant_scope(
    tx: &tokio_postgres::Transaction<'_>,
    tenant_id: &str,
) -> Result<(), String> {
    tx.execute(
        "SELECT set_config('app.tenant_id', $1, true)",
        &[&tenant_id],
    )
    .await
    .map_err(redacted_pg_error)?;
    Ok(())
}

fn required_value<T>(row: &Row, index: usize, field: &str) -> Result<T, String>
where
    T: tokio_postgres::types::FromSqlOwned,
{
    row.try_get::<_, Option<T>>(index)
        .map_err(redacted_pg_error)?
        .ok_or_else(|| format!("UPLOAD_SAGA_RECEIPT_MISSING:{field}"))
}

fn completed_upload_from_begin_row(
    tenant_id: &str,
    job_id: &str,
    correlation_id: &str,
    row: &Row,
) -> Result<CompletedUpload, String> {
    let asset_id: String = required_value(row, 5, "asset_id")?;
    let message_id: i64 = required_value(row, 6, "telegram_message_id")?;
    let storage_peer_id: i64 = required_value(row, 7, "storage_peer_id")?;
    let storage_peer_kind: String = required_value(row, 8, "storage_peer_kind")?;
    let telegram_file_id: Option<String> = row.get(9);
    let file_name: String = required_value(row, 10, "telegram_file_name")?;
    let file_size: i64 = required_value(row, 11, "telegram_file_size")?;
    let mime_type: String = required_value(row, 12, "telegram_mime_type")?;
    Ok(CompletedUpload {
        tenant_id: tenant_id.to_string(),
        job_id: job_id.to_string(),
        correlation_id: correlation_id.to_string(),
        asset_id,
        receipt: TelegramUploadReceipt {
            message_id: message_id as i32,
            telegram_file_id,
            file_name,
            file_size: file_size.max(0) as u64,
            mime_type,
            storage_peer_id,
            storage_peer_kind,
            bot_pool_index: row.get::<_, Option<i32>>(17).map(|value| value as u32),
            uploader_bot_id: row.get(16),
        },
    })
}

fn completed_upload_from_finalize_row(
    pending: &PendingUpload,
    row: &Row,
) -> Result<CompletedUpload, String> {
    let asset_id: String = required_value(row, 10, "asset_id")?;
    let message_id: i64 = required_value(row, 3, "telegram_message_id")?;
    let storage_peer_id: i64 = required_value(row, 4, "storage_peer_id")?;
    let storage_peer_kind: String = required_value(row, 5, "storage_peer_kind")?;
    let telegram_file_id: Option<String> = row.get(6);
    let file_name: String = required_value(row, 7, "telegram_file_name")?;
    let file_size: i64 = required_value(row, 8, "telegram_file_size")?;
    let mime_type: String = required_value(row, 9, "telegram_mime_type")?;
    Ok(CompletedUpload {
        tenant_id: pending.tenant_id.clone(),
        job_id: pending.job_id.clone(),
        correlation_id: pending.correlation_id.clone(),
        asset_id,
        receipt: TelegramUploadReceipt {
            message_id: message_id as i32,
            telegram_file_id,
            file_name,
            file_size: file_size.max(0) as u64,
            mime_type,
            storage_peer_id,
            storage_peer_kind,
            bot_pool_index: row.get::<_, Option<i32>>(13).map(|value| value as u32),
            uploader_bot_id: row.get(12),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres_control_plane::upload_request_fingerprint;

    #[test]
    fn recovery_credentials_fail_closed_and_allow_independent_node_ids() {
        assert_eq!(
            SagaNodeCredentials::new("node-a".to_string(), "short".to_string()),
            Err("SAGA_NODE_TOKEN_INVALID".to_string())
        );
        assert_eq!(
            SagaNodeCredentials::new("bad node".to_string(), "x".repeat(32)),
            Err("SAGA_NODE_ID_INVALID".to_string())
        );
        let first =
            SagaNodeCredentials::new("node-a".to_string(), "a".repeat(32)).expect("first node");
        let second =
            SagaNodeCredentials::new("node-b".to_string(), "b".repeat(32)).expect("second node");
        assert_ne!(first.node_id, second.node_id);
        assert_ne!(first.token(), second.token());
    }

    #[test]
    fn recovery_lease_validation_is_fenced_and_bounded() {
        let credentials =
            SagaNodeCredentials::new("node-a".to_string(), "a".repeat(32)).expect("node");
        let pending = PendingUpload {
            tenant_id: uuid::Uuid::new_v4().to_string(),
            job_id: uuid::Uuid::new_v4().to_string(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            attempt_token: uuid::Uuid::new_v4().to_string(),
        };
        assert!(validate_recovery_lease_request(&credentials, &pending, 30).is_ok());
        assert!(validate_recovery_lease_request(&credentials, &pending, 900).is_ok());
        assert!(validate_recovery_lease_request(&credentials, &pending, 29).is_err());
        assert!(validate_recovery_lease_request(&credentials, &pending, 901).is_err());
        let mut invalid = pending;
        invalid.attempt_token = "not-a-uuid".to_string();
        assert!(validate_recovery_identity(&credentials, &invalid).is_err());
    }
    #[tokio::test]
    async fn upload_recovery_record_is_atomic_and_clearable() {
        let dir = std::env::temp_dir().join(format!("td-saga-recovery-{}", uuid::Uuid::new_v4()));
        ensure_upload_recovery_storage(&dir)
            .await
            .expect("preflight");
        let pending = PendingUpload {
            tenant_id: uuid::Uuid::new_v4().to_string(),
            job_id: uuid::Uuid::new_v4().to_string(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            attempt_token: uuid::Uuid::new_v4().to_string(),
        };
        let receipt = TelegramUploadReceipt {
            message_id: 42,
            telegram_file_id: Some("file-id".to_string()),
            file_name: "safe.bin".to_string(),
            file_size: 4,
            mime_type: "application/octet-stream".to_string(),
            storage_peer_id: -1_000_000_000_042,
            storage_peer_kind: "channel".to_string(),
            bot_pool_index: None,
            uploader_bot_id: None,
        };
        let record = persist_upload_recovery_record(
            &dir,
            &pending,
            &receipt,
            "test-node",
            None,
            "saga-staging/test.upload",
            "bot",
            "channel:-1000000000042",
        )
        .await
        .expect("persist");
        let loaded = load_upload_recovery_records(&dir, "test-node")
            .await
            .expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].job_id, pending.job_id);
        assert_eq!(loaded[0].receipt.message_id, 42);
        assert_eq!(loaded[0].phase, UploadRecoveryPhase::ReceiptPending);
        assert_eq!(record.transport_mode, "bot");
        clear_upload_recovery_records(&dir, &pending.job_id)
            .await
            .expect("clear");
        assert!(load_upload_recovery_records(&dir, "test-node")
            .await
            .expect("load after clear")
            .is_empty());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    #[ignore = "requires local PostgreSQL credentials from .env"]
    async fn upload_saga_is_fenced_idempotent_and_replayable() {
        let control_plane = PostgresControlPlane::from_env().expect("postgres config");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let owner_id = format!("tenant:saga-{suffix}");
        let idempotency_key = format!("upload-{suffix}");
        let fingerprint = upload_request_fingerprint(
            "saga.mp4",
            8192,
            Some(77),
            "abcd",
            "saga-staging/test-upload.bin",
            "bot",
            "channel:-1000000000777",
        );
        let request = BeginUploadRequest {
            owner_id: &owner_id,
            idempotency_key: &idempotency_key,
            request_fingerprint: &fingerprint,
            file_name: "saga.mp4",
            size_bytes: 8192,
            folder_id: Some(77),
            source_ref: "saga-staging/test-upload.bin",
            transport_mode: "bot",
            target: "channel:-1000000000777",
            staging_node_id: "test-node",
        };
        let pending = match control_plane
            .begin_upload(request.clone())
            .await
            .expect("begin")
        {
            BeginUploadDecision::Proceed(pending) => pending,
            other => panic!("expected proceed, got {other:?}"),
        };
        assert!(matches!(
            control_plane
                .begin_upload(request.clone())
                .await
                .expect("replay"),
            BeginUploadDecision::InProgress { .. }
        ));
        let different = BeginUploadRequest {
            request_fingerprint: &upload_request_fingerprint(
                "saga.mp4",
                8193,
                Some(77),
                "abcd",
                "saga-staging/test-upload.bin",
                "bot",
                "channel:-1000000000777",
            ),
            ..request.clone()
        };
        assert_eq!(
            control_plane.begin_upload(different).await.unwrap_err(),
            "IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_REQUEST"
        );

        assert!(matches!(
            control_plane
                .begin_upload(request.clone())
                .await
                .expect("active lease"),
            BeginUploadDecision::InProgress { .. }
        ));
        let mut stale = pending.clone();
        stale.attempt_token = uuid::Uuid::new_v4().to_string();

        let receipt = TelegramUploadReceipt {
            message_id: 2_130_000_000 + (uuid::Uuid::new_v4().as_u128() % 1_000_000) as i32,
            telegram_file_id: Some(format!("file-{suffix}")),
            file_name: "saga.mp4".to_string(),
            file_size: 8192,
            mime_type: "video/mp4".to_string(),
            storage_peer_id: -1_000_000_000_777,
            storage_peer_kind: "channel".to_string(),
            bot_pool_index: None,
            uploader_bot_id: None,
        };
        assert_eq!(
            control_plane
                .record_upload_receipt(&stale, &receipt)
                .await
                .unwrap_err(),
            "UPLOAD_SAGA_FENCE_REJECTED"
        );
        control_plane
            .record_upload_receipt(&pending, &receipt)
            .await
            .expect("receipt");
        let completed = control_plane
            .finalize_upload(&pending)
            .await
            .expect("finalize");
        assert_eq!(completed.receipt, receipt);
        match control_plane
            .begin_upload(request.clone())
            .await
            .expect("completed replay")
        {
            BeginUploadDecision::Completed(replayed) => assert_eq!(replayed, completed),
            other => panic!("expected completed replay, got {other:?}"),
        }

        cleanup_saga_fixture(&control_plane, &completed).await;
    }

    #[tokio::test]
    #[ignore = "requires local PostgreSQL credentials from .env"]
    async fn upload_saga_persists_compensation_state() {
        let control_plane = PostgresControlPlane::from_env().expect("postgres config");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let owner_id = format!("tenant:compensation-{suffix}");
        let idempotency_key = format!("upload-{suffix}");
        let fingerprint = upload_request_fingerprint(
            "failed.bin",
            32,
            None,
            "beef",
            "saga-staging/failed.bin",
            "bot",
            "channel:-1000000000777",
        );
        let pending = match control_plane
            .begin_upload(BeginUploadRequest {
                owner_id: &owner_id,
                idempotency_key: &idempotency_key,
                request_fingerprint: &fingerprint,
                file_name: "failed.bin",
                size_bytes: 32,
                folder_id: None,
                source_ref: "saga-staging/failed.bin",
                transport_mode: "bot",
                target: "channel:-1000000000777",
                staging_node_id: "test-node",
            })
            .await
            .expect("begin")
        {
            BeginUploadDecision::Proceed(pending) => pending,
            other => panic!("expected proceed, got {other:?}"),
        };
        control_plane
            .mark_compensation_pending(&pending, "FINALIZE_FAILED", Some("delete failed"))
            .await
            .expect("mark pending");

        let mut client = control_plane.connect_checked().await.expect("connect");
        let tx = client.transaction().await.expect("inspect transaction");
        set_tenant_scope(&tx, &pending.tenant_id)
            .await
            .expect("scope");
        let row = tx
            .query_one(
                "SELECT status,compensation_status,compensation_error FROM transfer_jobs WHERE id=$1::text::uuid",
                &[&pending.job_id],
            )
            .await
            .expect("inspect compensation");
        assert_eq!(row.get::<_, String>(0), "compensation_pending");
        assert_eq!(row.get::<_, String>(1), "reconcile");
        assert_eq!(
            row.get::<_, Option<String>>(2).as_deref(),
            Some("delete failed")
        );
        tx.commit().await.expect("inspect commit");

        control_plane
            .mark_compensated(&pending)
            .await
            .expect("mark compensated");
        let mut client = control_plane
            .connect_checked()
            .await
            .expect("cleanup connect");
        let tx = client.transaction().await.expect("cleanup transaction");
        set_tenant_scope(&tx, &pending.tenant_id)
            .await
            .expect("scope");
        tx.execute(
            "DELETE FROM transfer_jobs WHERE id=$1::text::uuid",
            &[&pending.job_id],
        )
        .await
        .expect("job cleanup");
        tx.execute(
            "DELETE FROM tenants WHERE id=$1::text::uuid",
            &[&pending.tenant_id],
        )
        .await
        .expect("tenant cleanup");
        tx.commit().await.expect("cleanup commit");
    }

    #[tokio::test]
    #[ignore = "requires local PostgreSQL credentials from .env"]
    async fn upload_saga_concurrent_begin_has_single_owner() {
        let control_plane =
            std::sync::Arc::new(PostgresControlPlane::from_env().expect("postgres config"));
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let owner_id = format!("tenant:concurrent-{suffix}");
        let idempotency_key = format!("upload-{suffix}");
        let source_ref = format!("saga-staging/{suffix}.upload");
        let fingerprint = upload_request_fingerprint(
            "concurrent.bin",
            64,
            None,
            "cafe",
            &source_ref,
            "bot",
            "channel:-1000000000777",
        );
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let control_plane = control_plane.clone();
            let owner_id = owner_id.clone();
            let idempotency_key = idempotency_key.clone();
            let source_ref = source_ref.clone();
            let fingerprint = fingerprint.clone();
            tasks.spawn(async move {
                control_plane
                    .begin_upload(BeginUploadRequest {
                        owner_id: &owner_id,
                        idempotency_key: &idempotency_key,
                        request_fingerprint: &fingerprint,
                        file_name: "concurrent.bin",
                        size_bytes: 64,
                        folder_id: None,
                        source_ref: &source_ref,
                        transport_mode: "bot",
                        target: "channel:-1000000000777",
                        staging_node_id: "test-node",
                    })
                    .await
            });
        }
        let mut proceed = Vec::new();
        let mut in_progress = 0usize;
        while let Some(result) = tasks.join_next().await {
            match result.expect("task join").expect("begin upload") {
                BeginUploadDecision::Proceed(pending) => proceed.push(pending),
                BeginUploadDecision::InProgress { .. } => in_progress += 1,
                other => panic!("unexpected concurrent decision: {other:?}"),
            }
        }
        assert_eq!(proceed.len(), 1);
        assert_eq!(in_progress, 19);
        cleanup_pending_fixture(&control_plane, &proceed[0]).await;
    }

    async fn cleanup_pending_fixture(
        control_plane: &PostgresControlPlane,
        pending: &PendingUpload,
    ) {
        let mut client = control_plane
            .connect_checked()
            .await
            .expect("cleanup connect");
        let tx = client.transaction().await.expect("cleanup transaction");
        set_tenant_scope(&tx, &pending.tenant_id)
            .await
            .expect("scope");
        tx.execute(
            "DELETE FROM transfer_jobs WHERE id=$1::text::uuid",
            &[&pending.job_id],
        )
        .await
        .expect("job cleanup");
        tx.execute(
            "DELETE FROM tenants WHERE id=$1::text::uuid",
            &[&pending.tenant_id],
        )
        .await
        .expect("tenant cleanup");
        tx.commit().await.expect("cleanup commit");
    }
    async fn cleanup_saga_fixture(
        control_plane: &PostgresControlPlane,
        completed: &CompletedUpload,
    ) {
        let mut client = control_plane
            .connect_checked()
            .await
            .expect("cleanup connect");
        let tx = client.transaction().await.expect("cleanup transaction");
        set_tenant_scope(&tx, &completed.tenant_id)
            .await
            .expect("scope");
        tx.execute(
            "DELETE FROM audit_events WHERE correlation_id=$1::text::uuid",
            &[&completed.correlation_id],
        )
        .await
        .expect("audit cleanup");
        tx.execute(
            "DELETE FROM usage_ledger WHERE transfer_job_id=$1::text::uuid",
            &[&completed.job_id],
        )
        .await
        .expect("ledger cleanup");
        tx.execute(
            "DELETE FROM transfer_jobs WHERE id=$1::text::uuid",
            &[&completed.job_id],
        )
        .await
        .expect("job cleanup");
        tx.execute(
            "DELETE FROM assets WHERE id=$1::text::uuid",
            &[&completed.asset_id],
        )
        .await
        .expect("asset cleanup");
        tx.execute(
            "DELETE FROM tenants WHERE id=$1::text::uuid",
            &[&completed.tenant_id],
        )
        .await
        .expect("tenant cleanup");
        tx.commit().await.expect("cleanup commit");
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UploadRecoveryPhase {
    ReceiptPending,
    CompensationPending,
    DeleteConfirmed,
}

impl UploadRecoveryPhase {
    fn ordinal(&self) -> u8 {
        match self {
            Self::ReceiptPending => 10,
            Self::CompensationPending => 20,
            Self::DeleteConfirmed => 30,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct UploadRecoveryRecord {
    pub version: u8,
    pub created_at_ms: i64,
    pub staging_node_id: String,
    pub tenant_id: String,
    pub job_id: String,
    pub correlation_id: String,
    pub attempt_token: String,
    pub folder_id: Option<i64>,
    pub source_ref: String,
    pub transport_mode: String,
    pub target: String,
    pub phase: UploadRecoveryPhase,
    pub last_error: Option<String>,
    pub receipt: TelegramUploadReceipt,
}

pub async fn ensure_upload_recovery_storage(data_dir: &std::path::Path) -> Result<(), String> {
    let directory = data_dir.join("saga-recovery");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|_| "UPLOAD_RECOVERY_DIRECTORY_FAILED".to_string())?;
    let probe = directory.join(format!(".probe-{}", uuid::Uuid::new_v4()));
    let mut file = secure_create_new(&probe).await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, b"ok")
        .await
        .map_err(|_| "UPLOAD_RECOVERY_WRITE_FAILED".to_string())?;
    file.sync_all()
        .await
        .map_err(|_| "UPLOAD_RECOVERY_SYNC_FAILED".to_string())?;
    drop(file);
    tokio::fs::remove_file(&probe)
        .await
        .map_err(|_| "UPLOAD_RECOVERY_CLEANUP_FAILED".to_string())?;
    sync_parent_directory(&directory).await
}

pub async fn persist_upload_recovery_record(
    data_dir: &std::path::Path,
    pending: &PendingUpload,
    receipt: &TelegramUploadReceipt,
    staging_node_id: &str,
    folder_id: Option<i64>,
    source_ref: &str,
    transport_mode: &str,
    target: &str,
) -> Result<UploadRecoveryRecord, String> {
    let record = UploadRecoveryRecord {
        version: 1,
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        staging_node_id: staging_node_id.to_string(),
        tenant_id: pending.tenant_id.clone(),
        job_id: pending.job_id.clone(),
        correlation_id: pending.correlation_id.clone(),
        attempt_token: pending.attempt_token.clone(),
        folder_id,
        source_ref: source_ref.to_string(),
        transport_mode: transport_mode.to_string(),
        target: target.to_string(),
        phase: UploadRecoveryPhase::ReceiptPending,
        last_error: None,
        receipt: receipt.clone(),
    };
    write_upload_recovery_record(data_dir, record.clone()).await?;
    Ok(record)
}

pub async fn advance_upload_recovery_record(
    data_dir: &std::path::Path,
    record: &UploadRecoveryRecord,
    phase: UploadRecoveryPhase,
    last_error: Option<&str>,
) -> Result<UploadRecoveryRecord, String> {
    let mut next = record.clone();
    next.created_at_ms = chrono::Utc::now().timestamp_millis();
    next.phase = phase;
    next.last_error = last_error.map(|value| value.chars().take(512).collect());
    write_upload_recovery_record(data_dir, next.clone()).await?;
    Ok(next)
}
async fn write_upload_recovery_record(
    data_dir: &std::path::Path,
    record: UploadRecoveryRecord,
) -> Result<std::path::PathBuf, String> {
    validate_upload_recovery_record(&record)?;
    let directory = data_dir.join("saga-recovery");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|_| "UPLOAD_RECOVERY_DIRECTORY_FAILED".to_string())?;
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let target = directory.join(format!(
        "{}.{}-{}.json",
        record.job_id,
        record.phase.ordinal(),
        suffix
    ));
    let temporary = directory.join(format!(".{}.tmp", suffix));
    let payload =
        serde_json::to_vec(&record).map_err(|_| "UPLOAD_RECOVERY_SERIALIZE_FAILED".to_string())?;
    let mut file = secure_create_new(&temporary).await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, &payload)
        .await
        .map_err(|_| "UPLOAD_RECOVERY_WRITE_FAILED".to_string())?;
    file.sync_all()
        .await
        .map_err(|_| "UPLOAD_RECOVERY_SYNC_FAILED".to_string())?;
    drop(file);
    atomic_promote(&temporary, &target).await?;
    sync_parent_directory(&directory).await?;
    Ok(target)
}

pub async fn load_upload_recovery_records(
    data_dir: &std::path::Path,
    staging_node_id: &str,
) -> Result<Vec<UploadRecoveryRecord>, String> {
    let directory = data_dir.join("saga-recovery");
    let mut entries = match tokio::fs::read_dir(&directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("UPLOAD_RECOVERY_DIRECTORY_READ_FAILED".to_string()),
    };
    let mut latest = std::collections::HashMap::<String, UploadRecoveryRecord>::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|_| "UPLOAD_RECOVERY_DIRECTORY_READ_FAILED".to_string())?
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let payload = match tokio::fs::read(&path).await {
            Ok(payload) => payload,
            Err(_) => continue,
        };
        let record: UploadRecoveryRecord = match serde_json::from_slice(&payload) {
            Ok(record) => record,
            Err(_) => {
                log::error!(
                    "upload Saga recovery journal is malformed: {}",
                    path.display()
                );
                continue;
            }
        };
        if validate_upload_recovery_record(&record).is_err()
            || record.staging_node_id != staging_node_id
        {
            continue;
        }
        let replace = latest.get(&record.job_id).map_or(true, |current| {
            (record.phase.ordinal(), record.created_at_ms)
                > (current.phase.ordinal(), current.created_at_ms)
        });
        if replace {
            latest.insert(record.job_id.clone(), record);
        }
    }
    Ok(latest.into_values().collect())
}

pub async fn clear_upload_recovery_records(
    data_dir: &std::path::Path,
    job_id: &str,
) -> Result<(), String> {
    uuid::Uuid::parse_str(job_id).map_err(|_| "UPLOAD_RECOVERY_JOB_ID_INVALID".to_string())?;
    let directory = data_dir.join("saga-recovery");
    let mut entries = match tokio::fs::read_dir(&directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("UPLOAD_RECOVERY_DIRECTORY_READ_FAILED".to_string()),
    };
    let prefix = format!("{job_id}.");
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|_| "UPLOAD_RECOVERY_DIRECTORY_READ_FAILED".to_string())?
    {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with(&prefix) {
            tokio::fs::remove_file(entry.path())
                .await
                .map_err(|_| "UPLOAD_RECOVERY_CLEANUP_FAILED".to_string())?;
        }
    }
    sync_parent_directory(&directory).await
}

fn validate_upload_recovery_record(record: &UploadRecoveryRecord) -> Result<(), String> {
    if record.version != 1
        || record.staging_node_id.trim().is_empty()
        || uuid::Uuid::parse_str(&record.tenant_id).is_err()
        || uuid::Uuid::parse_str(&record.job_id).is_err()
        || uuid::Uuid::parse_str(&record.correlation_id).is_err()
        || uuid::Uuid::parse_str(&record.attempt_token).is_err()
        || record.source_ref.trim().is_empty()
        || !matches!(record.transport_mode.as_str(), "bot" | "user")
        || record.target.trim().is_empty()
    {
        return Err("UPLOAD_RECOVERY_RECORD_INVALID".to_string());
    }
    Ok(())
}

async fn secure_create_new(path: &std::path::Path) -> Result<tokio::fs::File, String> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .await
        .map_err(|_| "UPLOAD_RECOVERY_WRITE_FAILED".to_string())
}

pub async fn atomic_promote(
    temporary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), String> {
    let temporary = temporary.to_path_buf();
    let target = target.to_path_buf();
    tokio::task::spawn_blocking(move || atomic_promote_blocking(&temporary, &target))
        .await
        .map_err(|_| "UPLOAD_RECOVERY_PROMOTE_FAILED".to_string())?
}

#[cfg(not(windows))]
fn atomic_promote_blocking(
    temporary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), String> {
    std::fs::rename(temporary, target).map_err(|_| "UPLOAD_RECOVERY_PROMOTE_FAILED".to_string())
}

#[cfg(windows)]
fn atomic_promote_blocking(
    temporary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;
    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let existing: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let new: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe { MoveFileExW(existing.as_ptr(), new.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if result == 0 {
        Err("UPLOAD_RECOVERY_PROMOTE_FAILED".to_string())
    } else {
        Ok(())
    }
}
pub async fn sync_parent_directory(directory: &std::path::Path) -> Result<(), String> {
    let directory = directory.to_path_buf();
    tokio::task::spawn_blocking(move || sync_directory_blocking(&directory))
        .await
        .map_err(|_| "UPLOAD_RECOVERY_DIRECTORY_SYNC_FAILED".to_string())?
}

#[cfg(unix)]
fn sync_directory_blocking(directory: &std::path::Path) -> Result<(), String> {
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| "UPLOAD_RECOVERY_DIRECTORY_SYNC_FAILED".to_string())
}

#[cfg(windows)]
fn sync_directory_blocking(_directory: &std::path::Path) -> Result<(), String> {
    // Journal and staging promotion use MoveFileExW(MOVEFILE_WRITE_THROUGH),
    // which is the Windows durability primitive for the directory entry.
    Ok(())
}

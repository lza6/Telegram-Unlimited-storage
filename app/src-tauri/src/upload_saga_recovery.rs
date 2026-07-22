use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::commands::TelegramState;
use crate::postgres_control_plane::{PendingUpload, PostgresControlPlane};
use crate::postgres_upload_saga::{
    advance_upload_recovery_record, clear_upload_recovery_records, load_upload_recovery_records,
    persist_upload_recovery_record, spawn_upload_recovery_lease_heartbeat, ClaimedUploadRecovery,
    SagaNodeCredentials, UploadRecoveryPhase, UploadRecoveryRecord, UploadRecoveryReleaseOutcome,
};
use crate::server_config::ServerConfig;
use crate::telegram_transport::TransportHandle;
use crate::vpn_optimizer::NetworkConfig;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadRecoveryPassSummary {
    pub journal_seen: usize,
    pub database_claimed: usize,
    pub recovered: usize,
    pub deferred: usize,
    pub failed: usize,
}

pub fn spawn_upload_saga_recovery(
    config: Arc<ServerConfig>,
    tg_state: Arc<TelegramState>,
    net_config: Arc<NetworkConfig>,
    db: crate::db::DbConnection,
    transport: Arc<TransportHandle>,
) -> Option<tokio::task::JoinHandle<()>> {
    let control_plane = match PostgresControlPlane::from_env() {
        Ok(control_plane) if control_plane.enabled() => control_plane,
        Ok(_) => return None,
        Err(error) => {
            log::error!("upload Saga recovery disabled: {error}");
            return None;
        }
    };
    let credentials = match SagaNodeCredentials::from_env(&config.data_dir) {
        Ok(credentials) => credentials,
        Err(error) => {
            log::error!("upload Saga recovery node credentials are invalid: {error}");
            return None;
        }
    };
    let interval_secs = std::env::var("UPLOAD_SAGA_RECOVERY_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value >= 5)
        .unwrap_or(30);
    Some(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            let summary = run_upload_saga_recovery_pass(
                &control_plane,
                &credentials,
                &config,
                &tg_state,
                &net_config,
                &db,
                &transport,
            )
            .await;
            if summary.recovered > 0 || summary.failed > 0 {
                log::info!(
                    "upload Saga recovery: journal_seen={}, database_claimed={}, recovered={}, deferred={}, failed={}",
                    summary.journal_seen,
                    summary.database_claimed,
                    summary.recovered,
                    summary.deferred,
                    summary.failed
                );
            }
        }
    }))
}

pub async fn run_upload_saga_recovery_pass(
    control_plane: &PostgresControlPlane,
    credentials: &SagaNodeCredentials,
    config: &ServerConfig,
    tg_state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    db: &crate::db::DbConnection,
    transport: &TransportHandle,
) -> UploadRecoveryPassSummary {
    let mut summary = UploadRecoveryPassSummary::default();
    match load_upload_recovery_records(&config.data_dir, &credentials.node_id).await {
        Ok(records) => {
            summary.journal_seen = records.len();
            for record in records {
                match recover_journal_record(
                    control_plane,
                    &record,
                    config,
                    tg_state,
                    net_config,
                    db,
                    transport,
                )
                .await
                {
                    Ok(true) => summary.recovered += 1,
                    Ok(false) => summary.deferred += 1,
                    Err(error) => {
                        summary.failed += 1;
                        log::warn!(
                            "upload Saga journal recovery failed for job {}: {}",
                            record.job_id,
                            error
                        );
                    }
                }
            }
        }
        Err(error) => {
            summary.failed += 1;
            log::warn!("upload Saga journal scan failed: {error}");
        }
    }

    match control_plane
        .claim_upload_recovery(&credentials.node_id, credentials.token(), 25)
        .await
    {
        Ok(claimed) => {
            summary.database_claimed = claimed.len();
            for job in claimed {
                let heartbeat = spawn_upload_recovery_lease_heartbeat(
                    control_plane.clone(),
                    credentials.clone(),
                    job.pending.clone(),
                );
                let result = recover_claimed_job(
                    control_plane,
                    credentials,
                    &job,
                    config,
                    tg_state,
                    net_config,
                    db,
                    transport,
                )
                .await;
                let ownership_lost = heartbeat.ownership_lost();
                drop(heartbeat);
                let result = if ownership_lost {
                    Err("UPLOAD_RECOVERY_LEASE_LOST".to_string())
                } else {
                    result
                };
                match result {
                    Ok(()) => summary.recovered += 1,
                    Err(error) => {
                        let outcome = recovery_release_outcome(&error);
                        match control_plane
                            .release_upload_recovery(
                                credentials,
                                &job.pending,
                                outcome,
                                Some(&error),
                            )
                            .await
                        {
                            Ok(()) if outcome == UploadRecoveryReleaseOutcome::Retry => {
                                summary.deferred += 1;
                            }
                            Ok(()) => summary.failed += 1,
                            Err(release_error) => {
                                summary.failed += 1;
                                log::warn!(
                                    "upload Saga recovery lease release failed for job {}: {}",
                                    job.pending.job_id,
                                    release_error
                                );
                            }
                        }
                        log::warn!(
                            "upload Saga database recovery failed for job {}: {}",
                            job.pending.job_id,
                            error
                        );
                    }
                }
            }
        }
        Err(error) => {
            summary.failed += 1;
            log::warn!("upload Saga database claim failed: {error}");
        }
    }
    summary
}

fn recovery_release_outcome(error: &str) -> UploadRecoveryReleaseOutcome {
    if matches!(
        error,
        "UPLOAD_RECOVERY_RECEIPT_MISSING"
            | "UPLOAD_RECOVERY_JOB_MISSING"
            | "SAGA_SOURCE_REF_INVALID"
            | "UPLOAD_RECOVERY_LEASE_LOST"
    ) || error.starts_with("UPLOAD_RECOVERY_STATE_UNSUPPORTED:")
    {
        UploadRecoveryReleaseOutcome::ManualReview
    } else {
        UploadRecoveryReleaseOutcome::Retry
    }
}
async fn recover_journal_record(
    control_plane: &PostgresControlPlane,
    record: &UploadRecoveryRecord,
    config: &ServerConfig,
    tg_state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    db: &crate::db::DbConnection,
    transport: &TransportHandle,
) -> Result<bool, String> {
    let Some(state) = control_plane
        .upload_recovery_state(&record.tenant_id, &record.job_id)
        .await?
    else {
        return Err("UPLOAD_RECOVERY_JOB_MISSING".to_string());
    };
    if matches!(state.status.as_str(), "finalized" | "compensated") {
        clear_upload_recovery_records(&config.data_dir, &record.job_id).await?;
        remove_staged_source(config, &record.source_ref).await?;
        return Ok(true);
    }
    if state.lease_active {
        return Ok(false);
    }
    if state.attempt_token.as_deref() != Some(record.attempt_token.as_str()) {
        return Ok(false);
    }
    let pending = PendingUpload {
        tenant_id: record.tenant_id.clone(),
        job_id: record.job_id.clone(),
        correlation_id: record.correlation_id.clone(),
        attempt_token: record.attempt_token.clone(),
    };
    if record.phase == UploadRecoveryPhase::DeleteConfirmed
        || state.compensation_status == "deleted"
    {
        control_plane.mark_compensated(&pending).await?;
        clear_upload_recovery_records(&config.data_dir, &record.job_id).await?;
        remove_staged_source(config, &record.source_ref).await?;
        return Ok(true);
    }
    if record.phase == UploadRecoveryPhase::CompensationPending
        || state.status == "compensation_pending"
    {
        compensate_from_record(
            control_plane,
            &pending,
            record,
            "RECOVERY_COMPENSATION",
            config,
            tg_state,
            net_config,
            db,
            transport,
        )
        .await?;
        return Ok(true);
    }
    if state.status == "running" {
        control_plane
            .record_upload_receipt(&pending, &record.receipt)
            .await?;
    } else if state.status != "telegram_succeeded" {
        return Err(format!(
            "UPLOAD_RECOVERY_STATE_UNSUPPORTED:{}",
            state.status
        ));
    }
    match control_plane.finalize_upload(&pending).await {
        Ok(_) => {
            clear_upload_recovery_records(&config.data_dir, &record.job_id).await?;
            remove_staged_source(config, &record.source_ref).await?;
            Ok(true)
        }
        Err(_) => {
            compensate_from_record(
                control_plane,
                &pending,
                record,
                "RECOVERY_FINALIZE_FAILED",
                config,
                tg_state,
                net_config,
                db,
                transport,
            )
            .await?;
            Ok(true)
        }
    }
}

async fn recover_claimed_job(
    control_plane: &PostgresControlPlane,
    credentials: &SagaNodeCredentials,
    job: &ClaimedUploadRecovery,
    config: &ServerConfig,
    tg_state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    db: &crate::db::DbConnection,
    transport: &TransportHandle,
) -> Result<(), String> {
    let Some(receipt) = &job.receipt else {
        return Err("UPLOAD_RECOVERY_RECEIPT_MISSING".to_string());
    };
    let state = control_plane
        .upload_recovery_state(&job.pending.tenant_id, &job.pending.job_id)
        .await?
        .ok_or_else(|| "UPLOAD_RECOVERY_JOB_MISSING".to_string())?;
    if state.compensation_status == "deleted" {
        control_plane.mark_compensated(&job.pending).await?;
        clear_upload_recovery_records(&config.data_dir, &job.pending.job_id).await?;
        remove_staged_source(config, &job.source_ref).await?;
        return Ok(());
    }
    if job.status == "telegram_succeeded" {
        if control_plane.finalize_upload(&job.pending).await.is_ok() {
            clear_upload_recovery_records(&config.data_dir, &job.pending.job_id).await?;
            remove_staged_source(config, &job.source_ref).await?;
            return Ok(());
        }
    }
    let record = persist_upload_recovery_record(
        &config.data_dir,
        &job.pending,
        receipt,
        &credentials.node_id,
        job.folder_id,
        &job.source_ref,
        &job.transport_mode,
        &job.target,
    )
    .await?;
    compensate_from_record(
        control_plane,
        &job.pending,
        &record,
        "DATABASE_RECOVERY_COMPENSATION",
        config,
        tg_state,
        net_config,
        db,
        transport,
    )
    .await
}

async fn compensate_from_record(
    control_plane: &PostgresControlPlane,
    pending: &PendingUpload,
    record: &UploadRecoveryRecord,
    error_code: &str,
    config: &ServerConfig,
    tg_state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    db: &crate::db::DbConnection,
    transport: &TransportHandle,
) -> Result<(), String> {
    let compensation_record = advance_upload_recovery_record(
        &config.data_dir,
        record,
        UploadRecoveryPhase::CompensationPending,
        Some(error_code),
    )
    .await?;
    control_plane
        .mark_compensation_pending(pending, error_code, None)
        .await?;
    match crate::http_upload::compensate_uploaded_receipt(
        &record.receipt,
        record.folder_id,
        tg_state,
        net_config,
        config,
        db,
        transport,
        &record.transport_mode,
    )
    .await
    {
        Ok(()) => {
            advance_upload_recovery_record(
                &config.data_dir,
                &compensation_record,
                UploadRecoveryPhase::DeleteConfirmed,
                None,
            )
            .await?;
            control_plane
                .mark_compensation_delete_confirmed(pending)
                .await?;
            control_plane.mark_compensated(pending).await?;
            clear_upload_recovery_records(&config.data_dir, &pending.job_id).await?;
            remove_staged_source(config, &record.source_ref).await
        }
        Err(error) => {
            let error: String = error.chars().take(512).collect();
            advance_upload_recovery_record(
                &config.data_dir,
                &compensation_record,
                UploadRecoveryPhase::CompensationPending,
                Some(&error),
            )
            .await?;
            control_plane
                .mark_compensation_pending(
                    pending,
                    error_code,
                    Some(&format!("Telegram delete failed: {error}")),
                )
                .await?;
            Err("UPLOAD_COMPENSATION_PENDING".to_string())
        }
    }
}

async fn remove_staged_source(config: &ServerConfig, source_ref: &str) -> Result<(), String> {
    let path = resolve_staged_source(&config.data_dir, source_ref)?;
    match tokio::fs::remove_file(&path).await {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                crate::postgres_upload_saga::sync_parent_directory(parent).await?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("SAGA_STAGING_CLEANUP_FAILED".to_string()),
    }
}

fn resolve_staged_source(data_dir: &Path, source_ref: &str) -> Result<PathBuf, String> {
    let relative = Path::new(source_ref);
    let mut components = relative.components();
    if components.next() != Some(Component::Normal(std::ffi::OsStr::new("saga-staging")))
        || components.any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("SAGA_SOURCE_REF_INVALID".to_string());
    }
    Ok(data_dir.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_errors_choose_backoff_or_manual_review() {
        assert_eq!(
            recovery_release_outcome("POSTGRES_UNAVAILABLE"),
            UploadRecoveryReleaseOutcome::Retry
        );
        assert_eq!(
            recovery_release_outcome("UPLOAD_COMPENSATION_PENDING"),
            UploadRecoveryReleaseOutcome::Retry
        );
        assert_eq!(
            recovery_release_outcome("UPLOAD_RECOVERY_RECEIPT_MISSING"),
            UploadRecoveryReleaseOutcome::ManualReview
        );
        assert_eq!(
            recovery_release_outcome("UPLOAD_RECOVERY_STATE_UNSUPPORTED:running-away"),
            UploadRecoveryReleaseOutcome::ManualReview
        );
        assert_eq!(
            recovery_release_outcome("UPLOAD_RECOVERY_LEASE_LOST"),
            UploadRecoveryReleaseOutcome::ManualReview
        );
    }
    #[test]
    fn staged_source_resolution_rejects_escape() {
        let root = Path::new("C:/data");
        assert!(resolve_staged_source(root, "saga-staging/a.upload").is_ok());
        assert!(resolve_staged_source(root, "../escape.upload").is_err());
        assert!(resolve_staged_source(root, "saga-staging/../escape.upload").is_err());
        assert!(resolve_staged_source(root, "other/a.upload").is_err());
    }
}

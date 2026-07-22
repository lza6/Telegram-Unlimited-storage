use crate::{
    postgres_control_plane::PostgresControlPlane, postgres_upload_saga::SagaNodeCredentials,
};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};
use tokio::sync::{oneshot, Notify};

const SCHEDULER_LEASE_SECONDS: u32 = 300;
const SCHEDULER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const SCHEDULER_RENEW_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerResourceSet {
    keys: Vec<String>,
}
impl SchedulerResourceSet {
    pub fn transfer(
        transport: &str,
        method: &str,
        tenant: &str,
        direction: &str,
        bot: Option<&str>,
        peer: Option<(&str, &str, i64)>,
    ) -> Result<Self, String> {
        let mut k = vec![
            format!("global:{}:{}", seg(transport)?, seg(method)?),
            format!("tenant:{}:{}", uid(tenant)?, seg(direction)?),
        ];
        if let Some(b) = bot {
            k.push(format!("bot:{}:{}", seg(b)?, seg(method)?));
        }
        if let Some((t, kind, id)) = peer {
            k.push(format!(
                "peer:{}:{}:{}:{}",
                seg(t)?,
                seg(kind)?,
                id,
                seg(method)?
            ));
        }
        validate(&k)?;
        Ok(Self { keys: k })
    }
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerLease {
    pub lease_id: String,
    pub attempt_token: String,
    pub fence_token: i64,
    pub lease_expires_at: String,
    pub resource_keys: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerOutcome {
    Success,
    Retry { after_seconds: u32 },
    DeadLetter,
    ManualReview,
    Cancelled,
}
impl SchedulerOutcome {
    fn parts(self) -> Result<(&'static str, Option<i32>), String> {
        Ok(match self {
            Self::Success => ("success", None),
            Self::Retry { after_seconds } if (1..=86400).contains(&after_seconds) => {
                ("retry", Some(after_seconds as i32))
            }
            Self::Retry { .. } => return Err("SCHEDULER_RETRY_DELAY_INVALID".into()),
            Self::DeadLetter => ("dead_letter", None),
            Self::ManualReview => ("manual_review", None),
            Self::Cancelled => ("cancelled", None),
        })
    }
}

const LEASE_ACTIVE: u8 = 0;
const LEASE_FINALIZING: u8 = 1;
const LEASE_FINISHED: u8 = 2;
const LEASE_LOST: u8 = 3;

struct SchedulerLeaseLifecycle {
    state: AtomicU8,
    result: Mutex<Option<String>>,
    changed: Notify,
}

impl SchedulerLeaseLifecycle {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(LEASE_ACTIVE),
            result: Mutex::new(None),
            changed: Notify::new(),
        }
    }

    fn is_lost(&self) -> bool {
        self.state.load(Ordering::Acquire) == LEASE_LOST
    }

    fn ensure_owned(&self) -> Result<(), String> {
        if self.is_lost() {
            Err("SCHEDULER_LEASE_LOST".to_string())
        } else {
            Ok(())
        }
    }

    fn mark_lost(&self) -> bool {
        loop {
            match self.state.load(Ordering::Acquire) {
                LEASE_ACTIVE | LEASE_FINALIZING => {
                    let current = self.state.load(Ordering::Acquire);
                    if !matches!(current, LEASE_ACTIVE | LEASE_FINALIZING) {
                        continue;
                    }
                    if self
                        .state
                        .compare_exchange(current, LEASE_LOST, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        self.changed.notify_waiters();
                        return true;
                    }
                }
                LEASE_FINISHED | LEASE_LOST => return false,
                _ => return false,
            }
        }
    }

    async fn claim_finish(&self) -> Result<bool, String> {
        loop {
            match self.state.load(Ordering::Acquire) {
                LEASE_ACTIVE => {
                    if self
                        .state
                        .compare_exchange(
                            LEASE_ACTIVE,
                            LEASE_FINALIZING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return Ok(true);
                    }
                }
                LEASE_FINALIZING => {
                    let changed = self.changed.notified();
                    if self.state.load(Ordering::Acquire) == LEASE_FINALIZING {
                        changed.await;
                    }
                }
                LEASE_FINISHED => return Ok(false),
                LEASE_LOST => return Err("SCHEDULER_LEASE_LOST".to_string()),
                _ => return Err("SCHEDULER_LEASE_STATE_INVALID".to_string()),
            }
        }
    }

    fn claim_drop_finish(&self) -> bool {
        self.state
            .compare_exchange(
                LEASE_ACTIVE,
                LEASE_FINALIZING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn mark_finish_failed(&self) {
        if self
            .state
            .compare_exchange(
                LEASE_FINALIZING,
                LEASE_ACTIVE,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.changed.notify_waiters();
        }
    }

    fn mark_finished(&self, result: String) {
        *self.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        self.state.store(LEASE_FINISHED, Ordering::Release);
        self.changed.notify_waiters();
    }

    fn finished_result(&self) -> Option<String> {
        self.result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    async fn wait_for_loss(&self) {
        loop {
            if self.is_lost() {
                return;
            }
            let changed = self.changed.notified();
            if self.is_lost() {
                return;
            }
            changed.await;
        }
    }
}

#[async_trait::async_trait]
pub(crate) trait SchedulerLeaseBackend: Send + Sync {
    async fn renew(&self, lease: SchedulerLease, seconds: u32) -> Result<SchedulerLease, String>;

    async fn finish(
        &self,
        lease: SchedulerLease,
        outcome: SchedulerOutcome,
        code: Option<String>,
        message: Option<String>,
    ) -> Result<String, String>;
}

struct PostgresSchedulerLeaseBackend {
    control_plane: PostgresControlPlane,
    credentials: SagaNodeCredentials,
}

#[async_trait::async_trait]
impl SchedulerLeaseBackend for PostgresSchedulerLeaseBackend {
    async fn renew(
        &self,
        mut lease: SchedulerLease,
        seconds: u32,
    ) -> Result<SchedulerLease, String> {
        self.control_plane
            .renew_scheduler_lease(&self.credentials, &mut lease, seconds)
            .await?;
        Ok(lease)
    }

    async fn finish(
        &self,
        lease: SchedulerLease,
        outcome: SchedulerOutcome,
        code: Option<String>,
        message: Option<String>,
    ) -> Result<String, String> {
        self.control_plane
            .finish_scheduler_lease(
                &self.credentials,
                &lease,
                outcome,
                code.as_deref(),
                message.as_deref(),
            )
            .await
    }
}

struct FinishAttempt {
    lifecycle: Arc<SchedulerLeaseLifecycle>,
    resolved: bool,
}

impl FinishAttempt {
    fn new(lifecycle: Arc<SchedulerLeaseLifecycle>) -> Self {
        Self {
            lifecycle,
            resolved: false,
        }
    }

    fn finish(mut self, result: String) {
        self.lifecycle.mark_finished(result);
        self.resolved = true;
    }
}

impl Drop for FinishAttempt {
    fn drop(&mut self) {
        if !self.resolved {
            self.lifecycle.mark_finish_failed();
        }
    }
}

struct SchedulerLeaseGuardInner {
    backend: Arc<dyn SchedulerLeaseBackend>,
    lease: Mutex<SchedulerLease>,
    lifecycle: Arc<SchedulerLeaseLifecycle>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    lease_seconds: u32,
    drop_outcome: SchedulerOutcome,
    drop_code: &'static str,
}

impl SchedulerLeaseGuardInner {
    fn stop_heartbeat(&self) {
        if let Some(stop) = self.stop.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for SchedulerLeaseGuardInner {
    fn drop(&mut self) {
        self.stop_heartbeat();
        if !self.lifecycle.claim_drop_finish() {
            return;
        }

        let backend = self.backend.clone();
        let lease = self.lease.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let lifecycle = self.lifecycle.clone();
        let outcome = self.drop_outcome;
        let code = self.drop_code.to_string();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            lifecycle.mark_lost();
            log::error!("scheduler lease drop finalizer has no Tokio runtime");
            return;
        };
        runtime.spawn(async move {
            match backend.finish(lease, outcome, Some(code), None).await {
                Ok(result) => lifecycle.mark_finished(result),
                Err(error) => {
                    lifecycle.mark_lost();
                    log::error!("scheduler lease drop finalization failed: {error}");
                }
            }
        });
    }
}

#[derive(Clone)]
pub struct SchedulerLeaseGuard {
    inner: Arc<SchedulerLeaseGuardInner>,
}

impl SchedulerLeaseGuard {
    pub fn start_upload(
        control_plane: PostgresControlPlane,
        credentials: SagaNodeCredentials,
        lease: SchedulerLease,
    ) -> Self {
        Self::start(
            lease,
            Arc::new(PostgresSchedulerLeaseBackend {
                control_plane,
                credentials,
            }),
            SCHEDULER_LEASE_SECONDS,
            SCHEDULER_HEARTBEAT_INTERVAL,
            SCHEDULER_RENEW_TIMEOUT,
            SchedulerOutcome::Retry { after_seconds: 5 },
            "UPLOAD_SCHEDULER_GUARD_DROPPED",
        )
    }

    pub fn start_download(
        control_plane: PostgresControlPlane,
        credentials: SagaNodeCredentials,
        lease: SchedulerLease,
    ) -> Self {
        Self::start(
            lease,
            Arc::new(PostgresSchedulerLeaseBackend {
                control_plane,
                credentials,
            }),
            SCHEDULER_LEASE_SECONDS,
            SCHEDULER_HEARTBEAT_INTERVAL,
            SCHEDULER_RENEW_TIMEOUT,
            SchedulerOutcome::Cancelled,
            "DOWNLOAD_STREAM_DROPPED",
        )
    }

    #[cfg(test)]
    pub(crate) fn start_for_test(
        lease: SchedulerLease,
        backend: Arc<dyn SchedulerLeaseBackend>,
        lease_seconds: u32,
        heartbeat_interval: Duration,
        renew_timeout: Duration,
        drop_outcome: SchedulerOutcome,
        drop_code: &'static str,
    ) -> Self {
        Self::start(
            lease,
            backend,
            lease_seconds,
            heartbeat_interval,
            renew_timeout,
            drop_outcome,
            drop_code,
        )
    }

    fn start(
        lease: SchedulerLease,
        backend: Arc<dyn SchedulerLeaseBackend>,
        lease_seconds: u32,
        heartbeat_interval: Duration,
        renew_timeout: Duration,
        drop_outcome: SchedulerOutcome,
        drop_code: &'static str,
    ) -> Self {
        let lifecycle = Arc::new(SchedulerLeaseLifecycle::new());
        let (stop, stopped) = oneshot::channel();
        let inner = Arc::new(SchedulerLeaseGuardInner {
            backend,
            lease: Mutex::new(lease),
            lifecycle: lifecycle.clone(),
            stop: Mutex::new(Some(stop)),
            lease_seconds,
            drop_outcome,
            drop_code,
        });
        let weak = Arc::downgrade(&inner);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(scheduler_lease_heartbeat(
                weak,
                lifecycle,
                stopped,
                heartbeat_interval,
                renew_timeout,
            ));
        } else {
            inner.lifecycle.mark_lost();
        }
        Self { inner }
    }

    pub fn is_lost(&self) -> bool {
        self.inner.lifecycle.is_lost()
    }

    pub fn ensure_owned(&self) -> Result<(), String> {
        self.inner.lifecycle.ensure_owned()
    }

    pub async fn wait_for_loss(&self) {
        self.inner.lifecycle.wait_for_loss().await;
    }

    pub async fn run_while_owned<F, T>(&self, future: F) -> Result<T, String>
    where
        F: Future<Output = T>,
    {
        self.ensure_owned()?;
        tokio::pin!(future);
        let output = tokio::select! {
            biased;
            _ = self.wait_for_loss() => return Err("SCHEDULER_LEASE_LOST".to_string()),
            output = &mut future => output,
        };
        self.ensure_owned()?;
        Ok(output)
    }

    pub async fn finish(
        &self,
        outcome: SchedulerOutcome,
        code: Option<&str>,
        message: Option<&str>,
    ) -> Result<String, String> {
        if !self.inner.lifecycle.claim_finish().await? {
            return Ok(self
                .inner
                .lifecycle
                .finished_result()
                .unwrap_or_else(|| "already_finished".to_string()));
        }

        let attempt = FinishAttempt::new(self.inner.lifecycle.clone());
        let lease = self
            .inner
            .lease
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let result = self
            .inner
            .backend
            .finish(
                lease,
                outcome,
                code.map(str::to_string),
                message.map(str::to_string),
            )
            .await?;
        self.inner.stop_heartbeat();
        attempt.finish(result.clone());
        Ok(result)
    }
}

async fn scheduler_lease_heartbeat(
    inner: Weak<SchedulerLeaseGuardInner>,
    lifecycle: Arc<SchedulerLeaseLifecycle>,
    mut stopped: oneshot::Receiver<()>,
    heartbeat_interval: Duration,
    renew_timeout: Duration,
) {
    let mut interval = tokio::time::interval_at(
        tokio::time::Instant::now() + heartbeat_interval,
        heartbeat_interval,
    );
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = &mut stopped => return,
            _ = interval.tick() => {}
        }
        if lifecycle.is_lost() || lifecycle.state.load(Ordering::Acquire) == LEASE_FINISHED {
            return;
        }
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let lease = inner
            .lease
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let renewed = tokio::time::timeout(
            renew_timeout,
            inner.backend.renew(lease, inner.lease_seconds),
        )
        .await;
        match renewed {
            Ok(Ok(lease)) => {
                *inner.lease.lock().unwrap_or_else(|e| e.into_inner()) = lease;
            }
            Ok(Err(error)) => {
                lifecycle.mark_lost();
                log::warn!("scheduler lease heartbeat stopped: {error}");
                return;
            }
            Err(_) => {
                lifecycle.mark_lost();
                log::warn!("scheduler lease heartbeat timed out");
                return;
            }
        }
    }
}

impl PostgresControlPlane {
    pub async fn acquire_scheduler_lease(
        &self,
        c: &SagaNodeCredentials,
        job: &str,
        r: &SchedulerResourceSet,
        seconds: u32,
    ) -> Result<Option<SchedulerLease>, String> {
        if !self.enabled() {
            return Ok(None);
        }
        uid(job)?;
        validate(r.keys())?;
        let s = lease_seconds(seconds)?;
        let db = self.connect_checked().await?;
        let row=db.query_one("SELECT lease_id::text,attempt_token::text,fence_token,lease_expires_at::text FROM acquire_transfer_scheduler_lease($1,$2,$3::text::uuid,$4,$5)",&[&c.node_id,&c.token(),&job,&r.keys(),&s]).await.map_err(|e|format!("scheduler acquire failed: {e}"))?;
        Ok(Some(SchedulerLease {
            lease_id: row.get(0),
            attempt_token: row.get(1),
            fence_token: row.get(2),
            lease_expires_at: row.get(3),
            resource_keys: r.keys.clone(),
        }))
    }
    pub async fn renew_scheduler_lease(
        &self,
        c: &SagaNodeCredentials,
        l: &mut SchedulerLease,
        seconds: u32,
    ) -> Result<(), String> {
        if !self.enabled() {
            return Err("POSTGRES_SCHEDULER_REQUIRED".into());
        }
        let s = lease_seconds(seconds)?;
        let db = self.connect_checked().await?;
        let row=db.query_one("SELECT renew_transfer_scheduler_lease($1,$2,$3::text::uuid,$4::text::uuid,$5,$6)::text",&[&c.node_id,&c.token(),&l.lease_id,&l.attempt_token,&l.fence_token,&s]).await.map_err(|e|format!("scheduler renew failed: {e}"))?;
        l.lease_expires_at = row.get(0);
        Ok(())
    }
    pub async fn finish_scheduler_lease(
        &self,
        c: &SagaNodeCredentials,
        l: &SchedulerLease,
        o: SchedulerOutcome,
        code: Option<&str>,
        message: Option<&str>,
    ) -> Result<String, String> {
        if !self.enabled() {
            return Err("POSTGRES_SCHEDULER_REQUIRED".into());
        }
        let (name, retry) = o.parts()?;
        let db = self.connect_checked().await?;
        let row=db.query_one("SELECT finish_transfer_scheduler_lease($1,$2,$3::text::uuid,$4::text::uuid,$5,$6,$7,$8,$9)",&[&c.node_id,&c.token(),&l.lease_id,&l.attempt_token,&l.fence_token,&name,&code,&message,&retry]).await.map_err(|e|format!("scheduler finish failed: {e}"))?;
        Ok(row.get(0))
    }
    pub async fn set_scheduler_cooldown(
        &self,
        c: &SagaNodeCredentials,
        key: &str,
        seconds: u32,
        reason: &str,
    ) -> Result<String, String> {
        if !self.enabled() {
            return Err("POSTGRES_SCHEDULER_REQUIRED".into());
        }
        key_ok(key)?;
        if !(1..=86400).contains(&seconds) {
            return Err("SCHEDULER_COOLDOWN_INVALID".into());
        }
        let s = seconds as i32;
        let db = self.connect_checked().await?;
        let row = db
            .query_one(
                "SELECT set_transfer_scheduler_cooldown($1,$2,$3,$4,$5)::text",
                &[&c.node_id, &c.token(), &key, &s, &reason],
            )
            .await
            .map_err(|e| format!("scheduler cooldown failed: {e}"))?;
        Ok(row.get(0))
    }
}
fn lease_seconds(s: u32) -> Result<i32, String> {
    if (15..=900).contains(&s) {
        Ok(s as i32)
    } else {
        Err("SCHEDULER_LEASE_SECONDS_INVALID".into())
    }
}
fn seg(v: &str) -> Result<&str, String> {
    let v = v.trim();
    if !v.is_empty()
        && v.len() <= 128
        && !v.contains(':')
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Ok(v)
    } else {
        Err("SCHEDULER_RESOURCE_SEGMENT_INVALID".into())
    }
}
fn uid(v: &str) -> Result<&str, String> {
    uuid::Uuid::parse_str(v).map_err(|_| "SCHEDULER_UUID_INVALID".to_string())?;
    Ok(v)
}
fn rank(k: &str) -> u8 {
    match k.split(':').next() {
        Some("global") => 1,
        Some("tenant") => 2,
        Some("bot") => 3,
        Some("peer") => 4,
        _ => 99,
    }
}
fn key_ok(k: &str) -> Result<(), String> {
    if (3..=512).contains(&k.len()) && rank(k) < 99 {
        Ok(())
    } else {
        Err("SCHEDULER_RESOURCE_KEY_INVALID".into())
    }
}
fn validate(k: &[String]) -> Result<(), String> {
    if !(2..=4).contains(&k.len()) || rank(&k[0]) != 1 || rank(&k[1]) != 2 {
        return Err("SCHEDULER_RESOURCE_ORDER_INVALID".into());
    }
    let mut last = 0;
    let mut seen = std::collections::HashSet::new();
    for x in k {
        key_ok(x)?;
        let r = rank(x);
        if r <= last || !seen.insert(x) {
            return Err("SCHEDULER_RESOURCE_ORDER_INVALID".into());
        }
        last = r;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;

    const T: &str = "123e4567-e89b-12d3-a456-426614174000";

    #[derive(Default)]
    struct FakeSchedulerBackend {
        fail_renew: AtomicBool,
        stall_renew: AtomicBool,
        finish_calls: AtomicUsize,
        outcomes: Mutex<Vec<SchedulerOutcome>>,
    }

    #[async_trait::async_trait]
    impl SchedulerLeaseBackend for FakeSchedulerBackend {
        async fn renew(
            &self,
            mut lease: SchedulerLease,
            _seconds: u32,
        ) -> Result<SchedulerLease, String> {
            if self.stall_renew.load(Ordering::Acquire) {
                std::future::pending::<()>().await;
            }
            if self.fail_renew.load(Ordering::Acquire) {
                return Err("TEST_RENEW_FAILED".to_string());
            }
            lease.lease_expires_at = "renewed".to_string();
            Ok(lease)
        }

        async fn finish(
            &self,
            _lease: SchedulerLease,
            outcome: SchedulerOutcome,
            _code: Option<String>,
            _message: Option<String>,
        ) -> Result<String, String> {
            self.finish_calls.fetch_add(1, Ordering::AcqRel);
            self.outcomes.lock().unwrap().push(outcome);
            Ok(match outcome {
                SchedulerOutcome::Success => "succeeded",
                SchedulerOutcome::Retry { .. } => "retry_wait",
                SchedulerOutcome::DeadLetter => "dead_letter",
                SchedulerOutcome::ManualReview => "manual_review",
                SchedulerOutcome::Cancelled => "cancelled",
            }
            .to_string())
        }
    }

    fn test_lease() -> SchedulerLease {
        SchedulerLease {
            lease_id: "123e4567-e89b-12d3-a456-426614174001".to_string(),
            attempt_token: "123e4567-e89b-12d3-a456-426614174002".to_string(),
            fence_token: 1,
            lease_expires_at: "initial".to_string(),
            resource_keys: vec![
                "global:bot:upload".to_string(),
                format!("tenant:{T}:upload"),
            ],
        }
    }

    fn test_guard(
        backend: Arc<FakeSchedulerBackend>,
        heartbeat: Duration,
        renew_timeout: Duration,
        drop_outcome: SchedulerOutcome,
    ) -> SchedulerLeaseGuard {
        SchedulerLeaseGuard::start_for_test(
            test_lease(),
            backend,
            30,
            heartbeat,
            renew_timeout,
            drop_outcome,
            "TEST_GUARD_DROPPED",
        )
    }
    #[test]
    fn ordered() {
        let r = SchedulerResourceSet::transfer(
            "bot",
            "sendDocument",
            T,
            "upload",
            Some("bot-a"),
            Some(("bot", "channel", -1001)),
        )
        .unwrap();
        assert_eq!(
            r.keys(),
            [
                "global:bot:sendDocument",
                &format!("tenant:{T}:upload"),
                "bot:bot-a:sendDocument",
                "peer:bot:channel:-1001:sendDocument"
            ]
        )
    }
    #[test]
    fn invalid_fails_closed() {
        assert!(SchedulerResourceSet::transfer("bot:x", "send", T, "upload", None, None).is_err());
        assert!(
            SchedulerResourceSet::transfer("bot", "send", "bad", "upload", None, None).is_err()
        );
        assert!(lease_seconds(14).is_err());
        assert!(SchedulerOutcome::Retry { after_seconds: 0 }
            .parts()
            .is_err())
    }
    #[test]
    fn duplicate_or_order_fails() {
        assert!(validate(&["tenant:x:u".into(), "global:b:m".into()]).is_err());
        assert!(validate(&[
            "global:b:m".into(),
            "tenant:x:u".into(),
            "tenant:x:u".into()
        ])
        .is_err())
    }

    #[tokio::test]
    async fn finish_is_idempotent_and_drop_does_not_finish_twice() {
        let backend = Arc::new(FakeSchedulerBackend::default());
        let guard = test_guard(
            backend.clone(),
            Duration::from_secs(60),
            Duration::from_secs(1),
            SchedulerOutcome::Retry { after_seconds: 5 },
        );

        assert_eq!(
            guard
                .finish(SchedulerOutcome::Success, None, None)
                .await
                .unwrap(),
            "succeeded"
        );
        assert_eq!(
            guard
                .finish(SchedulerOutcome::Success, None, None)
                .await
                .unwrap(),
            "succeeded"
        );
        drop(guard);
        tokio::task::yield_now().await;

        assert_eq!(backend.finish_calls.load(Ordering::Acquire), 1);
        assert_eq!(
            backend.outcomes.lock().unwrap().as_slice(),
            &[SchedulerOutcome::Success]
        );
    }

    #[tokio::test]
    async fn renew_failure_marks_lease_lost_and_rejects_success() {
        let backend = Arc::new(FakeSchedulerBackend::default());
        backend.fail_renew.store(true, Ordering::Release);
        let guard = test_guard(
            backend.clone(),
            Duration::from_millis(1),
            Duration::from_millis(20),
            SchedulerOutcome::Retry { after_seconds: 5 },
        );

        tokio::time::timeout(Duration::from_millis(100), guard.wait_for_loss())
            .await
            .expect("heartbeat should report ownership loss");
        assert!(guard.is_lost());
        assert_eq!(
            guard
                .finish(SchedulerOutcome::Success, None, None)
                .await
                .unwrap_err(),
            "SCHEDULER_LEASE_LOST"
        );
        drop(guard);
        tokio::task::yield_now().await;
        assert_eq!(backend.finish_calls.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn renew_timeout_marks_lease_lost() {
        let backend = Arc::new(FakeSchedulerBackend::default());
        backend.stall_renew.store(true, Ordering::Release);
        let guard = test_guard(
            backend,
            Duration::from_millis(1),
            Duration::from_millis(5),
            SchedulerOutcome::Retry { after_seconds: 5 },
        );

        tokio::time::timeout(Duration::from_millis(100), guard.wait_for_loss())
            .await
            .expect("bounded renew timeout should report ownership loss");
        assert!(guard.is_lost());
    }

    #[tokio::test]
    async fn upload_early_return_drop_schedules_one_retry_finish() {
        let backend = Arc::new(FakeSchedulerBackend::default());
        let guard = test_guard(
            backend.clone(),
            Duration::from_secs(60),
            Duration::from_secs(1),
            SchedulerOutcome::Retry { after_seconds: 5 },
        );

        drop(guard);
        tokio::time::timeout(Duration::from_millis(100), async {
            while backend.finish_calls.load(Ordering::Acquire) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("drop finalizer should be scheduled");

        assert_eq!(backend.finish_calls.load(Ordering::Acquire), 1);
        assert_eq!(
            backend.outcomes.lock().unwrap().as_slice(),
            &[SchedulerOutcome::Retry { after_seconds: 5 }]
        );
    }
}

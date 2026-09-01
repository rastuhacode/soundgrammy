//! Shared pacing for Telegram media RPCs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ferogram::InvocationError;
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};
use tokio::time::Instant;

const MAX_CONCURRENT_REQUESTS: usize = 2;
const MAX_BACKGROUND_REQUESTS: usize = 1;
const MAX_AUTOMATIC_FLOOD_WAIT: Duration = Duration::from_secs(60);
const FLOOD_WAIT_GRACE: Duration = Duration::from_millis(250);
const CANCELLATION_POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaPriority {
    Playback,
    Background,
}

#[derive(Default)]
struct CooldownState {
    by_dc: HashMap<i32, Instant>,
}

pub struct MediaRequestCoordinator {
    requests: Semaphore,
    background_requests: Semaphore,
    cooldowns: Mutex<CooldownState>,
}

pub struct MediaRequestPermit<'a> {
    _request: SemaphorePermit<'a>,
    _background: Option<SemaphorePermit<'a>>,
}

impl Default for MediaRequestCoordinator {
    fn default() -> Self {
        Self {
            requests: Semaphore::new(MAX_CONCURRENT_REQUESTS),
            background_requests: Semaphore::new(MAX_BACKGROUND_REQUESTS),
            cooldowns: Default::default(),
        }
    }
}

impl MediaRequestCoordinator {
    /// Waits for Telegram's DC cooldown and reserves aggregate request capacity.
    pub async fn acquire(
        &self,
        dc_id: i32,
        priority: MediaPriority,
        active: Option<&AtomicBool>,
    ) -> Result<MediaRequestPermit<'_>, InvocationError> {
        loop {
            self.wait_for_cooldown(dc_id, active).await?;
            require_active(active)?;

            let background = match priority {
                MediaPriority::Playback => None,
                MediaPriority::Background => Some(
                    self.background_requests
                        .acquire()
                        .await
                        .map_err(|_| InvocationError::Dropped)?,
                ),
            };
            let request = self
                .requests
                .acquire()
                .await
                .map_err(|_| InvocationError::Dropped)?;
            require_active(active)?;

            // A concurrent request may have established a cooldown while this
            // request was queued for a permit. Re-check before contacting Telegram.
            if self.cooldown_remaining(dc_id).await.is_some() {
                drop(request);
                drop(background);
                continue;
            }
            return Ok(MediaRequestPermit {
                _request: request,
                _background: background,
            });
        }
    }

    /// Records a server-requested wait. Returns false for waits that should
    /// surface to the caller rather than be retried indefinitely in-app.
    pub async fn observe_flood_wait(&self, dc_id: i32, error: &InvocationError) -> bool {
        let Some(wait) = automatic_flood_wait(error) else {
            return false;
        };
        let deadline = Instant::now() + wait + FLOOD_WAIT_GRACE;
        let mut cooldowns = self.cooldowns.lock().await;
        let current = cooldowns.by_dc.entry(dc_id).or_insert(deadline);
        if deadline > *current {
            *current = deadline;
        }
        true
    }

    async fn wait_for_cooldown(
        &self,
        dc_id: i32,
        active: Option<&AtomicBool>,
    ) -> Result<(), InvocationError> {
        while let Some(remaining) = self.cooldown_remaining(dc_id).await {
            require_active(active)?;
            let sleep_for = if active.is_some() {
                remaining.min(CANCELLATION_POLL)
            } else {
                remaining
            };
            tokio::time::sleep(sleep_for).await;
        }
        require_active(active)
    }

    async fn cooldown_remaining(&self, dc_id: i32) -> Option<Duration> {
        let mut cooldowns = self.cooldowns.lock().await;
        let deadline = cooldowns.by_dc.get(&dc_id).copied()?;
        let now = Instant::now();
        if deadline <= now {
            cooldowns.by_dc.remove(&dc_id);
            None
        } else {
            Some(deadline - now)
        }
    }
}

fn automatic_flood_wait(error: &InvocationError) -> Option<Duration> {
    let InvocationError::Rpc(rpc) = error else {
        return None;
    };
    if rpc.code != 420 || !matches!(rpc.name.as_str(), "FLOOD_WAIT" | "FLOOD_PREMIUM_WAIT") {
        return None;
    }
    let wait = Duration::from_secs(u64::from(rpc.value?));
    (wait <= MAX_AUTOMATIC_FLOOD_WAIT).then_some(wait)
}

fn require_active(active: Option<&AtomicBool>) -> Result<(), InvocationError> {
    if active.is_some_and(|active| !active.load(Ordering::Acquire)) {
        Err(InvocationError::Dropped)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ferogram::RpcError;

    use super::*;

    fn flood_wait(seconds: u32) -> InvocationError {
        InvocationError::Rpc(RpcError {
            code: 420,
            name: "FLOOD_WAIT".into(),
            value: Some(seconds),
        })
    }

    #[test]
    fn recognizes_only_bounded_telegram_flood_waits() {
        assert_eq!(
            automatic_flood_wait(&flood_wait(8)),
            Some(Duration::from_secs(8))
        );
        assert_eq!(automatic_flood_wait(&flood_wait(61)), None);
        assert_eq!(
            automatic_flood_wait(&InvocationError::Rpc(RpcError {
                code: 400,
                name: "FILE_REFERENCE_EXPIRED".into(),
                value: None,
            })),
            None
        );
    }

    #[tokio::test(start_paused = true)]
    async fn all_requests_to_a_dc_obey_the_longest_observed_cooldown() {
        let coordinator = Arc::new(MediaRequestCoordinator::default());
        assert!(coordinator.observe_flood_wait(2, &flood_wait(5)).await);
        assert!(coordinator.observe_flood_wait(2, &flood_wait(8)).await);

        let waiting = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(2, MediaPriority::Playback, None)
                    .await
                    .is_ok()
            })
        };
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        tokio::time::advance(Duration::from_secs(8)).await;
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        tokio::time::advance(FLOOD_WAIT_GRACE).await;
        assert!(waiting.await.unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn cooldowns_are_isolated_per_media_dc() {
        let coordinator = MediaRequestCoordinator::default();
        assert!(coordinator.observe_flood_wait(2, &flood_wait(8)).await);
        assert!(coordinator
            .acquire(4, MediaPriority::Playback, None)
            .await
            .is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn queued_requests_recheck_a_cooldown_before_contacting_telegram() {
        let coordinator = Arc::new(MediaRequestCoordinator::default());
        let first = coordinator
            .acquire(2, MediaPriority::Playback, None)
            .await
            .unwrap();
        let second = coordinator
            .acquire(2, MediaPriority::Playback, None)
            .await
            .unwrap();
        let queued = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(2, MediaPriority::Playback, None)
                    .await
                    .is_ok()
            })
        };
        tokio::task::yield_now().await;
        assert!(!queued.is_finished());

        assert!(coordinator.observe_flood_wait(2, &flood_wait(5)).await);
        drop(first);
        tokio::task::yield_now().await;
        assert!(!queued.is_finished());

        tokio::time::advance(Duration::from_secs(5) + FLOOD_WAIT_GRACE).await;
        assert!(queued.await.unwrap());
        drop(second);
    }

    #[tokio::test]
    async fn aggregate_media_concurrency_is_bounded() {
        let coordinator = Arc::new(MediaRequestCoordinator::default());
        let first = coordinator
            .acquire(2, MediaPriority::Playback, None)
            .await
            .unwrap();
        let second = coordinator
            .acquire(2, MediaPriority::Playback, None)
            .await
            .unwrap();
        let third = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(2, MediaPriority::Playback, None)
                    .await
                    .is_ok()
            })
        };
        tokio::task::yield_now().await;
        assert!(!third.is_finished());

        drop(first);
        assert!(third.await.unwrap());
        drop(second);
    }

    #[tokio::test(start_paused = true)]
    async fn skipped_playback_cancels_before_a_flood_retry() {
        let coordinator = Arc::new(MediaRequestCoordinator::default());
        assert!(coordinator.observe_flood_wait(2, &flood_wait(8)).await);
        let active = Arc::new(AtomicBool::new(true));
        let waiting = {
            let coordinator = Arc::clone(&coordinator);
            let active = Arc::clone(&active);
            tokio::spawn(async move {
                match coordinator
                    .acquire(2, MediaPriority::Playback, Some(&active))
                    .await
                {
                    Err(InvocationError::Dropped) => true,
                    Ok(permit) => {
                        drop(permit);
                        false
                    }
                    Err(_) => false,
                }
            })
        };
        tokio::task::yield_now().await;
        active.store(false, Ordering::Release);
        tokio::time::advance(CANCELLATION_POLL).await;

        assert!(waiting.await.unwrap());
    }

    #[tokio::test]
    async fn background_work_leaves_capacity_for_playback() {
        let coordinator = Arc::new(MediaRequestCoordinator::default());
        let first_background = coordinator
            .acquire(2, MediaPriority::Background, None)
            .await
            .unwrap();

        let second_background = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(2, MediaPriority::Background, None)
                    .await
                    .is_ok()
            })
        };
        tokio::task::yield_now().await;
        assert!(!second_background.is_finished());

        let playback = coordinator
            .acquire(2, MediaPriority::Playback, None)
            .await
            .unwrap();
        assert!(!second_background.is_finished());

        drop(playback);
        drop(first_background);
        assert!(second_background.await.unwrap());
    }
}

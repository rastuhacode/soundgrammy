//! A document lease spans every file variant and is released when its consumer ends.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub struct LeaseRegistry(Arc<Mutex<HashMap<String, usize>>>);
pub struct CacheLease {
    registry: LeaseRegistry,
    key: String,
}
pub struct AudioLease {
    pub path: PathBuf,
    pub(super) _lease: CacheLease,
}
impl AsRef<Path> for AudioLease {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}
impl std::ops::Deref for AudioLease {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}
impl LeaseRegistry {
    // Acquisition and deletion eligibility are serialized by AppState::cache_lifecycle.
    pub fn acquire(&self, key: &str) -> CacheLease {
        *self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(key.into())
            .or_default() += 1;
        CacheLease {
            registry: self.clone(),
            key: key.into(),
        }
    }
    pub fn protected_paths(&self, directory: &Path) -> Vec<PathBuf> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .keys()
            .map(|key| directory.join(key))
            .collect()
    }
}
impl Drop for CacheLease {
    fn drop(&mut self) {
        let mut leases = self.registry.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(count) = leases.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                leases.remove(&self.key);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leases_span_multiple_consumers_and_release_only_after_the_last_drop() {
        let registry = LeaseRegistry::default();
        let a = registry.acquire("123");
        let b = registry.acquire("123");
        drop(a);
        assert_eq!(
            registry.protected_paths(Path::new("/cache")),
            vec![PathBuf::from("/cache/123")]
        );
        drop(b);
        assert!(registry.protected_paths(Path::new("/cache")).is_empty());
    }
}

/// Explicit jobs reserve logical document bytes; overlapping jobs share one reservation.
#[derive(Clone, Default)]
pub struct BudgetRegistry(Arc<Mutex<HashMap<String, (u64, usize)>>>);
pub struct Reservation {
    registry: BudgetRegistry,
    key: String,
}
impl BudgetRegistry {
    pub fn snapshot(&self) -> HashMap<String, u64> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|(k, (bytes, _))| (k.clone(), *bytes))
            .collect()
    }
    pub fn reserve(&self, key: &str, total: u64) -> Reservation {
        let mut values = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let (bytes, owners) = values.entry(key.into()).or_insert((total, 0));
        *bytes = (*bytes).max(total);
        *owners += 1;
        Reservation {
            registry: self.clone(),
            key: key.into(),
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut values = self.registry.0.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((_, owners)) = values.get_mut(&self.key) {
            *owners -= 1;
            if *owners == 0 {
                values.remove(&self.key);
            }
        }
    }
}
#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn overlapping_reservations_count_a_document_once_and_release_independently() {
        let budget = BudgetRegistry::default();
        let a = budget.reserve("song", 100);
        let b = budget.reserve("song", 100);
        assert_eq!(budget.snapshot().values().sum::<u64>(), 100);
        drop(a);
        assert_eq!(budget.snapshot()["song"], 100);
        drop(b);
        assert!(budget.snapshot().is_empty());
    }
}

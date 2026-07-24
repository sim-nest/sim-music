use std::collections::{HashMap, VecDeque};

use sim_lib_sound_core::{Partial, Tone};

/// Caller-owned bounded cache for deterministic timbre renders.
#[derive(Clone, Debug)]
pub struct TimbreCache {
    /// Maximum cached bytes retained by this cache.
    pub max_bytes: usize,
    entries: HashMap<TimbreCacheKey, CachedTone>,
    order: VecDeque<TimbreCacheKey>,
    used_bytes: usize,
}

#[derive(Clone, Debug)]
struct CachedTone {
    tone: Tone,
    bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TimbreCacheKey {
    pub(crate) name: String,
    pub(crate) recipe: String,
    pub(crate) frequency_bits: u64,
    pub(crate) duration_nanos: u128,
}

impl TimbreCache {
    /// Builds an empty cache with a byte ceiling.
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            entries: HashMap::new(),
            order: VecDeque::new(),
            used_bytes: 0,
        }
    }

    /// Returns the current approximate byte use.
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    /// Returns the number of cached renders.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when there are no cached renders.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all cached renders.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.used_bytes = 0;
    }

    pub(crate) fn get(&mut self, key: &TimbreCacheKey) -> Option<Tone> {
        let tone = self.entries.get(key).map(|entry| entry.tone.clone())?;
        self.touch(key);
        Some(tone)
    }

    pub(crate) fn insert(&mut self, key: TimbreCacheKey, tone: Tone) {
        let bytes = estimate_tone_bytes(&tone);
        if bytes > self.max_bytes {
            return;
        }
        if let Some(existing) = self.entries.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(existing.bytes);
            self.order.retain(|candidate| candidate != &key);
        }
        self.used_bytes = self.used_bytes.saturating_add(bytes);
        self.order.push_back(key.clone());
        self.entries.insert(key, CachedTone { tone, bytes });
        self.evict_to_bound();
    }

    fn touch(&mut self, key: &TimbreCacheKey) {
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.clone());
    }

    fn evict_to_bound(&mut self) {
        while self.used_bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                self.used_bytes = 0;
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.used_bytes = self.used_bytes.saturating_sub(entry.bytes);
            }
        }
    }
}

fn estimate_tone_bytes(tone: &Tone) -> usize {
    std::mem::size_of::<Tone>()
        + tone
            .partials
            .len()
            .saturating_mul(std::mem::size_of::<Partial>())
}

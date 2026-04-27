use std::{
    hash::{BuildHasher, Hash},
    sync::OnceLock,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use hashbrown::HashMap;
use tokio::sync::RwLock;

static START: OnceLock<Instant> = OnceLock::new();

/// Initialize the global process start timestamp used by `now_millis_u32()`.
#[inline]
pub fn init_start() {
    START.get_or_init(Instant::now);
}

/// Convert an `Instant` to milliseconds since process start (wrapping u32).
#[inline]
pub fn instant_to_millis_u32(now: Instant) -> u32 {
    now.duration_since(
        *START.get().expect(
            "middleware::rate_limiting::rate_limiter_table::init_start() must be called first",
        ),
    )
    .as_millis() as u32
}

#[derive(Debug)]
pub struct LimiterEntry {
    pub count: AtomicU32,
    pub first_access: AtomicU32, // milliseconds since process start
}

pub struct RateLimiterTable<K, H> {
    pub map: HashMap<K, LimiterEntry, H>,
    pub last_clear: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckAndRecordResult {
    Allowed,
    /// Request is blocked; contains the updated count (including this blocked attempt).
    Blocked {
        count: u32,
    },
}

impl<K, H> RateLimiterTable<K, H>
where
    H: BuildHasher + Default,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::with_hasher(H::default()),
            last_clear: Instant::now(),
        }
    }
}

/// Create a leaked, `'static` set of sharded tables.
pub fn new_sharded_tables<K, H, const SHARDS: usize>()
-> &'static [RwLock<RateLimiterTable<K, H>>; SHARDS]
where
    H: BuildHasher + Default,
{
    Box::leak(Box::new(std::array::from_fn(|_| {
        RwLock::new(RateLimiterTable::new())
    })))
}

/// Check and record a hit against the given key in the selected shard.
pub async fn check_and_record<K: Eq + Hash, H: BuildHasher>(
    shard: &RwLock<RateLimiterTable<K, H>>,
    key: K,
    now: Instant,
    now_millis: u32,
    max_per_period: u32,
    period: Duration,
    clear_frequency: Duration,
) -> CheckAndRecordResult {
    let result = {
        let table = shard.read().await;
        let entry = table.map.get(&key);

        if let Some(entry) = entry {
            let first_access_millis = entry.first_access.load(Ordering::Relaxed);

            if now_millis.wrapping_sub(first_access_millis) > period.as_millis() as u32 {
                entry.first_access.store(now_millis, Ordering::Relaxed);
                entry.count.store(1, Ordering::Relaxed);
                Some(CheckAndRecordResult::Allowed)
            } else {
                // Increment first, then decide: avoids race where two requests both see
                // count < limit and both return Allowed (would allow one extra request).
                let prev = entry.count.fetch_add(1, Ordering::Relaxed);
                let count = prev + 1;
                if count <= max_per_period {
                    Some(CheckAndRecordResult::Allowed)
                } else {
                    Some(CheckAndRecordResult::Blocked { count })
                }
            }
        } else {
            None
        }
    };

    if let Some(result) = result {
        return result;
    }

    {
        let mut table = shard.write().await;

        if now.duration_since(table.last_clear) >= clear_frequency {
            table.map.clear();
            table.map.shrink_to_fit();
            table.last_clear = now;
        }

        table
            .map
            .entry(key)
            .and_modify(|entry| {
                entry.count.fetch_add(1, Ordering::Relaxed);
            })
            .or_insert_with(|| LimiterEntry {
                first_access: AtomicU32::new(now_millis),
                count: AtomicU32::new(1),
            });
    }

    CheckAndRecordResult::Allowed
}

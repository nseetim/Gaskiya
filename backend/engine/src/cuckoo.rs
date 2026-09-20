//! Cuckoo hashing for shard assignment: each key (a listing id) has two
//! candidate shards, `h1(key)` and `h2(key)`. Insertion places the key in
//! whichever candidate bucket has room; on a full bucket it evicts an
//! existing occupant to its own alternate bucket, cascading up to
//! `max_loop` times before giving up and rehashing into a larger table.
//! No central allocator decides placement — it falls out of the two hash
//! functions alone.

use sha2::{Digest, Sha256};

fn hash_with_salt(salt: &str, key: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    u64::from_be_bytes(digest[0..8].try_into().unwrap())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ShardAssignment {
    pub shard_id: usize,
    /// Number of evictions this insert triggered before settling (0 = placed
    /// directly at its first candidate bucket).
    pub evictions: usize,
    /// Whether the table had to grow and rehash everything to fit this key.
    pub rehashed: bool,
}

pub struct CuckooShardTable {
    num_shards: usize,
    bucket_capacity: usize,
    max_loop: usize,
    buckets: Vec<Vec<String>>,
}

impl CuckooShardTable {
    pub fn new(num_shards: usize, bucket_capacity: usize) -> Self {
        assert!(num_shards > 0);
        Self {
            num_shards,
            bucket_capacity,
            max_loop: num_shards.max(8) * 4,
            buckets: vec![Vec::new(); num_shards],
        }
    }

    pub fn num_shards(&self) -> usize {
        self.num_shards
    }

    fn h1(&self, key: &str) -> usize {
        (hash_with_salt("gaskiya-cuckoo-h1", key) % self.num_shards as u64) as usize
    }

    fn h2(&self, key: &str) -> usize {
        (hash_with_salt("gaskiya-cuckoo-h2", key) % self.num_shards as u64) as usize
    }

    fn other_bucket(&self, key: &str, current: usize) -> usize {
        let a = self.h1(key);
        let b = self.h2(key);
        if current == a {
            b
        } else {
            a
        }
    }

    /// Distribution of occupancy across shards, for the transparency
    /// dashboard's bar chart.
    pub fn distribution(&self) -> Vec<usize> {
        self.buckets.iter().map(|b| b.len()).collect()
    }

    pub fn insert(&mut self, key: &str) -> ShardAssignment {
        if let Some(assignment) = self.try_insert(key) {
            return assignment;
        }
        self.rehash_and_reinsert(key.to_string())
    }

    /// Runs the eviction-chain insert within `max_loop` steps. Leaves the
    /// table unchanged and returns `None` if the chain cascades past
    /// `max_loop` without everyone finding a bucket, signaling the caller
    /// should rehash into a larger table instead.
    fn try_insert(&mut self, key: &str) -> Option<ShardAssignment> {
        let snapshot = self.buckets.clone();
        let mut current_key = key.to_string();
        let mut pos = self.h1(&current_key);
        let mut evictions = 0;

        for _ in 0..self.max_loop {
            if self.buckets[pos].len() < self.bucket_capacity {
                self.buckets[pos].push(current_key.clone());
                return Some(ShardAssignment {
                    shard_id: pos,
                    evictions,
                    rehashed: false,
                });
            }
            let victim = self.buckets[pos].remove(0);
            self.buckets[pos].push(current_key.clone());
            evictions += 1;
            let victim_home = self.other_bucket(&victim, pos);
            current_key = victim;
            pos = victim_home;
        }

        self.buckets = snapshot;
        None
    }

    /// Grows the table one shard at a time and replays every key currently
    /// held (plus `pending_key`) until a full reinsertion succeeds. Growth
    /// is bounded to eventually succeed since capacity increases each pass
    /// while the key set stays fixed.
    fn rehash_and_reinsert(&mut self, pending_key: String) -> ShardAssignment {
        let mut all_keys: Vec<String> = self.buckets.iter().flatten().cloned().collect();
        all_keys.push(pending_key.clone());

        loop {
            self.num_shards += 1;
            self.max_loop = self.num_shards.max(8) * 4;
            self.buckets = vec![Vec::new(); self.num_shards];

            let all_placed = all_keys.iter().all(|k| self.try_insert(k).is_some());
            if all_placed {
                break;
            }
        }

        let shard_id = self
            .buckets
            .iter()
            .position(|b| b.contains(&pending_key))
            .expect("pending key was just reinserted into the rehashed table");
        ShardAssignment {
            shard_id,
            evictions: 0,
            rehashed: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distributes_keys_across_shards() {
        let mut table = CuckooShardTable::new(5, 4);
        for i in 0..15 {
            table.insert(&format!("listing-{i}"));
        }
        let dist = table.distribution();
        assert_eq!(dist.iter().sum::<usize>(), 15);
        assert!(dist.iter().all(|&count| count > 0), "expected roughly even spread: {dist:?}");
    }

    #[test]
    fn same_key_placement_is_deterministic() {
        let mut t1 = CuckooShardTable::new(5, 4);
        let mut t2 = CuckooShardTable::new(5, 4);
        for i in 0..10 {
            let key = format!("listing-{i}");
            assert_eq!(t1.insert(&key).shard_id, t2.insert(&key).shard_id);
        }
    }

    #[test]
    fn grows_via_rehash_when_overfull() {
        let mut table = CuckooShardTable::new(2, 1);
        for i in 0..10 {
            table.insert(&format!("listing-{i}"));
        }
        assert!(table.num_shards() > 2, "table should have rehashed to grow past 2 shards");
        assert_eq!(table.distribution().iter().sum::<usize>(), 10);
    }
}

use engine::cuckoo::CuckooShardTable;
use sqlx::PgPool;
use std::sync::Mutex;

use crate::anchor::IpfsClient;

pub struct AppState {
    pub pool: PgPool,
    /// Simulated logical shards (spec Section 3): the placement algorithm
    /// is real, the operational independence of separate machines is not.
    pub shard_table: Mutex<CuckooShardTable>,
    pub ipfs: IpfsClient,
    pub ipfs_gateway_base: String,
}

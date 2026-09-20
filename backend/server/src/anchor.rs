//! Periodic batch Merkle anchoring (spec Section 9/12): every unanchored
//! `ListingVersion.content_hash` is folded into one Merkle tree, the root
//! is pinned to IPFS as a small JSON manifest, and the resulting
//! `(batch_root_hash, ipfs_cid, version_ids_included)` triple is recorded
//! append-only in `merkle_anchors`. A client can later ask for a specific
//! version's inclusion proof against that pinned root without trusting the
//! server's word for it.

use chrono::Utc;
use engine::merkle_tree::{MerkleProof, MerkleTree};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::MerkleAnchorRow;

pub struct IpfsClient {
    api_base: String,
    http: reqwest::Client,
}

impl IpfsClient {
    pub fn new(api_base: String) -> Self {
        Self {
            api_base,
            http: reqwest::Client::new(),
        }
    }

    /// Adds and pins `json_bytes` to the local IPFS node, returning its CID.
    pub async fn add_pinned_json(&self, filename: &str, json_bytes: Vec<u8>) -> anyhow::Result<String> {
        let url = format!("{}/api/v0/add?pin=true", self.api_base);
        let part = reqwest::multipart::Part::bytes(json_bytes)
            .file_name(filename.to_string())
            .mime_str("application/json")?;
        let form = reqwest::multipart::Form::new().part("file", part);

        let response = self.http.post(&url).multipart(form).send().await?.error_for_status()?;
        let body: serde_json::Value = response.json().await?;
        let cid = body
            .get("Hash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| anyhow::anyhow!("ipfs add response missing Hash field: {body}"))?;
        Ok(cid.to_string())
    }
}

/// Finds every `ListingVersion` not yet covered by any existing anchor,
/// builds one batch over them, pins it, and records the anchor. Returns
/// `Ok(None)` if there was nothing pending.
pub async fn run_anchor_batch(pool: &PgPool, ipfs: &IpfsClient) -> AppResult<Option<MerkleAnchorRow>> {
    let pending: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT lv.id, lv.content_hash
         FROM listing_versions lv
         WHERE NOT EXISTS (
            SELECT 1 FROM merkle_anchors ma WHERE lv.id = ANY(ma.version_ids_included)
         )
         ORDER BY lv.created_at ASC, lv.id ASC",
    )
    .fetch_all(pool)
    .await?;

    if pending.is_empty() {
        return Ok(None);
    }

    let ids: Vec<Uuid> = pending.iter().map(|(id, _)| *id).collect();
    let hashes: Vec<String> = pending.into_iter().map(|(_, hash)| hash).collect();

    let tree = MerkleTree::build(hashes);
    let root = tree.root().to_string();

    let manifest = serde_json::json!({
        "batch_root_hash": root,
        "version_ids": ids,
        "anchored_at": Utc::now(),
    });
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    let cid = ipfs
        .add_pinned_json("gaskiya-anchor-batch.json", bytes)
        .await
        .map_err(|e| AppError::Ipfs(e.to_string()))?;

    let anchor_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO merkle_anchors (id, batch_root_hash, ipfs_cid, anchored_at, version_ids_included)
         VALUES ($1,$2,$3,now(),$4)",
    )
    .bind(anchor_id)
    .bind(&root)
    .bind(&cid)
    .bind(&ids)
    .execute(pool)
    .await?;

    let row = sqlx::query_as::<_, MerkleAnchorRow>("SELECT * FROM merkle_anchors WHERE id = $1")
        .bind(anchor_id)
        .fetch_one(pool)
        .await?;
    Ok(Some(row))
}

pub struct AnchorProofResult {
    pub anchor: MerkleAnchorRow,
    pub proof: MerkleProof,
}

/// Finds the most recent anchor covering `version_id` and rebuilds its
/// Merkle inclusion proof from scratch (re-fetching every leaf's content
/// hash in the order the anchor recorded them), rather than trusting a
/// cached proof.
pub async fn find_proof_for_version(pool: &PgPool, version_id: Uuid) -> AppResult<Option<AnchorProofResult>> {
    let anchor = sqlx::query_as::<_, MerkleAnchorRow>(
        "SELECT * FROM merkle_anchors WHERE $1 = ANY(version_ids_included) ORDER BY anchored_at DESC LIMIT 1",
    )
    .bind(version_id)
    .fetch_optional(pool)
    .await?;

    let Some(anchor) = anchor else {
        return Ok(None);
    };

    let mut hashes = Vec::with_capacity(anchor.version_ids_included.len());
    for vid in &anchor.version_ids_included {
        let (content_hash,): (String,) =
            sqlx::query_as("SELECT content_hash FROM listing_versions WHERE id = $1")
                .bind(vid)
                .fetch_one(pool)
                .await?;
        hashes.push(content_hash);
    }

    let tree = MerkleTree::build(hashes);
    let index = anchor
        .version_ids_included
        .iter()
        .position(|id| *id == version_id)
        .expect("version_id was matched by the WHERE clause that fetched this anchor");
    let proof = tree
        .proof(index)
        .expect("index came from the same leaf list the tree was built from");

    Ok(Some(AnchorProofResult { anchor, proof }))
}

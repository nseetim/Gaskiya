use actix_web::{web, HttpResponse};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::models::{
    AuthorType, CorroborateRequest, CreateInstitutionRequest, CreateListingRequest,
    CreateVersionRequest, CreateWatchRequest, InstitutionRow, ListingVersionRow,
    OpportunityListingRow, ReportRequest, SearchParams, SearchResultRow, WatchlistEntryRow,
};
use crate::state::AppState;
use crate::trust;

async fn fetch_version(pool: &sqlx::PgPool, version_id: Uuid) -> AppResult<ListingVersionRow> {
    sqlx::query_as::<_, ListingVersionRow>("SELECT * FROM listing_versions WHERE id = $1")
        .bind(version_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// Lazily registers a pseudonymous user on first use — there is no login
/// flow (spec Section 4: citizens are "anonymous or lightly registered"),
/// so any client-generated UUID becomes a valid identity the first time it
/// is used.
async fn ensure_user(pool: &sqlx::PgPool, user_id: Uuid) -> AppResult<()> {
    sqlx::query("INSERT INTO users (id, display_name, created_at) VALUES ($1, NULL, now()) ON CONFLICT (id) DO NOTHING")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn current_status_for_listing(pool: &sqlx::PgPool, listing_id: Uuid) -> AppResult<String> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM trust_score_snapshots WHERE listing_id = $1 ORDER BY computed_at DESC LIMIT 1",
    )
    .bind(listing_id)
    .fetch_optional(pool)
    .await?;
    Ok(status.unwrap_or_else(|| "unverified".to_string()))
}

/// Rejects an institution-authored submission unless it carries a real
/// Ed25519 signature over the content hash, verified against a registered,
/// admin-onboarded public key. A user-authored submission is never allowed
/// to carry a signature — that field only means something for institutions.
async fn verify_institution_signature_if_needed(
    pool: &sqlx::PgPool,
    author_type: AuthorType,
    author_id: Uuid,
    signature: &Option<String>,
    content_hash: &str,
) -> AppResult<()> {
    if author_type != AuthorType::Institution {
        return Ok(());
    }
    let sig = signature
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("institution submissions require a signature".into()))?;

    let public_key: Option<String> = sqlx::query_scalar(
        "SELECT public_key FROM institutions WHERE id = $1 AND verified_at IS NOT NULL",
    )
    .bind(author_id)
    .fetch_optional(pool)
    .await?;
    let public_key =
        public_key.ok_or_else(|| AppError::BadRequest("unknown or unverified institution".into()))?;

    engine::signature::verify_hex(&public_key, content_hash.as_bytes(), sig)
        .map_err(|_| AppError::BadRequest("institution signature does not verify against content hash".into()))?;
    Ok(())
}

/// POST /api/listings — creates a listing and its v1 (defaults to unverified).
pub async fn create_listing(
    state: web::Data<AppState>,
    body: web::Json<CreateListingRequest>,
) -> AppResult<HttpResponse> {
    let body = body.into_inner();
    let content_hash = engine::merkle_chain::content_hash(&body.content)?;
    verify_institution_signature_if_needed(&state.pool, body.author_type, body.author_id, &body.signature, &content_hash)
        .await?;

    let listing_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();

    // Shard assignment happens once, at listing creation — all of a
    // listing's future versions stay on the same shard.
    let shard_id = {
        let mut table = state.shard_table.lock().unwrap();
        table.insert(&listing_id.to_string()).shard_id as i32
    };

    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "INSERT INTO opportunity_listings (id, current_version_id, category, region, created_at)
         VALUES ($1, NULL, $2, $3, now())",
    )
    .bind(listing_id)
    .bind(body.category.as_str())
    .bind(&body.region)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO listing_versions
            (id, listing_id, version_number, title, description, eligibility, deadline,
             application_url, contact, content_hash, prev_version_hash, author_type,
             author_id, signature, shard_id, created_at)
         VALUES ($1,$2,1,$3,$4,$5,$6,$7,$8,$9,NULL,$10,$11,$12,$13,now())",
    )
    .bind(version_id)
    .bind(listing_id)
    .bind(&body.content.title)
    .bind(&body.content.description)
    .bind(&body.content.eligibility)
    .bind(body.content.deadline)
    .bind(&body.content.application_url)
    .bind(&body.content.contact)
    .bind(&content_hash)
    .bind(body.author_type.as_str())
    .bind(body.author_id)
    .bind(&body.signature)
    .bind(shard_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE opportunity_listings SET current_version_id = $1 WHERE id = $2")
        .bind(version_id)
        .bind(listing_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO current_version_log (id, listing_id, version_id, became_current_at)
         VALUES ($1,$2,$3,now())",
    )
    .bind(Uuid::new_v4())
    .bind(listing_id)
    .bind(version_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let trust_result = trust::recompute_trust(&state.pool, listing_id).await?;
    let row = fetch_version(&state.pool, version_id).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({
        "listing_id": listing_id,
        "version": row,
        "trust": trust_result,
    })))
}

/// POST /api/listings/:id/versions — submits an edit as a new, hash-chained version.
pub async fn create_version(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<CreateVersionRequest>,
) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();
    let body = body.into_inner();

    let latest = sqlx::query_as::<_, ListingVersionRow>(
        "SELECT * FROM listing_versions WHERE listing_id = $1 ORDER BY version_number DESC LIMIT 1",
    )
    .bind(listing_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let content_hash = engine::merkle_chain::content_hash(&body.content)?;
    verify_institution_signature_if_needed(&state.pool, body.author_type, body.author_id, &body.signature, &content_hash)
        .await?;

    let version_id = Uuid::new_v4();
    let version_number = latest.version_number + 1;

    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "INSERT INTO listing_versions
            (id, listing_id, version_number, title, description, eligibility, deadline,
             application_url, contact, content_hash, prev_version_hash, author_type,
             author_id, signature, shard_id, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,now())",
    )
    .bind(version_id)
    .bind(listing_id)
    .bind(version_number)
    .bind(&body.content.title)
    .bind(&body.content.description)
    .bind(&body.content.eligibility)
    .bind(body.content.deadline)
    .bind(&body.content.application_url)
    .bind(&body.content.contact)
    .bind(&content_hash)
    .bind(&latest.content_hash)
    .bind(body.author_type.as_str())
    .bind(body.author_id)
    .bind(&body.signature)
    .bind(latest.shard_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE opportunity_listings SET current_version_id = $1 WHERE id = $2")
        .bind(version_id)
        .bind(listing_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO current_version_log (id, listing_id, version_id, became_current_at)
         VALUES ($1,$2,$3,now())",
    )
    .bind(Uuid::new_v4())
    .bind(listing_id)
    .bind(version_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let trust_result = trust::recompute_trust(&state.pool, listing_id).await?;
    let row = fetch_version(&state.pool, version_id).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({
        "listing_id": listing_id,
        "version": row,
        "trust": trust_result,
    })))
}

/// GET /api/listings/:id — current version + current trust status with its
/// full rationale (expandable in the UI per spec Section 8, not just a
/// bare badge).
pub async fn get_listing(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();
    let listing = sqlx::query_as::<_, OpportunityListingRow>(
        "SELECT * FROM opportunity_listings WHERE id = $1",
    )
    .bind(listing_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let current_version_id = listing.current_version_id.ok_or(AppError::NotFound)?;
    let version = fetch_version(&state.pool, current_version_id).await?;

    let trust = sqlx::query_as::<_, crate::models::TrustScoreSnapshotRow>(
        "SELECT * FROM trust_score_snapshots WHERE listing_id = $1 ORDER BY computed_at DESC LIMIT 1",
    )
    .bind(listing_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "listing": listing,
        "current_version": version,
        "trust": trust,
    })))
}

fn diff_fields(prev: &ListingVersionRow, next: &ListingVersionRow) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if prev.title != next.title {
        changed.push("title");
    }
    if prev.description != next.description {
        changed.push("description");
    }
    if prev.eligibility != next.eligibility {
        changed.push("eligibility");
    }
    if prev.deadline != next.deadline {
        changed.push("deadline");
    }
    if prev.application_url != next.application_url {
        changed.push("application_url");
    }
    if prev.contact != next.contact {
        changed.push("contact");
    }
    changed
}

/// GET /api/listings/:id/history — the flagship view: every version, who
/// authored it, and what changed from the previous one.
pub async fn get_history(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();
    let versions = sqlx::query_as::<_, ListingVersionRow>(
        "SELECT * FROM listing_versions WHERE listing_id = $1 ORDER BY version_number ASC",
    )
    .bind(listing_id)
    .fetch_all(&state.pool)
    .await?;

    if versions.is_empty() {
        return Err(AppError::NotFound);
    }

    let entries: Vec<_> = versions
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let changed_fields = if i == 0 { Vec::new() } else { diff_fields(&versions[i - 1], v) };
            serde_json::json!({
                "version": v,
                "changed_fields": changed_fields,
            })
        })
        .collect();

    // The trust trajectory alongside the version history (spec Section 6:
    // "the demo-defining detail") — e.g. "Verified on Mar 2 -> Flagged on
    // Mar 14 after application link changed." Every snapshot ever computed,
    // not just the current one, since trust_score_snapshots is append-only.
    let trust_trajectory = sqlx::query_as::<_, crate::models::TrustScoreSnapshotRow>(
        "SELECT * FROM trust_score_snapshots WHERE listing_id = $1 ORDER BY computed_at ASC",
    )
    .bind(listing_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "listing_id": listing_id,
        "history": entries,
        "trust_trajectory": trust_trajectory,
    })))
}

/// GET /api/listings/:id/verify — recomputes the full hash chain and each
/// version's content_hash from stored content, independent of anything the
/// server otherwise claims. IPFS anchor + batch Merkle proof are added
/// once anchoring (build order phase 3) lands.
pub async fn verify_listing(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();
    let versions = sqlx::query_as::<_, ListingVersionRow>(
        "SELECT * FROM listing_versions WHERE listing_id = $1 ORDER BY version_number ASC",
    )
    .bind(listing_id)
    .fetch_all(&state.pool)
    .await?;

    if versions.is_empty() {
        return Err(AppError::NotFound);
    }

    let links: Vec<engine::merkle_chain::VersionLink> = versions
        .iter()
        .map(|v| engine::merkle_chain::VersionLink {
            content_hash: v.content_hash.clone(),
            prev_version_hash: v.prev_version_hash.clone(),
        })
        .collect();
    let chain_result = engine::merkle_chain::verify_chain(&links);

    let mut all_content_hashes_match = true;
    let mut per_version = Vec::with_capacity(versions.len());
    for v in &versions {
        let recomputed = engine::merkle_chain::content_hash(&v.content()).unwrap_or_default();
        let content_matches = recomputed == v.content_hash;
        if !content_matches {
            all_content_hashes_match = false;
        }

        let anchor_proof = crate::anchor::find_proof_for_version(&state.pool, v.id).await?;
        let anchor_json = anchor_proof.map(|found| {
            let proof_verifies = found.proof.verify(&found.anchor.batch_root_hash);
            serde_json::json!({
                "batch_root_hash": found.anchor.batch_root_hash,
                "ipfs_cid": found.anchor.ipfs_cid,
                "ipfs_gateway_url": found.anchor.ipfs_cid.as_ref().map(|cid| format!("{}/ipfs/{}", state.ipfs_gateway_base, cid)),
                "anchored_at": found.anchor.anchored_at,
                "proof": found.proof,
                "proof_verifies_against_root": proof_verifies,
            })
        });

        per_version.push(serde_json::json!({
            "version_number": v.version_number,
            "stored_content_hash": v.content_hash,
            "recomputed_content_hash": recomputed,
            "content_matches": content_matches,
            "anchor": anchor_json,
        }));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "listing_id": listing_id,
        "chain_valid": chain_result.is_ok(),
        "chain_error": chain_result.err().map(|e| e.to_string()),
        "all_content_hashes_match": all_content_hashes_match,
        "versions": per_version,
    })))
}

/// GET /api/admin/anchors — list of Merkle batch anchors + IPFS CIDs.
pub async fn list_anchors(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let anchors = sqlx::query_as::<_, crate::models::MerkleAnchorRow>(
        "SELECT * FROM merkle_anchors ORDER BY anchored_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let anchors: Vec<_> = anchors
        .into_iter()
        .map(|a| {
            let gateway_url = a
                .ipfs_cid
                .as_ref()
                .map(|cid| format!("{}/ipfs/{}", state.ipfs_gateway_base, cid));
            serde_json::json!({
                "id": a.id,
                "batch_root_hash": a.batch_root_hash,
                "ipfs_cid": a.ipfs_cid,
                "ipfs_gateway_url": gateway_url,
                "anchored_at": a.anchored_at,
                "version_count": a.version_ids_included.len(),
                "version_ids_included": a.version_ids_included,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({ "anchors": anchors })))
}

/// POST /api/admin/anchors/run — manually triggers an anchor batch
/// immediately, instead of waiting for the periodic background pass. Handy
/// for demo control.
pub async fn trigger_anchor(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    match crate::anchor::run_anchor_batch(&state.pool, &state.ipfs).await? {
        Some(anchor) => Ok(HttpResponse::Ok().json(serde_json::json!({ "anchored": true, "anchor": anchor }))),
        None => Ok(HttpResponse::Ok().json(serde_json::json!({ "anchored": false, "reason": "no pending versions" }))),
    }
}

/// POST /api/institutions — admin: onboard a verified institution keypair.
/// If `public_key` is omitted, a fresh Ed25519 keypair is generated and its
/// private half is returned once (never stored server-side) so the demo
/// institution console can sign with it. Onboarding itself IS the
/// verification step for this hackathon (spec Section 3: a manually
/// curated allowlist, not real-world KYC).
pub async fn create_institution(
    state: web::Data<AppState>,
    body: web::Json<CreateInstitutionRequest>,
) -> AppResult<HttpResponse> {
    let body = body.into_inner();

    let (public_key, generated_private_key) = match &body.public_key {
        Some(pk) => {
            if !engine::signature::is_valid_public_key_hex(pk) {
                return Err(AppError::BadRequest(
                    "public_key must be 64 hex characters (32 bytes)".into(),
                ));
            }
            (pk.clone(), None)
        }
        None => {
            let (private_key, public_key) = engine::signature::generate_keypair();
            (public_key, Some(private_key))
        }
    };

    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO institutions (id, name, public_key, verified_at, category) VALUES ($1,$2,$3,now(),$4)")
        .bind(id)
        .bind(&body.name)
        .bind(&public_key)
        .bind(body.category.as_str())
        .execute(&state.pool)
        .await?;

    let institution = sqlx::query_as::<_, InstitutionRow>("SELECT * FROM institutions WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "institution": institution,
        "private_key_hex": generated_private_key,
        "note": generated_private_key.as_ref().map(|_|
            "Shown once, never stored server-side. Use it to sign a version's content_hash (hex-encode the signature) when publishing as this institution."
        ),
    })))
}

/// GET /api/institutions — list onboarded institutions (public keys only).
pub async fn list_institutions(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let institutions = sqlx::query_as::<_, InstitutionRow>("SELECT * FROM institutions ORDER BY name ASC")
        .fetch_all(&state.pool)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "institutions": institutions })))
}

/// POST /api/listings/:id/corroborate — an independent account vouches for
/// the listing's current version. Self-corroboration (by the version's own
/// author) doesn't count (spec Section 6, signal 2's basic Sybil check).
pub async fn corroborate_listing(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<CorroborateRequest>,
) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();
    let body = body.into_inner();

    let listing = sqlx::query_as::<_, OpportunityListingRow>(
        "SELECT * FROM opportunity_listings WHERE id = $1",
    )
    .bind(listing_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let current_version_id = listing.current_version_id.ok_or(AppError::NotFound)?;
    let current_version = fetch_version(&state.pool, current_version_id).await?;

    if current_version.author_id == body.corroborator_id {
        return Err(AppError::BadRequest("a listing's own author cannot corroborate it".into()));
    }

    ensure_user(&state.pool, body.corroborator_id).await?;

    sqlx::query(
        "INSERT INTO corroboration_records (id, listing_id, version_id_corroborated, corroborator_id, created_at, note)
         VALUES ($1,$2,$3,$4,now(),$5)",
    )
    .bind(Uuid::new_v4())
    .bind(listing_id)
    .bind(current_version_id)
    .bind(body.corroborator_id)
    .bind(&body.note)
    .execute(&state.pool)
    .await?;

    let trust_result = trust::recompute_trust(&state.pool, listing_id).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "trust": trust_result })))
}

/// POST /api/listings/:id/report — flag a listing as suspicious/scam.
pub async fn report_listing(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<ReportRequest>,
) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();
    let body = body.into_inner();

    let listing = sqlx::query_as::<_, OpportunityListingRow>(
        "SELECT * FROM opportunity_listings WHERE id = $1",
    )
    .bind(listing_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let current_version_id = listing.current_version_id.ok_or(AppError::NotFound)?;

    ensure_user(&state.pool, body.reporter_id).await?;

    sqlx::query(
        "INSERT INTO scam_reports (id, listing_id, version_id_reported, reporter_id, reason, created_at)
         VALUES ($1,$2,$3,$4,$5,now())",
    )
    .bind(Uuid::new_v4())
    .bind(listing_id)
    .bind(current_version_id)
    .bind(body.reporter_id)
    .bind(&body.reason)
    .execute(&state.pool)
    .await?;

    let trust_result = trust::recompute_trust(&state.pool, listing_id).await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "trust": trust_result })))
}

/// POST /api/admin/listings/:id/confirm-scam — moderator adjudication
/// (spec Section 4/6): the one status transition the automatic rule engine
/// never makes on its own, and the one that's sticky against every future
/// recompute once set.
pub async fn confirm_scam(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let listing_id = path.into_inner();

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM opportunity_listings WHERE id = $1)")
        .bind(listing_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO trust_score_snapshots (id, listing_id, score, status, computed_at, inputs_summary)
         VALUES ($1,$2,-100,'confirmed_scam',now(),'{\"admin_override\": true}'::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(listing_id)
    .execute(&state.pool)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "confirmed_scam" })))
}

/// POST /api/watchlist — start tracking a listing through your own
/// application process, so a status change (e.g. flagged after a
/// post-verification pivot) reaches you even after you've moved on from
/// browsing. Idempotent: watching an already-watched listing is a no-op.
pub async fn create_watch(
    state: web::Data<AppState>,
    body: web::Json<CreateWatchRequest>,
) -> AppResult<HttpResponse> {
    let body = body.into_inner();

    let listing_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM opportunity_listings WHERE id = $1)")
            .bind(body.listing_id)
            .fetch_one(&state.pool)
            .await?;
    if !listing_exists {
        return Err(AppError::NotFound);
    }

    ensure_user(&state.pool, body.user_id).await?;
    let status = current_status_for_listing(&state.pool, body.listing_id).await?;

    sqlx::query(
        "INSERT INTO watchlist_entries (id, user_id, listing_id, created_at, last_seen_status, last_seen_at)
         VALUES ($1,$2,$3,now(),$4,now())
         ON CONFLICT (user_id, listing_id) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(body.user_id)
    .bind(body.listing_id)
    .bind(&status)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as::<_, WatchlistEntryRow>(
        "SELECT * FROM watchlist_entries WHERE user_id = $1 AND listing_id = $2",
    )
    .bind(body.user_id)
    .bind(body.listing_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(HttpResponse::Created().json(row))
}

/// GET /api/watchlist/:user_id — every listing a user is tracking, each
/// annotated with whether its trust status has moved since they last
/// checked.
pub async fn get_watchlist(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let user_id = path.into_inner();

    let watches = sqlx::query_as::<_, WatchlistEntryRow>(
        "SELECT * FROM watchlist_entries WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    let mut entries = Vec::with_capacity(watches.len());
    for w in watches {
        let listing = sqlx::query_as::<_, OpportunityListingRow>(
            "SELECT * FROM opportunity_listings WHERE id = $1",
        )
        .bind(w.listing_id)
        .fetch_optional(&state.pool)
        .await?;
        let Some(listing) = listing else { continue };

        let current_version = match listing.current_version_id {
            Some(vid) => Some(fetch_version(&state.pool, vid).await?),
            None => None,
        };
        let current_status = current_status_for_listing(&state.pool, w.listing_id).await?;

        entries.push(serde_json::json!({
            "watch_id": w.id,
            "listing_id": w.listing_id,
            "current_version": current_version,
            "current_status": current_status,
            "last_seen_status": w.last_seen_status,
            "status_changed": current_status != w.last_seen_status,
            "watching_since": w.created_at,
        }));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({ "watchlist": entries })))
}

/// POST /api/watchlist/:watch_id/acknowledge — dismisses a status-change
/// alert by recording the listing's current status as "seen."
pub async fn acknowledge_watch(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let watch_id = path.into_inner();
    let watch = sqlx::query_as::<_, WatchlistEntryRow>("SELECT * FROM watchlist_entries WHERE id = $1")
        .bind(watch_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let status = current_status_for_listing(&state.pool, watch.listing_id).await?;
    sqlx::query("UPDATE watchlist_entries SET last_seen_status = $1, last_seen_at = now() WHERE id = $2")
        .bind(&status)
        .bind(watch_id)
        .execute(&state.pool)
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "acknowledged": true, "status": status })))
}

/// DELETE /api/watchlist/:watch_id — stop tracking a listing.
pub async fn delete_watch(state: web::Data<AppState>, path: web::Path<Uuid>) -> AppResult<HttpResponse> {
    let watch_id = path.into_inner();
    let result = sqlx::query("DELETE FROM watchlist_entries WHERE id = $1")
        .bind(watch_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(HttpResponse::NoContent().finish())
}

/// GET /api/search?category=&region=&deadline_before=&status= — filters
/// backed by the native Postgres B-tree indexes on category/region/deadline
/// (see DECISIONS.md: native DB index over a hand-rolled B-tree).
pub async fn search_listings(
    state: web::Data<AppState>,
    query: web::Query<SearchParams>,
) -> AppResult<HttpResponse> {
    let params = query.into_inner();

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            ol.id AS listing_id, ol.category, ol.region,
            lv.id AS version_id, lv.version_number, lv.title, lv.description, lv.eligibility,
            lv.deadline, lv.application_url, lv.contact, lv.content_hash, lv.shard_id,
            lv.created_at AS version_created_at,
            COALESCE(ts.status, 'unverified') AS status, COALESCE(ts.score, 0) AS score,
            ts.computed_at AS status_computed_at
         FROM opportunity_listings ol
         JOIN listing_versions lv ON lv.id = ol.current_version_id
         LEFT JOIN LATERAL (
            SELECT status, score, computed_at FROM trust_score_snapshots
            WHERE listing_id = ol.id ORDER BY computed_at DESC LIMIT 1
         ) ts ON true
         WHERE 1 = 1",
    );

    if let Some(category) = &params.category {
        qb.push(" AND ol.category = ").push_bind(category.as_str());
    }
    if let Some(region) = &params.region {
        qb.push(" AND ol.region ILIKE ").push_bind(format!("%{region}%"));
    }
    if let Some(deadline_before) = &params.deadline_before {
        qb.push(" AND lv.deadline <= ").push_bind(*deadline_before);
    }
    if let Some(status) = &params.status {
        qb.push(" AND COALESCE(ts.status, 'unverified') = ").push_bind(status.clone());
    }

    qb.push(" ORDER BY lv.deadline ASC LIMIT 100");

    let results = qb
        .build_query_as::<SearchResultRow>()
        .fetch_all(&state.pool)
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "results": results })))
}

/// GET /api/admin/shards — live Cuckoo-hash shard distribution, for the
/// transparency dashboard.
pub async fn get_shard_distribution(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let table = state.shard_table.lock().unwrap();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "num_shards": table.num_shards(),
        "distribution": table.distribution(),
    })))
}

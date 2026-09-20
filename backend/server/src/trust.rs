//! Deterministic, explainable trust-status rule engine (spec Section 6).
//! Every signal here is a plain rule a human can audit — no opaque ML —
//! because both judges and real users need to see *why* a listing is
//! flagged, and because the paper's contribution is about verifiable
//! structure, not statistical inference.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::errors::AppResult;
use crate::models::ListingVersionRow;

pub const COMMUNITY_VERIFIED_THRESHOLD: i64 = 3;
pub const FLAGGED_REPORT_THRESHOLD: i64 = 5;
const NEAR_DUPLICATE_THRESHOLD: f64 = 0.8;
const SHINGLE_SIZE: usize = 3;

/// The flagship detection routine (spec Section 6, signal 3): flags the
/// index of the first version that changes `application_url` and/or
/// `contact` while leaving every other field untouched, *after* the
/// listing had already earned some trust (an institutional signature or at
/// least one corroboration). That "everything else the same" shape is what
/// separates a legitimate full-content update from a verify-then-pivot
/// attack — a scammer swapping the payout link rarely bothers rewriting
/// the eligibility text too.
pub fn detect_post_verification_pivot(
    versions: &[ListingVersionRow],
    institution_signature_valid: &[bool],
    first_corroboration_at: Option<DateTime<Utc>>,
) -> Option<usize> {
    debug_assert_eq!(versions.len(), institution_signature_valid.len());
    let mut institution_verified_seen = false;

    for i in 0..versions.len() {
        if i > 0 {
            let had_trust_before = institution_verified_seen
                || first_corroboration_at.is_some_and(|t| t < versions[i].created_at);

            if had_trust_before {
                let prev = &versions[i - 1];
                let next = &versions[i];
                let sensitive_changed =
                    prev.application_url != next.application_url || prev.contact != next.contact;
                let other_fields_same = prev.title == next.title
                    && prev.description == next.description
                    && prev.eligibility == next.eligibility
                    && prev.deadline == next.deadline;

                if sensitive_changed && other_fields_same {
                    return Some(i);
                }
            }
        }
        if institution_signature_valid[i] {
            institution_verified_seen = true;
        }
    }
    None
}

pub fn normalize_text(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn shingles(s: &str, size: usize) -> HashSet<String> {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() <= size {
        return HashSet::from([words.join(" ")]);
    }
    words.windows(size).map(|w| w.join(" ")).collect()
}

/// Jaccard similarity over word k-shingles (spec Section 6, signal 4:
/// "simple Jaccard/shingling on normalized text"), for detecting near-copy
/// scam reposts that a byte-exact hash comparison would miss.
pub fn jaccard_similarity(a: &str, b: &str, shingle_size: usize) -> f64 {
    let sa = shingles(a, shingle_size);
    let sb = shingles(b, shingle_size);
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let intersection = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Rule half of spec Section 6 signal 5 (the LLM-assisted half is deferred
/// to the AI-assist build-order phase). Deliberately narrow, high-precision
/// phrase lists rather than single keywords, to avoid flagging every
/// listing that merely mentions "fee" or a real deadline.
pub fn structural_red_flags(text: &str) -> Vec<&'static str> {
    const FEE_PHRASES: &[&str] = &[
        "processing fee",
        "release your funds",
        "release the funds",
        "administrative fee",
        "activation fee",
        "fee to release",
        "pay to receive",
    ];
    const URGENCY_PHRASES: &[&str] = &[
        "act now",
        "limited time",
        "immediately or lose",
        "offer expires today",
        "last chance",
    ];

    let lower = text.to_lowercase();
    let mut flags = Vec::new();
    if FEE_PHRASES.iter().any(|p| lower.contains(p)) {
        flags.push("requests_upfront_payment");
    }
    if URGENCY_PHRASES.iter().any(|p| lower.contains(p)) {
        flags.push("urgency_pressure_language");
    }
    flags
}

pub struct DuplicateFinding {
    pub kind: &'static str,
    pub other_listing_id: Uuid,
    pub similarity: f64,
}

/// Checks a listing's current content against every other listing in the
/// system: an exact `content_hash` match elsewhere is a byte-for-byte
/// clone (a real edit should go through `POST .../versions`, not a new
/// listing id); a near-duplicate specifically against a `flagged` or
/// `confirmed_scam` listing suggests a reposted scam pattern (spec Section
/// 6, signal 4 — the "known scam corpus" here is simply the ledger's own
/// prior flagged/confirmed listings, rather than a separately seeded set).
pub async fn check_duplicate(
    pool: &PgPool,
    listing_id: Uuid,
    content_hash: &str,
    normalized_text: &str,
) -> AppResult<Option<DuplicateFinding>> {
    let exact: Option<Uuid> = sqlx::query_scalar(
        "SELECT listing_id FROM listing_versions WHERE content_hash = $1 AND listing_id != $2 LIMIT 1",
    )
    .bind(content_hash)
    .bind(listing_id)
    .fetch_optional(pool)
    .await?;

    if let Some(other_listing_id) = exact {
        return Ok(Some(DuplicateFinding {
            kind: "exact_duplicate",
            other_listing_id,
            similarity: 1.0,
        }));
    }

    let candidates: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT ol.id, lv.title, lv.description, lv.eligibility
         FROM opportunity_listings ol
         JOIN listing_versions lv ON lv.id = ol.current_version_id
         JOIN LATERAL (
            SELECT status FROM trust_score_snapshots WHERE listing_id = ol.id ORDER BY computed_at DESC LIMIT 1
         ) ts ON true
         WHERE ts.status IN ('flagged', 'confirmed_scam') AND ol.id != $1",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;

    let mut best: Option<DuplicateFinding> = None;
    for (other_id, title, description, eligibility) in candidates {
        let other_text = normalize_text(&format!("{title} {description} {eligibility}"));
        let similarity = jaccard_similarity(normalized_text, &other_text, SHINGLE_SIZE);
        if similarity >= NEAR_DUPLICATE_THRESHOLD
            && best.as_ref().is_none_or(|b| similarity > b.similarity)
        {
            best = Some(DuplicateFinding {
                kind: "near_duplicate_of_flagged_listing",
                other_listing_id: other_id,
                similarity,
            });
        }
    }
    Ok(best)
}

fn compute_score(
    signature_valid: bool,
    corroborations: i64,
    reports: i64,
    pivot: bool,
    duplicate: bool,
    upfront_payment_flag: bool,
    urgency_flag: bool,
    confirmed_scam: bool,
) -> f64 {
    if confirmed_scam {
        return -100.0;
    }
    let mut score = 0.0;
    if signature_valid {
        score += 40.0;
    }
    score += corroborations.min(COMMUNITY_VERIFIED_THRESHOLD) as f64 * 10.0;
    score -= reports as f64 * 15.0;
    if pivot {
        score -= 100.0;
    }
    if duplicate {
        score -= 60.0;
    }
    if upfront_payment_flag {
        score -= 60.0;
    }
    if urgency_flag {
        score -= 10.0;
    }
    score
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustRecomputeResult {
    pub status: String,
    pub score: f64,
    pub inputs_summary: serde_json::Value,
}

/// Recomputes every signal for a listing and appends a fresh, append-only
/// `TrustScoreSnapshot` — called after every version submission,
/// corroboration, and report (spec Section 6: "recomputed on every new
/// version/event"), so the trust trajectory itself becomes a timeline a
/// user can read, not just a single current badge.
pub async fn recompute_trust(pool: &PgPool, listing_id: Uuid) -> AppResult<TrustRecomputeResult> {
    let versions = sqlx::query_as::<_, ListingVersionRow>(
        "SELECT * FROM listing_versions WHERE listing_id = $1 ORDER BY version_number ASC",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;

    let mut signature_valid_per_version = Vec::with_capacity(versions.len());
    for v in &versions {
        let valid = if v.author_type == "institution" {
            match &v.signature {
                Some(sig) => {
                    let public_key: Option<String> = sqlx::query_scalar(
                        "SELECT public_key FROM institutions WHERE id = $1 AND verified_at IS NOT NULL",
                    )
                    .bind(v.author_id)
                    .fetch_optional(pool)
                    .await?;
                    match public_key {
                        Some(pk) => engine::signature::verify_hex(&pk, v.content_hash.as_bytes(), sig).is_ok(),
                        None => false,
                    }
                }
                None => false,
            }
        } else {
            false
        };
        signature_valid_per_version.push(valid);
    }
    let current_signature_valid = *signature_valid_per_version.last().unwrap_or(&false);

    let first_corroboration_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT MIN(created_at) FROM corroboration_records WHERE listing_id = $1")
            .bind(listing_id)
            .fetch_one(pool)
            .await?;

    let pivot_index = detect_post_verification_pivot(&versions, &signature_valid_per_version, first_corroboration_at);

    let current_author_id = versions.last().map(|v| v.author_id).unwrap_or(Uuid::nil());
    let corroboration_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT corroborator_id) FROM corroboration_records WHERE listing_id = $1 AND corroborator_id != $2",
    )
    .bind(listing_id)
    .bind(current_author_id)
    .fetch_one(pool)
    .await?;

    let report_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scam_reports WHERE listing_id = $1")
        .bind(listing_id)
        .fetch_one(pool)
        .await?;

    let current = versions.last();
    let normalized_text = current
        .map(|v| normalize_text(&format!("{} {} {}", v.title, v.description, v.eligibility)))
        .unwrap_or_default();
    let duplicate = match current {
        Some(v) => check_duplicate(pool, listing_id, &v.content_hash, &normalized_text).await?,
        None => None,
    };

    let combined_text = current
        .map(|v| format!("{} {} {}", v.description, v.eligibility, v.contact))
        .unwrap_or_default();
    let red_flags = structural_red_flags(&combined_text);
    let upfront_payment_flag = red_flags.contains(&"requests_upfront_payment");
    let urgency_flag = red_flags.contains(&"urgency_pressure_language");

    let prior_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM trust_score_snapshots WHERE listing_id = $1 ORDER BY computed_at DESC LIMIT 1",
    )
    .bind(listing_id)
    .fetch_optional(pool)
    .await?;

    let status = if prior_status.as_deref() == Some("confirmed_scam") {
        "confirmed_scam".to_string()
    } else if pivot_index.is_some()
        || report_count >= FLAGGED_REPORT_THRESHOLD
        || duplicate.is_some()
        || upfront_payment_flag
    {
        "flagged".to_string()
    } else if prior_status.as_deref() == Some("flagged") {
        // Sticky: once flagged, stays flagged until an admin adjudicates
        // (spec Section 6 lifecycle shows no automatic path back).
        "flagged".to_string()
    } else if current_signature_valid {
        "institutionally_verified".to_string()
    } else if corroboration_count >= COMMUNITY_VERIFIED_THRESHOLD {
        "community_verified".to_string()
    } else {
        "unverified".to_string()
    };

    let score = compute_score(
        current_signature_valid,
        corroboration_count,
        report_count,
        pivot_index.is_some(),
        duplicate.is_some(),
        upfront_payment_flag,
        urgency_flag,
        status == "confirmed_scam",
    );

    let inputs_summary = serde_json::json!({
        "institution_signature_valid": current_signature_valid,
        "corroboration_count": corroboration_count,
        "report_count": report_count,
        "pivot_detected_at_version_number": pivot_index.map(|i| versions[i].version_number),
        "duplicate": duplicate.as_ref().map(|d| serde_json::json!({
            "kind": d.kind,
            "other_listing_id": d.other_listing_id,
            "similarity": d.similarity,
        })),
        "structural_red_flags": red_flags,
    });

    sqlx::query(
        "INSERT INTO trust_score_snapshots (id, listing_id, score, status, computed_at, inputs_summary)
         VALUES ($1,$2,$3,$4,now(),$5)",
    )
    .bind(Uuid::new_v4())
    .bind(listing_id)
    .bind(score)
    .bind(&status)
    .bind(&inputs_summary)
    .execute(pool)
    .await?;

    Ok(TrustRecomputeResult {
        status,
        score,
        inputs_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn version(
        version_number: i32,
        application_url: &str,
        contact: &str,
        title: &str,
        created_at: DateTime<Utc>,
    ) -> ListingVersionRow {
        ListingVersionRow {
            id: Uuid::new_v4(),
            listing_id: Uuid::new_v4(),
            version_number,
            title: title.to_string(),
            description: "same description".to_string(),
            eligibility: "same eligibility".to_string(),
            deadline: Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap(),
            application_url: application_url.to_string(),
            contact: contact.to_string(),
            content_hash: format!("hash-{version_number}"),
            prev_version_hash: None,
            author_type: "user".to_string(),
            author_id: Uuid::new_v4(),
            signature: None,
            shard_id: None,
            created_at,
        }
    }

    fn t(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, hour, 0, 0).unwrap()
    }

    #[test]
    fn flags_pivot_after_corroboration_when_only_sensitive_fields_change() {
        let versions = vec![
            version(1, "https://real.example/apply", "real@example.org", "Grant", t(0)),
            version(2, "https://phishing.example/apply", "fake@phishing.example", "Grant", t(2)),
        ];
        let sig_valid = vec![false, false];
        let first_corroboration_at = Some(t(1)); // between v1 and v2

        assert_eq!(
            detect_post_verification_pivot(&versions, &sig_valid, first_corroboration_at),
            Some(1)
        );
    }

    #[test]
    fn does_not_flag_pivot_before_any_trust_established() {
        let versions = vec![
            version(1, "https://real.example/apply", "real@example.org", "Grant", t(0)),
            version(2, "https://phishing.example/apply", "fake@phishing.example", "Grant", t(2)),
        ];
        let sig_valid = vec![false, false];
        assert_eq!(detect_post_verification_pivot(&versions, &sig_valid, None), None);
    }

    #[test]
    fn does_not_flag_full_content_rewrite_as_pivot() {
        let versions = vec![
            version(1, "https://real.example/apply", "real@example.org", "Grant", t(0)),
            version(2, "https://real.example/apply-v2", "new@example.org", "Updated Grant Name", t(2)),
        ];
        let sig_valid = vec![true, true]; // institution verified from v1
        assert_eq!(detect_post_verification_pivot(&versions, &sig_valid, None), None);
    }

    #[test]
    fn jaccard_identical_text_is_one() {
        let text = "apply for the national scholarship program today";
        assert_eq!(jaccard_similarity(text, text, 3), 1.0);
    }

    #[test]
    fn jaccard_detects_near_duplicate_with_minor_edits() {
        let a = normalize_text("Apply for the National Youth Scholarship before the deadline");
        let b = normalize_text("apply for the national youth scholarship, before the deadline!");
        assert!(jaccard_similarity(&a, &b, 3) > 0.8);
    }

    #[test]
    fn jaccard_unrelated_text_is_low() {
        let a = normalize_text("Scholarship for STEM students in Kano");
        let b = normalize_text("Grant funding for women-led tech startups in Accra");
        assert!(jaccard_similarity(&a, &b, 3) < 0.3);
    }

    #[test]
    fn flags_upfront_fee_language() {
        let flags = structural_red_flags("Pay a small processing fee to release your funds immediately.");
        assert!(flags.contains(&"requests_upfront_payment"));
    }

    #[test]
    fn does_not_flag_ordinary_deadline_language() {
        let flags = structural_red_flags("Applications close on December 1st. No fees required to apply.");
        assert!(!flags.contains(&"requests_upfront_payment"));
    }
}

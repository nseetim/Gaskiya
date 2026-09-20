use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The versioned, hash-chained payload of a listing. Kept as its own type
/// (rather than inlined into request/DB structs) because it's exactly what
/// gets canonicalized and hashed — every caller that needs a content_hash
/// builds one of these first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingContent {
    pub title: String,
    pub description: String,
    pub eligibility: String,
    pub deadline: DateTime<Utc>,
    pub application_url: String,
    pub contact: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Scholarship,
    Grant,
    Subsidy,
    Job,
    Training,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Scholarship => "scholarship",
            Category::Grant => "grant",
            Category::Subsidy => "subsidy",
            Category::Job => "job",
            Category::Training => "training",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorType {
    Institution,
    User,
}

impl AuthorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthorType::Institution => "institution",
            AuthorType::User => "user",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    pub category: Category,
    pub region: String,
    pub content: ListingContent,
    pub author_type: AuthorType,
    pub author_id: Uuid,
    pub signature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVersionRequest {
    pub content: ListingContent,
    pub author_type: AuthorType,
    pub author_id: Uuid,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ListingVersionRow {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub version_number: i32,
    pub title: String,
    pub description: String,
    pub eligibility: String,
    pub deadline: DateTime<Utc>,
    pub application_url: String,
    pub contact: String,
    pub content_hash: String,
    pub prev_version_hash: Option<String>,
    pub author_type: String,
    pub author_id: Uuid,
    pub signature: Option<String>,
    pub shard_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl ListingVersionRow {
    pub fn content(&self) -> ListingContent {
        ListingContent {
            title: self.title.clone(),
            description: self.description.clone(),
            eligibility: self.eligibility.clone(),
            deadline: self.deadline,
            application_url: self.application_url.clone(),
            contact: self.contact.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OpportunityListingRow {
    pub id: Uuid,
    pub current_version_id: Option<Uuid>,
    pub category: String,
    pub region: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub category: Option<Category>,
    pub region: Option<String>,
    pub deadline_before: Option<DateTime<Utc>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstitutionCategory {
    Govt,
    University,
    Ngo,
}

impl InstitutionCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstitutionCategory::Govt => "govt",
            InstitutionCategory::University => "university",
            InstitutionCategory::Ngo => "ngo",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateInstitutionRequest {
    pub name: String,
    pub category: InstitutionCategory,
    /// Hex-encoded Ed25519 public key. The matching private key is
    /// generated and returned once by this endpoint — the server never
    /// stores it (spec Section 3: no HSM-backed key management in scope).
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct InstitutionRow {
    pub id: Uuid,
    pub name: String,
    pub public_key: String,
    pub verified_at: Option<DateTime<Utc>>,
    pub category: String,
}

#[derive(Debug, Deserialize)]
pub struct CorroborateRequest {
    pub corroborator_id: Uuid,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReportRequest {
    pub reporter_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWatchRequest {
    /// A client-generated, pseudonymous capability token (see DECISIONS.md
    /// "Operating constraints" — no login system exists, so possession of
    /// this UUID is the only access control for a user's own watchlist).
    pub user_id: Uuid,
    pub listing_id: Uuid,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WatchlistEntryRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub listing_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_seen_status: String,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TrustScoreSnapshotRow {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub score: f64,
    pub status: String,
    pub computed_at: DateTime<Utc>,
    pub inputs_summary: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MerkleAnchorRow {
    pub id: Uuid,
    pub batch_root_hash: String,
    pub ipfs_cid: Option<String>,
    pub anchored_at: DateTime<Utc>,
    pub version_ids_included: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SearchResultRow {
    pub listing_id: Uuid,
    pub category: String,
    pub region: String,
    pub version_id: Uuid,
    pub version_number: i32,
    pub title: String,
    pub description: String,
    pub eligibility: String,
    pub deadline: DateTime<Utc>,
    pub application_url: String,
    pub contact: String,
    pub content_hash: String,
    pub shard_id: Option<i32>,
    pub status: String,
    pub score: f64,
    /// When this version was published — the "how do I know this is
    /// current" signal called for by the trust/verification constraint.
    pub version_created_at: DateTime<Utc>,
    /// When trust status was last (re)computed; null until the trust engine
    /// (build-order phase 4) starts writing snapshots.
    pub status_computed_at: Option<DateTime<Utc>>,
}

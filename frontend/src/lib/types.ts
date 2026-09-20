// Mirrors the Rust models in backend/server/src/models.rs. Kept as plain
// types (not codegen'd) since the backend is the source of truth and this
// is a hackathon-scale surface.

export type Category = "scholarship" | "grant" | "subsidy" | "job" | "training";
export type AuthorType = "institution" | "user";
export type InstitutionCategory = "govt" | "university" | "ngo";
export type TrustStatus =
  | "unverified"
  | "community_verified"
  | "institutionally_verified"
  | "flagged"
  | "confirmed_scam";

export interface ListingContent {
  title: string;
  description: string;
  eligibility: string;
  /** RFC3339, e.g. "2026-12-01T00:00:00Z" */
  deadline: string;
  application_url: string;
  contact: string;
}

export interface ListingVersion extends ListingContent {
  id: string;
  listing_id: string;
  version_number: number;
  content_hash: string;
  prev_version_hash: string | null;
  author_type: AuthorType;
  author_id: string;
  signature: string | null;
  shard_id: number | null;
  created_at: string;
}

export interface OpportunityListing {
  id: string;
  current_version_id: string | null;
  category: Category;
  region: string;
  created_at: string;
}

export interface TrustScoreSnapshot {
  id: string;
  listing_id: string;
  score: number;
  status: TrustStatus;
  computed_at: string;
  inputs_summary: {
    institution_signature_valid?: boolean;
    corroboration_count?: number;
    report_count?: number;
    pivot_detected_at_version_number?: number | null;
    duplicate?: { kind: string; other_listing_id: string; similarity: number } | null;
    structural_red_flags?: string[];
    admin_override?: boolean;
  };
}

export interface SearchResult extends ListingContent {
  listing_id: string;
  category: Category;
  region: string;
  version_id: string;
  version_number: number;
  content_hash: string;
  shard_id: number | null;
  status: TrustStatus;
  score: number;
  version_created_at: string;
  status_computed_at: string | null;
}

export interface Institution {
  id: string;
  name: string;
  public_key: string;
  verified_at: string | null;
  category: InstitutionCategory;
}

export interface MerkleProof {
  leaf_hash: string;
  leaf_index: number;
  siblings: [string, "Left" | "Right"][];
}

export interface VerifyVersionResult {
  version_number: number;
  stored_content_hash: string;
  recomputed_content_hash: string;
  content_matches: boolean;
  anchor: {
    batch_root_hash: string;
    ipfs_cid: string | null;
    ipfs_gateway_url: string | null;
    anchored_at: string;
    proof: MerkleProof;
    proof_verifies_against_root: boolean;
  } | null;
}

export interface VerifyResult {
  listing_id: string;
  chain_valid: boolean;
  chain_error: string | null;
  all_content_hashes_match: boolean;
  versions: VerifyVersionResult[];
}

export interface HistoryEntry {
  version: ListingVersion;
  changed_fields: string[];
}

export interface HistoryResult {
  listing_id: string;
  history: HistoryEntry[];
  trust_trajectory: TrustScoreSnapshot[];
}

export interface ListingDetail {
  listing: OpportunityListing;
  current_version: ListingVersion;
  trust: TrustScoreSnapshot | null;
}

export interface WatchEntry {
  watch_id: string;
  listing_id: string;
  current_version: ListingVersion | null;
  current_status: TrustStatus;
  last_seen_status: TrustStatus;
  status_changed: boolean;
  watching_since: string;
}

export interface MerkleAnchor {
  id: string;
  batch_root_hash: string;
  ipfs_cid: string | null;
  ipfs_gateway_url: string | null;
  anchored_at: string;
  version_count: number;
  version_ids_included: string[];
}

export interface ShardDistribution {
  num_shards: number;
  distribution: number[];
}

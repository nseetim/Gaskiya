// Must byte-for-byte match backend/engine/src/merkle_chain.rs's content_hash:
// serde_json::to_value + serde_json::to_string relies on serde_json's
// BTreeMap-backed Value::Object sorting keys alphabetically at every level,
// with compact (no-whitespace) output. This replicates that exactly so a
// client-computed hash matches what the server independently recomputes —
// verified against the Rust implementation (see DECISIONS.md).
import type { ListingContent } from "./types";

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    const sorted: Record<string, unknown> = {};
    for (const key of Object.keys(value as Record<string, unknown>).sort()) {
      sorted[key] = canonicalize((value as Record<string, unknown>)[key]);
    }
    return sorted;
  }
  return value;
}

export function canonicalJsonString(value: unknown): string {
  return JSON.stringify(canonicalize(value));
}

export async function sha256Hex(text: string): Promise<string> {
  const data = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export async function contentHash(content: ListingContent): Promise<string> {
  return sha256Hex(canonicalJsonString(content));
}

// Mirrors backend/engine/src/merkle_tree.rs hash_pair: sha256 of the raw
// UTF-8 bytes of the two hex-string hashes concatenated (not their decoded
// binary) — matched exactly so a client can independently confirm a Merkle
// inclusion proof against an anchor's root without trusting the server's
// `proof_verifies_against_root` field.
export async function hashPair(left: string, right: string): Promise<string> {
  return sha256Hex(left + right);
}

export async function verifyMerkleProof(
  leafHash: string,
  siblings: [string, "Left" | "Right"][],
  expectedRoot: string,
): Promise<boolean> {
  let current = leafHash;
  for (const [sibling, side] of siblings) {
    current = side === "Right" ? await hashPair(current, sibling) : await hashPair(sibling, current);
  }
  return current === expectedRoot;
}

/** "YYYY-MM-DD" -> "YYYY-MM-DDT00:00:00Z", matching chrono's RFC3339
 * (AutoSi, Z-suffixed) serialization for a whole-second UTC midnight. */
export function dateOnlyToRfc3339(dateOnly: string): string {
  return `${dateOnly}T00:00:00Z`;
}

export function rfc3339ToDateOnly(rfc3339: string): string {
  return rfc3339.slice(0, 10);
}

import type {
  AuthorType,
  Category,
  HistoryResult,
  Institution,
  InstitutionCategory,
  ListingContent,
  ListingDetail,
  ListingVersion,
  MerkleAnchor,
  SearchResult,
  ShardDistribution,
  TrustScoreSnapshot,
  TrustStatus,
  VerifyResult,
  WatchEntry,
} from "./types";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://127.0.0.1:8080";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", ...init?.headers },
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new ApiError(res.status, body.error ?? res.statusText);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

export interface CreateListingInput {
  category: Category;
  region: string;
  content: ListingContent;
  author_type: AuthorType;
  author_id: string;
  signature: string | null;
}

export interface CreateResult {
  listing_id: string;
  version: ListingVersion;
  trust: TrustScoreSnapshot;
}

export const api = {
  createListing: (input: CreateListingInput) =>
    request<CreateResult>("/api/listings", { method: "POST", body: JSON.stringify(input) }),

  createVersion: (
    listingId: string,
    input: Omit<CreateListingInput, "category" | "region">,
  ) =>
    request<CreateResult>(`/api/listings/${listingId}/versions`, {
      method: "POST",
      body: JSON.stringify(input),
    }),

  getListing: (listingId: string) => request<ListingDetail>(`/api/listings/${listingId}`),

  getHistory: (listingId: string) => request<HistoryResult>(`/api/listings/${listingId}/history`),

  verify: (listingId: string) => request<VerifyResult>(`/api/listings/${listingId}/verify`),

  search: (params: { category?: string; region?: string; deadline_before?: string; status?: string }) => {
    const qs = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) {
      if (v) qs.set(k, v);
    }
    const query = qs.toString();
    return request<{ results: SearchResult[] }>(`/api/search${query ? `?${query}` : ""}`);
  },

  corroborate: (listingId: string, corroboratorId: string, note?: string) =>
    request<{ trust: TrustScoreSnapshot }>(`/api/listings/${listingId}/corroborate`, {
      method: "POST",
      body: JSON.stringify({ corroborator_id: corroboratorId, note: note ?? null }),
    }),

  report: (listingId: string, reporterId: string, reason: string) =>
    request<{ trust: TrustScoreSnapshot }>(`/api/listings/${listingId}/report`, {
      method: "POST",
      body: JSON.stringify({ reporter_id: reporterId, reason }),
    }),

  createInstitution: (name: string, category: InstitutionCategory, publicKey?: string) =>
    request<{ institution: Institution; private_key_hex: string | null; note: string | null }>(
      "/api/institutions",
      { method: "POST", body: JSON.stringify({ name, category, public_key: publicKey ?? null }) },
    ),

  listInstitutions: () => request<{ institutions: Institution[] }>("/api/institutions"),

  confirmScam: (listingId: string) =>
    request<{ status: TrustStatus }>(`/api/admin/listings/${listingId}/confirm-scam`, { method: "POST" }),

  watch: (userId: string, listingId: string) =>
    request(`/api/watchlist`, {
      method: "POST",
      body: JSON.stringify({ user_id: userId, listing_id: listingId }),
    }),

  getWatchlist: (userId: string) => request<{ watchlist: WatchEntry[] }>(`/api/watchlist/${userId}`),

  acknowledgeWatch: (watchId: string) =>
    request(`/api/watchlist/entry/${watchId}/acknowledge`, { method: "POST" }),

  unwatch: (watchId: string) => request(`/api/watchlist/entry/${watchId}`, { method: "DELETE" }),

  shardDistribution: () => request<ShardDistribution>("/api/admin/shards"),

  anchors: () => request<{ anchors: MerkleAnchor[] }>("/api/admin/anchors"),

  triggerAnchor: () =>
    request<{ anchored: boolean; reason?: string; anchor?: MerkleAnchor }>("/api/admin/anchors/run", {
      method: "POST",
    }),
};

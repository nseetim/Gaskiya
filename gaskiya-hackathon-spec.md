# Gaskiya — A Verified, Tamper-Evident Civic Opportunity Ledger

*("Gaskiya" is Hausa for "truth" — pick a different name if you prefer; this is a build spec, not a brand decision. Working name used throughout this doc.)*

**Audience for this document:** a coding agent (and the developer directing it) building this project for a hackathon. This is a build spec, not a pitch deck — it assumes the reader will implement against it directly. Treat unresolved judgment calls marked `[DECIDE]` as things to settle early, not to guess silently.

---

## 1. The problem, stated precisely

Hackathon prompt (verbatim, for traceability): *"Access to rights, services, and opportunities often depends on whether people can find information they understand and trust. Yet for many communities in Africa, essential information is fragmented, outdated, difficult to verify, or hidden behind systems that are hard to navigate."*

We are narrowing "essential information" to a concrete, high-stakes slice: **scholarships, grants, subsidies, fellowships, and job/training programs** ("opportunities"). This slice is chosen because:

- It's where fragmented/outdated/unverifiable information causes acute, individual harm (missed deadlines, missed eligibility, financial scams charging "processing fees").
- It's narrow enough to build and demo completely in a hackathon window, unlike a general civic-info wiki.
- It has a natural verification structure (issuing institutions, deadlines, eligibility criteria) that maps cleanly onto a versioned, auditable data model.
- Scams and stale reposts of expired programs are a documented, recurring pattern on the informal channels (WhatsApp groups, Facebook pages, unofficial blogs) where this information currently circulates.

**Concretely, the failure modes we're targeting:**

1. A real scholarship's deadline or eligibility criteria changes and the update never reaches the WhatsApp/Facebook copies still circulating.
2. A scam listing impersonates a real institution, asks for an upfront "processing fee," and disappears once caught — with no public record that it ever existed or that it was fraudulent.
3. A legitimate listing is quietly altered after initial trust is established (e.g., a correct link swapped for a phishing one) — a "verify-then-pivot" attack that a normal CMS/database cannot surface, because normal databases overwrite in place and don't expose edit history to end users.
4. There is no way for an ordinary user to independently confirm that what they're looking at hasn't been tampered with, short of trusting whichever platform hosts it.

---

## 2. Why this project, and why *this* research

This project is a product application of prior research by the project author: **the Distributed Merkle Cuckoo B-tree (DMC-Btree)**, described in `IET-Blockchain-paper.md` / `IEEE-conference paper.docx` in this workspace. Read those files for full technical grounding before implementing the data layer — do not re-derive the architecture from this spec alone.

The paper's core finding: conventional DAG-based blockchain alternatives (IOTA Tangle, Hashgraph) solve blockchain's scalability/throughput problems but **give up historical verifiability** — there is no structured way to audit a DAG's full history over time. The DMC-Btree closes that specific gap by combining:

| Component | What it does | Why it matters for this product |
|---|---|---|
| **Merkle-hashed balanced B-tree** | Every node and its children are cryptographically hashed at every tree level; hierarchical structure allows O(log n) indexed lookup instead of full-dataset scans. | Fast, indexed search over opportunity listings (by category, region, deadline) *and* cryptographic integrity per record — not a bolt-on hash, structural to the index itself. |
| **Cuckoo hashing for node assignment** | Deterministic two-hash-function placement with bounded-time collision resolution (eviction), distributing keys evenly across nodes without a central allocator. | Records are placed across storage shards without any single shard (or actor controlling a shard) gaining disproportionate control over which records exist or are reachable — relevant when the actors who might want a scam listing suppressed, or a legitimate one buried, include the platform operator itself. |
| **Append-only, versioned structure + IPFS-backed storage** | Every update creates a new branch and a new unique Merkle root; prior states are preserved and cannot be overwritten. | **This is the product's core feature, not a background guarantee.** Every opportunity listing's full edit history is public, cryptographically chained, and independently verifiable — this is what makes the "verify-then-pivot" scam pattern (failure mode #3 above) *detectable at all*, and what makes "this program was reported as a scam starting [date]" an unfalsifiable public record instead of a claim on a web page. |

**The pitch in one sentence:** normal opportunity boards can be silently edited and give you no way to check; this one cryptographically cannot be, and that property is what catches scams and stale info that other approaches structurally cannot see.

---

## 3. Scope: what "real, scoped-down prototype" means here

The team decided to build a **real, working implementation** of the DMC-Btree's core mechanisms — not a conventional database dressed up with blockchain vocabulary — but scoped to what's honestly buildable in a hackathon window. Be explicit in the demo and README about what's real vs. simulated; overclaiming undermines the technical credibility that's the whole point of building this on real research.

**Build for real:**
- Actual Merkle hash-chaining of every version of every record (SHA-256, chained to previous version hash).
- Actual Merkle proof generation and independent verification (a user or judge can recompute a hash and confirm it matches, without trusting the server's word for it).
- Actual Cuckoo hashing algorithm (two hash functions, eviction on collision, rehash on `Max-Loop` exceeded) assigning records to logical storage shards.
- Actual IPFS pinning of periodic batch Merkle roots (anchoring the ledger's state externally, so tampering would require rewriting IPFS-pinned history too).
- A real B-tree-indexed (or B-tree-equivalent, e.g. backed by a database's native B-tree index) search layer.

**Honestly simulated (say so in the UI/demo, don't hide it):**
- "Distributed nodes" are logical shards (e.g. 5 in-process or separate-process workers), not physically separate machines — the *algorithm* placing data across them is real; the *distribution* (multiple real machines/operators) is simulated for demo purposes.
- No consensus/BFT layer — matches the original paper's own explicit scope (Section 4.1: "the current implementation does not include a consensus algorithm"). Do not build one; it's out of scope and the paper itself flags it as future work.
- Institutional identity verification is a simple registered-keypair allowlist (admin manually onboards a small number of demo "verified institutions"), not a real-world KYC/legal verification process.

**Explicitly out of scope for the hackathon (do not attempt):**
- Real government/NGO partnerships or live data feeds.
- USSD/SMS access channel (web/PWA only per project decision).
- Native mobile apps.
- Multi-country localization beyond 1–2 demo languages.
- Payments of any kind.
- Production-grade key management / HSM-backed signing.
- Horizontal scaling beyond what's needed to demonstrate the Cuckoo-hash distribution visually.

---

## 4. User roles

| Role | Can do |
|---|---|
| **Citizen (anonymous or lightly registered)** | Search/browse listings, view trust badges, view full version history of any listing, run independent verification, submit a new listing (unverified by default), corroborate an existing listing, report a listing as suspicious/scam. |
| **Verified Institution** (university, NGO, govt agency — onboarded manually for demo) | All citizen actions, plus: publish/edit listings under a cryptographic signature that marks them as institutionally verified. |
| **Moderator/Admin** (demo-only role) | Onboard institutional keys, view the transparency/admin dashboard (shard distribution, anchor log, scam-detection stats), manually adjudicate contested trust-score edge cases. |

---

## 5. Data model

```
Institution
  id, name, public_key, verified_at, category (govt|university|ngo)

User
  id, display_name (optional/pseudonymous ok for hackathon), created_at

OpportunityListing
  id, current_version_id, category (scholarship|grant|subsidy|job|training),
  region, created_at

ListingVersion   -- the append-only, hash-chained core
  id, listing_id, version_number,
  content (title, description, eligibility, deadline, application_url, contact),
  content_hash (SHA-256 of canonicalized content),
  prev_version_hash (chains to previous version; null for v1),
  author_type (institution|user), author_id,
  signature (present if author_type = institution),
  shard_id (assigned via Cuckoo hashing),
  created_at

CorroborationRecord
  id, listing_id, version_id_corroborated, corroborator_id, created_at, note

ScamReport
  id, listing_id, version_id_reported, reporter_id, reason, created_at

TrustScoreSnapshot   -- also append-only/versioned, mirroring the ledger's own philosophy
  id, listing_id, score, status (verified|community|flagged|confirmed_scam),
  computed_at, inputs_summary (json: signature? / corroboration_count / report_count / anomaly_flags)

MerkleAnchor
  id, batch_root_hash, ipfs_cid, anchored_at, version_ids_included (array)
```

**Key design rule:** `ListingVersion` rows are never updated or deleted. An "edit" is always a new row with `prev_version_hash` pointing at the prior version's `content_hash`. `OpportunityListing.current_version_id` is the only mutable pointer, and even that should itself be logged (which version was "current" at what time) if time allows — this is what makes "show me this listing as it appeared on [date]" possible, directly mirroring the paper's historical-verifiability claim.

---

## 6. Scam / stale-info detection logic

This is the product's differentiating feature. Implement as a deterministic, explainable rule engine for the hackathon — not opaque ML — because judges (and real users) need to see *why* something is flagged, and because the paper's contribution is about verifiable structure, not statistical inference.

**Signals (each computed per listing, recomputed on every new version/event):**

1. **Institutional signature present and valid** → strong positive signal.
2. **Independent corroboration count** — corroborations from accounts with no shared institution/IP-class as the original author count toward a `Verified` threshold (`[DECIDE]` exact N, suggest N=3 for demo purposes). Corroborations from the same author or obviously coordinated accounts should not count (basic Sybil resistance — even a simple "distinct account age + no shared session fingerprint" check is enough for a hackathon).
3. **Post-verification pivot detection** — flag a listing if, *after* it reached `Verified` or received corroboration, a subsequent version changes `application_url`, `contact`, or any payment-related field in `content` while leaving other fields untouched. This is the flagship detection case: build a specific diff-and-flag routine for it (`detectPostVerificationPivot(versions[])`) and make sure the demo seed data includes one clean example of this exact pattern.
4. **Duplicate/clone detection** — hash-compare new submissions' normalized content against prior `ListingVersion.content_hash` values and a small seeded "known scam corpus"; near-duplicate (fuzzy match, e.g. simple Jaccard/shingling on normalized text) flags as likely clone.
5. **Structural red-flag heuristics** — simple rule + one LLM-assisted pass (Claude API) over listing content flagging: requests for upfront payment framed as a "fee to release funds," urgency/pressure language, claimed institutional affiliation whose domain doesn't match the institution's registered domain.
6. **Community reports** — each `ScamReport` decays the trust score; N independent reports (`[DECIDE]`, suggest N=5 or admin-adjudicated below that) moves status to `flagged`; admin can promote to `confirmed_scam`.

**Status lifecycle (each transition appended as a new `TrustScoreSnapshot`, never overwritten):**

`unverified` → `community_verified` (corroboration threshold met) → `institutionally_verified` (signed by onboarded institution) ; any status → `flagged` (pivot detected OR report threshold) → `confirmed_scam` (admin adjudication).

**The demo-defining detail:** because every `TrustScoreSnapshot` is itself append-only, the UI can show a listing's full trust trajectory over time ("Verified on Mar 2 → Flagged on Mar 14 after application link changed"). This single UI element is the clearest possible demonstration of the paper's "historical verifiability" contribution translated into a user-facing feature. Prioritize building this over any other single feature if time runs short.

---

## 7. API surface (suggested; adjust to chosen framework)

```
POST   /api/listings                        create (defaults to unverified)
POST   /api/listings/:id/versions            submit an edit (new ListingVersion)
GET    /api/listings/:id                     current version + trust status
GET    /api/listings/:id/history             full version chain, each entry showing
                                              author, signature (if any), diff from previous
GET    /api/listings/:id/verify              recompute hash chain + return Merkle proof
                                              + IPFS CID of the relevant anchor, for
                                              independent client-side verification
POST   /api/listings/:id/corroborate
POST   /api/listings/:id/report
GET    /api/search?category=&region=&deadline_before=&status=
POST   /api/institutions                     admin: onboard a verified institution keypair
GET    /api/admin/shards                     shard/cuckoo-distribution stats (for transparency dashboard)
GET    /api/admin/anchors                    list of Merkle batch anchors + IPFS CIDs
```

---

## 8. Frontend (Web/PWA)

1. **Search & Browse** — filter by category/region/deadline; each result card shows a trust badge (Verified / Community-sourced / Flagged / Confirmed Scam) with a one-line reason.
2. **Listing Detail** — full content; trust badge with expandable rationale; **"View full history"** timeline (the flagship view — show each version, who authored it, what changed, and the trust-status trajectory alongside it); **"Verify"** button that runs client-side hash recomputation against the returned Merkle proof and IPFS CID and shows a pass/fail result, not just a trust-me badge; corroborate / report actions.
3. **Submit / Edit Listing** — plain form; institutions get a "sign & publish" flow instead of anonymous submission.
4. **Institution Console** — for onboarded demo institutions to publish/edit signed listings.
5. **Transparency / Admin Dashboard** — this sells the technical story to judges: live Cuckoo-hash shard distribution (bar chart of records per shard, ideally with a visible rebalance if a shard is added), the Merkle anchor log with IPFS CIDs (clickable, resolving to a real IPFS gateway), and aggregate scam-detection stats.

`[DECIDE]` PWA installability (manifest + service worker for offline browsing of last-fetched listings) is a nice authenticity touch for the "many communities" framing given the web-only decision, but treat as stretch, not core.

---

## 9. Suggested tech stack

- **Frontend:** Next.js + TypeScript (React), Tailwind for speed. PWA manifest if time allows.
- **Backend:** Rust (Actix web) — implement the DMC-Btree-lite engine as its own module (`/engine`), independent of the web framework, so it's presentable as a standalone technical artifact.
- **Practical index/store:** PostgreSQL or SQLite for the queryable listing/version tables (this is what backs the "B-tree indexing" claim — native DB indexes are genuinely B-trees; be upfront that you're not hand-rolling a B-tree from scratch unless `[DECIDE]` the team wants to for extra technical-depth credit).
- **Custom-built (do hand-roll these — they're the actual differentiator):** Merkle hash-chaining module, Cuckoo hashing shard-assignment module, Merkle proof generation/verification.
- **Decentralized anchoring:** `ipfs-http-client` against a public gateway, or a pinning service (web3.storage / Pinata) for reliability during the demo — have a local IPFS daemon as fallback in case the pinning service is unreachable during judging.
- **AI-assist:** Claude API for scam-language heuristic pass, plain-language eligibility summaries, and optional translation for demo authenticity.
- **Deploy:** Vercel (frontend) + Railway/Render (backend + DB) for a live demo URL.

---

## 10. Build order (prioritized for a time-boxed hackathon)

Treat each phase as a checkpoint with a demoable increment — never leave the project in a state where nothing runnable exists.

0. **Setup** — repo scaffold, schema/migrations, seed script with ~10 realistic sample listings, including at minimum: one clean `institutionally_verified` example, one `community_verified` example, one exhibiting the exact post-verification-pivot pattern, and one obvious upfront-fee scam pattern.
1. **Versioning engine core** — `ListingVersion` hash-chaining, Merkle proof generation/verification, minimal CRUD API. Demoable increment: create a listing, edit it, prove the chain via API response alone (curl-level demo).
2. **Cuckoo shard assignment + search** — implement the real algorithm, wire to search API.
3. **IPFS anchoring** — periodic batch root pin, CID retrieval, `/verify` endpoint returns something a client can independently check.
4. **Trust scoring + detection rules** — corroboration, reports, pivot detection, status lifecycle, `TrustScoreSnapshot` history.
5. **Frontend** — search/browse → listing detail with history timeline + verify button (build this before institution console / admin dashboard if forced to cut scope) → submit/edit → institution console → transparency dashboard.
6. **AI-assist polish** — plain-language summaries, scam-language heuristic, translation.
7. **Demo rehearsal** — see Section 11. Budget real time for this; a rehearsed 3-minute demo of a narrower build beats an unrehearsed walkthrough of a broader one.

If time is critically short, the non-negotiable MVP is: **hash-chained versioning + history timeline UI + one working pivot-detection example + independent verify button.** Everything else (shards, IPFS, admin dashboard, AI features) is real added value but is cuttable in that order if forced.

---

## 11. Demo script (~3 minutes)

1. **Search & browse** — a citizen looking for a scholarship sees trust badges at a glance (Verified/Community/Flagged).
2. **Open the flagged listing** — walk the version-history timeline; point to the exact version where the application link/fee field changed *after* the listing had already been verified/corroborated. "A normal database or a DAG-based ledger can't surface this — nothing here was ever overwritten, so the pivot is structurally visible, not something we had to remember to log."
3. **Hit Verify** — show the client independently recomputing the hash chain against the returned Merkle proof and IPFS CID, passing/failing without trusting the server's say-so.
4. **Transparency dashboard** — show the Cuckoo-hash shard distribution and the IPFS anchor log with clickable CIDs. "This is a real, working implementation of published research, not blockchain terminology applied after the fact."
5. **Institution console** — a verified university signs and publishes a real scholarship; watch it get an institutional badge immediately.
6. **Close** — tie back to the prompt: fragmented, unverifiable civic-opportunity information → a fast-searchable, tamper-evident, independently verifiable registry, built on a specific, defensible technical property (historical verifiability) that alternative architectures explicitly lack.

---

## 12. Known limitations (state these proactively — mirrors the source paper's own honesty about scope)

- No consensus/BFT layer; a determined attacker with write access to the primary datastore could still corrupt un-anchored state between IPFS anchor batches. Mitigate by anchoring frequently and stating the anchor interval explicitly.
- Institutional identity verification is a manually-onboarded allowlist, not a legal/KYC process — fine for a hackathon demo, explicitly not production-ready.
- Trust scoring is a deterministic rule engine tuned by hand for the demo dataset; it is not adversarially hardened and would need real-world tuning (and likely a proper reputation/Sybil-resistance model) before production use.
- "Distributed nodes" are simulated logical shards, not independently operated machines — the placement algorithm is real, the operational independence is not (yet).

---

## 13. Success criteria mapped to likely judging dimensions

- **Problem-solution fit:** directly targets a named failure mode (stale/unverifiable/scam opportunity info) with a mechanism, not just a UI, addressing it.
- **Technical innovation:** grounded in the team's own prior published research (`IET-Blockchain-paper.md`), with a real (if scoped) implementation of its distinguishing mechanism — not blockchain-as-decoration.
- **Usability/accessibility:** trust badges and a plain-language history timeline make cryptographic guarantees legible to a non-technical user, not just to judges reading the code.
- **Feasibility/impact:** narrow enough to actually work end-to-end in the demo; the underlying mechanism (versioned, auditable civic records) generalizes beyond opportunities to the broader "civic life" framing in the prompt, which is worth saying explicitly if asked.

---

## 14. Open decisions for the team (`[DECIDE]` items collected)

- Corroboration threshold N for `community_verified` status (suggested: 3).
- Report threshold N for auto-`flagged` status (suggested: 5, admin-adjudicated below that).
- Whether to hand-roll the B-tree itself for extra technical-depth credit, or rely on the backing database's native B-tree index and focus hand-rolled effort on the Merkle/Cuckoo layers (recommended, given time constraints).
- Whether to add PWA installability/offline browsing (nice authenticity touch, not core).
- Final project name/branding (this doc uses "Gaskiya" as a placeholder).

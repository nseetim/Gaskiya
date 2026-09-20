# Gaskiya

A verified, tamper-evident civic opportunity ledger. See `gaskiya-hackathon-spec.md` for the full build spec and `DECISIONS.md` for resolved `[DECIDE]` items.

## Status

Build order phases 0-5 are done: setup, versioning engine core, Cuckoo shard assignment + search, IPFS anchoring, the trust-scoring/detection rule engine, and the frontend. Also added a watchlist feature (tracks status changes on listings a citizen is applying to) to satisfy the hackathon brief's "help people engage with governments and public services" requirement — see `DECISIONS.md` for the full rationale and the operating-constraints mapping. Not yet built: AI-assist polish (phase 6).

The trust engine (`server/src/trust.rs`) implements every signal from spec Section 6: real Ed25519 institutional signatures (see below), corroboration/report thresholds, the flagship `detect_post_verification_pivot` routine, Jaccard-shingling duplicate/clone detection against the ledger's own flagged listings, and rule-based structural red flags (upfront-fee language). The LLM-assisted half of the structural-heuristics signal is deferred to the AI-assist phase, as the build order specifies. Status transitions are append-only `TrustScoreSnapshot`s recomputed on every version/corroboration/report — the full trajectory (e.g. `unverified -> community_verified -> flagged`) is visible via `GET /api/listings/:id/history`.

Institutional signatures are real, not a placeholder: `POST /api/institutions` onboards a keypair (generating one server-side if none is supplied, returned once), and institution-authored listings are rejected with 400 unless they carry a valid Ed25519 signature over the version's `content_hash`, verified against the registered public key.

There is no login system by design (spec Section 4: citizens are "anonymous or lightly registered"). `user_id`/`author_id`/etc. are client-generated opaque UUIDs held in `localStorage` once the frontend exists — possession of the UUID is the only access control, which is fine for a hackathon demo but is explicitly not real auth (see DECISIONS.md).

## Layout

```
backend/
  engine/      hand-rolled Merkle hash-chaining, Merkle proof tree, Cuckoo shard hashing, Ed25519 signing (no web/DB deps)
  server/      Actix-web API on top of `engine`, using sqlx + Postgres, IPFS anchoring, and the trust engine
  migrations/  Postgres schema (Section 5 data model)
frontend/      Next.js + TypeScript + Tailwind + shadcn/ui (Base UI) PWA-less web client
docker-compose.yml   local Postgres + local IPFS (Kubo) daemon for dev
```

## Running locally

```bash
docker compose up -d
cd backend && cp .env.example .env && cargo run -p server   # http://127.0.0.1:8080
cd frontend && npm install && npm run dev                    # http://localhost:3000
```

Migrations run automatically on startup. A background task anchors any unanchored `ListingVersion`s to the local IPFS node every `ANCHOR_INTERVAL_SECS` (default 20s, tune down for a faster demo) — or trigger one immediately with `POST /api/admin/anchors/run`. The Cuckoo shard table is in-process state that resets on restart; it's rebuilt by replaying past listing IDs from the DB at startup, so the transparency dashboard stays accurate across restarts.

## Frontend

Five views per spec Section 8, all client components talking to the API directly (CORS is wide open on the backend — see DECISIONS.md):

- **Search & Browse** (`/`) — filters, trust badges with one-line reasons.
- **Listing Detail** (`/listings/:id`) — full content, expandable trust rationale, history timeline + trust trajectory, and a **Verify** button that independently recomputes the hash chain and the Merkle-anchor inclusion proof *in the browser* (`src/lib/canonical.ts`) rather than trusting the server's own verdict.
- **Submit/Edit** (`/submit`, `/listings/:id/edit`) — plain anonymous forms.
- **Institution Console** (`/institution`) — generates a real Ed25519 keypair *in the browser* (`@noble/ed25519`, verified byte-for-byte interoperable with the Rust `ed25519-dalek` engine — see DECISIONS.md), registers only the public key, and signs listings client-side. The private key never leaves the browser.
- **Transparency Dashboard** (`/admin`) — live Cuckoo shard bar chart, Merkle anchor log with clickable IPFS links, moderator confirm-scam action.
- **My Applications** (`/watchlist`) — our added feature; surfaces status-change alerts for listings a citizen is tracking.

Live-tested end-to-end in a real headless browser (not just curl): registered an institution, generated its keypair client-side, signed and published a listing, watched it, corroborated it, then edited it with a pivoted `application_url`/`contact` and confirmed the trust engine flagged it and the watchlist surfaced the alert — the exact flagship scenario, driven entirely through the UI.

## Tests

```bash
cd backend
cargo test --workspace   # 16 engine tests (Merkle/Cuckoo/signatures) + 8 trust-engine tests
```

## API implemented so far

```
POST   /api/listings                   create a listing (v1)
POST   /api/listings/:id/versions      submit an edit (new hash-chained version)
GET    /api/listings/:id               current version
GET    /api/listings/:id/history       full version chain + per-version diff
GET    /api/listings/:id/verify        recompute hash chain + per-version content hash + Merkle-anchor
                                        inclusion proof and IPFS CID, independent of server say-so
GET    /api/search?category=&region=&deadline_before=&status=
POST   /api/listings/:id/corroborate   vouch for a listing (self-corroboration rejected)
POST   /api/listings/:id/report        report a listing as suspicious/scam
POST   /api/institutions               admin: onboard an institution keypair (generates one if omitted)
GET    /api/institutions               list onboarded institutions
POST   /api/admin/listings/:id/confirm-scam   admin adjudication (sticky terminal status)
POST   /api/watchlist                          start tracking a listing (idempotent)
GET    /api/watchlist/:user_id                 watched listings + whether status changed since last seen
POST   /api/watchlist/entry/:watch_id/acknowledge   dismiss a status-change alert
DELETE /api/watchlist/entry/:watch_id          stop tracking
GET    /api/admin/shards               live Cuckoo shard distribution
GET    /api/admin/anchors              list of Merkle batch anchors + IPFS CIDs
POST   /api/admin/anchors/run          manually trigger an anchor batch (demo control)
```

Curl-level demo (institutionally-verified listing, real Ed25519 signature):

```bash
# 1. Onboard an institution (generates a keypair; the private key is shown once)
curl -X POST localhost:8080/api/institutions -H 'Content-Type: application/json' \
  -d '{"name": "Ministry of Youth Development", "category": "govt"}'
# -> {"institution": {"id": "<inst_id>", "public_key": "..."}, "private_key_hex": "<priv>"}

# 2. Sign the listing content with that private key (content_hash must match exactly)
CONTENT='{"title":"National Youth STEM Scholarship","description":"Full tuition award for undergraduate STEM students.","eligibility":"Nigerian citizens aged 16-22 enrolled in a STEM program.","deadline":"2026-12-01T00:00:00Z","application_url":"https://real-ministry.gov.ng/apply","contact":"scholarships@real-ministry.gov.ng"}'
echo "$CONTENT" | cargo run -q -p engine --example sign_content -- <priv>
# -> {"content_hash": "...", "signature": "..."}

# 3. Publish, using the returned signature
curl -X POST localhost:8080/api/listings -H 'Content-Type: application/json' -d "{
  \"category\": \"scholarship\", \"region\": \"Kano, Nigeria\",
  \"content\": $CONTENT,
  \"author_type\": \"institution\", \"author_id\": \"<inst_id>\", \"signature\": \"<signature>\"
}"
# -> trust.status: "institutionally_verified"
```

Or skip signing entirely with `"author_type": "user", "author_id": "<any-uuid>", "signature": null` for a plain community submission.

Then `POST /api/listings/:id/versions` with a changed `application_url`/`contact` (after 3 corroborations or a valid institution signature) to trigger real post-verification-pivot detection, and hit `GET /api/listings/:id/history` to see the full trust trajectory (e.g. `unverified -> community_verified -> flagged`) and `GET /api/listings/:id/verify` for the independent hash-chain + Merkle-anchor check.

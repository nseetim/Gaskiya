-- Section 5 data model. IDs are generated application-side (uuid v4) rather
-- than via DB defaults, so no pgcrypto/uuid-ossp extension is required.

CREATE TABLE institutions (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    public_key TEXT NOT NULL,
    verified_at TIMESTAMPTZ,
    category TEXT NOT NULL CHECK (category IN ('govt', 'university', 'ngo'))
);

CREATE TABLE users (
    id UUID PRIMARY KEY,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE opportunity_listings (
    id UUID PRIMARY KEY,
    current_version_id UUID,
    category TEXT NOT NULL CHECK (category IN ('scholarship', 'grant', 'subsidy', 'job', 'training')),
    region TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Append-only, hash-chained core. Rows here are never updated or deleted;
-- an edit is always a new row with prev_version_hash pointing at the prior
-- version's content_hash.
CREATE TABLE listing_versions (
    id UUID PRIMARY KEY,
    listing_id UUID NOT NULL REFERENCES opportunity_listings(id),
    version_number INT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    eligibility TEXT NOT NULL,
    deadline TIMESTAMPTZ NOT NULL,
    application_url TEXT NOT NULL,
    contact TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    prev_version_hash TEXT,
    author_type TEXT NOT NULL CHECK (author_type IN ('institution', 'user')),
    author_id UUID NOT NULL,
    signature TEXT,
    shard_id INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (listing_id, version_number)
);

ALTER TABLE opportunity_listings
    ADD CONSTRAINT fk_current_version FOREIGN KEY (current_version_id) REFERENCES listing_versions(id);

-- Which version was "current" at what time, so "show me this listing as it
-- appeared on [date]" is answerable even though current_version_id itself
-- is a mutable pointer (spec Section 5).
CREATE TABLE current_version_log (
    id UUID PRIMARY KEY,
    listing_id UUID NOT NULL REFERENCES opportunity_listings(id),
    version_id UUID NOT NULL REFERENCES listing_versions(id),
    became_current_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE corroboration_records (
    id UUID PRIMARY KEY,
    listing_id UUID NOT NULL REFERENCES opportunity_listings(id),
    version_id_corroborated UUID NOT NULL REFERENCES listing_versions(id),
    corroborator_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    note TEXT
);

CREATE TABLE scam_reports (
    id UUID PRIMARY KEY,
    listing_id UUID NOT NULL REFERENCES opportunity_listings(id),
    version_id_reported UUID NOT NULL REFERENCES listing_versions(id),
    reporter_id UUID NOT NULL,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Append-only, mirroring the ledger's own philosophy: a listing's full
-- trust trajectory is queryable, never just its latest status.
CREATE TABLE trust_score_snapshots (
    id UUID PRIMARY KEY,
    listing_id UUID NOT NULL REFERENCES opportunity_listings(id),
    score DOUBLE PRECISION NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('unverified', 'community_verified', 'institutionally_verified', 'flagged', 'confirmed_scam')
    ),
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    inputs_summary JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE merkle_anchors (
    id UUID PRIMARY KEY,
    batch_root_hash TEXT NOT NULL,
    ipfs_cid TEXT,
    anchored_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    version_ids_included UUID[] NOT NULL
);

CREATE INDEX idx_listing_versions_listing_id ON listing_versions(listing_id);
CREATE INDEX idx_opportunity_listings_category ON opportunity_listings(category);
CREATE INDEX idx_opportunity_listings_region ON opportunity_listings(region);
CREATE INDEX idx_listing_versions_deadline ON listing_versions(deadline);
CREATE INDEX idx_trust_score_snapshots_listing_id ON trust_score_snapshots(listing_id);

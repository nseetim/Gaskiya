-- Lets a citizen track a listing through their own application process, not
-- just at the moment they first found it. If the listing's trust status
-- changes after they started watching it (e.g. flagged after a
-- post-verification pivot), that's surfaced back to them the next time they
-- check — closing the loop from "find information" to "stay safe while
-- acting on it."
CREATE TABLE watchlist_entries (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    listing_id UUID NOT NULL REFERENCES opportunity_listings(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_status TEXT NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, listing_id)
);

CREATE INDEX idx_watchlist_entries_user_id ON watchlist_entries(user_id);

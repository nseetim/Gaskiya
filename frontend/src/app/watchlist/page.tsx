"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api";
import { getOrCreateUserId } from "@/lib/identity";
import type { WatchEntry } from "@/lib/types";
import { TrustBadge } from "@/components/trust-badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { AlertTriangle } from "lucide-react";
import { toast } from "sonner";

export default function WatchlistPage() {
  const [entries, setEntries] = useState<WatchEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const { watchlist } = await api.getWatchlist(getOrCreateUserId());
      setEntries(watchlist);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not load your watchlist.");
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function handleAcknowledge(watchId: string) {
    try {
      await api.acknowledgeWatch(watchId);
      toast.success("Acknowledged.");
      await load();
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not acknowledge.");
    }
  }

  async function handleUnwatch(watchId: string) {
    try {
      await api.unwatch(watchId);
      toast("Removed from watchlist.");
      await load();
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not remove.");
    }
  }

  return (
    <div className="space-y-6">
      <div className="rounded-xl border bg-gradient-to-br from-primary/10 via-transparent to-transparent p-6">
        <h1 className="text-2xl font-semibold tracking-tight">My Applications</h1>
        <p className="mt-1 text-muted-foreground">
          Track listings you&apos;re applying to. If a listing&apos;s trust status changes after you
          started — for example, flagged after its application link was swapped — you&apos;ll see it
          here, not just at the moment you first found it.
        </p>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertTitle>Couldn&apos;t load your watchlist</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {entries === null && !error && (
        <div className="space-y-3">
          <Skeleton className="h-24 w-full" />
          <Skeleton className="h-24 w-full" />
        </div>
      )}

      {entries !== null && entries.length === 0 && (
        <p className="text-muted-foreground">
          You&apos;re not watching any listings yet. Open a listing and click &ldquo;Watch this
          listing&rdquo; while you apply.
        </p>
      )}

      <div className="space-y-3">
        {entries?.map((entry) => (
          <Card key={entry.watch_id} className={entry.status_changed ? "border-amber-400" : undefined}>
            <CardHeader className="flex flex-row items-start justify-between gap-2">
              <div>
                <Link href={`/listings/${entry.listing_id}`} className="font-semibold hover:underline">
                  {entry.current_version?.title ?? "Listing"}
                </Link>
                <p className="text-xs text-muted-foreground">
                  Watching since {new Date(entry.watching_since).toLocaleDateString()}
                </p>
              </div>
              <TrustBadge status={entry.current_status} />
            </CardHeader>
            <CardContent>
              {entry.status_changed ? (
                <Alert variant="destructive" className="mb-3">
                  <AlertTriangle className="h-4 w-4" />
                  <AlertTitle>Status changed since you last checked</AlertTitle>
                  <AlertDescription>
                    Was <strong>{entry.last_seen_status}</strong>, now <strong>{entry.current_status}</strong>.
                  </AlertDescription>
                </Alert>
              ) : null}
              <div className="flex gap-2">
                {entry.status_changed && (
                  <Button size="sm" variant="outline" onClick={() => handleAcknowledge(entry.watch_id)}>
                    Acknowledge
                  </Button>
                )}
                <Button size="sm" variant="ghost" onClick={() => handleUnwatch(entry.watch_id)}>
                  Stop watching
                </Button>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}

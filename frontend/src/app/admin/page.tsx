"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api";
import type { MerkleAnchor, ShardDistribution } from "@/lib/types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { ShardChart } from "@/components/shard-chart";
import { ExternalLink } from "lucide-react";
import { toast } from "sonner";

export default function AdminDashboardPage() {
  const [shards, setShards] = useState<ShardDistribution | null>(null);
  const [anchors, setAnchors] = useState<MerkleAnchor[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scamListingId, setScamListingId] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [s, a] = await Promise.all([api.shardDistribution(), api.anchors()]);
      setShards(s);
      setAnchors(a.anchors);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not reach the Gaskiya API.");
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function handleTriggerAnchor() {
    setBusy(true);
    try {
      const result = await api.triggerAnchor();
      toast.success(result.anchored ? "New anchor batch created." : "Nothing pending to anchor.");
      await load();
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not trigger anchor.");
    } finally {
      setBusy(false);
    }
  }

  async function handleConfirmScam() {
    if (!scamListingId.trim()) return;
    setBusy(true);
    try {
      await api.confirmScam(scamListingId.trim());
      toast.success("Listing marked as confirmed scam.");
      setScamListingId("");
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not confirm scam.");
    } finally {
      setBusy(false);
    }
  }

  const totalAnchoredVersions = anchors?.reduce((sum, a) => sum + a.version_count, 0) ?? 0;

  return (
    <div className="space-y-8">
      <div className="rounded-xl border bg-gradient-to-br from-primary/10 via-transparent to-transparent p-6">
        <h1 className="text-2xl font-semibold tracking-tight">Transparency Dashboard</h1>
        <p className="mt-1 text-muted-foreground">
          This is a real, working implementation of published research, not blockchain terminology
          applied after the fact — the shard placement below is a live Cuckoo hash table, and every
          anchor is a real IPFS pin you can open yourself.
        </p>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertTitle>Couldn&apos;t load dashboard data</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Cuckoo shard distribution</CardTitle>
          </CardHeader>
          <CardContent>
            {shards ? (
              <>
                <ShardChart distribution={shards.distribution} />
                <p className="mt-2 text-xs text-muted-foreground">
                  {shards.num_shards} logical shards — simulated nodes (spec Section 3), real
                  placement algorithm.
                </p>
              </>
            ) : (
              <Skeleton className="h-56 w-full" />
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Scam-detection stats</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm">
            <p>
              <span className="font-medium">{anchors?.length ?? "—"}</span> anchor batches published
            </p>
            <p>
              <span className="font-medium">{totalAnchoredVersions}</span> listing versions anchored
              to IPFS
            </p>
            <p className="text-xs text-muted-foreground">
              Per-listing trust trajectories (corroborations, reports, pivot detections) are visible
              on each listing&apos;s detail page — this panel summarizes the anchoring layer.
            </p>
            <Button size="sm" variant="outline" onClick={handleTriggerAnchor} disabled={busy} className="mt-2">
              Trigger anchor batch now
            </Button>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Merkle anchor log</CardTitle>
        </CardHeader>
        <CardContent>
          {anchors === null && <Skeleton className="h-32 w-full" />}
          {anchors !== null && anchors.length === 0 && (
            <p className="text-sm text-muted-foreground">No anchors published yet.</p>
          )}
          {anchors !== null && anchors.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-xs text-muted-foreground">
                    <th className="py-2 pr-4">Anchored at</th>
                    <th className="py-2 pr-4">Versions</th>
                    <th className="py-2 pr-4">Batch root</th>
                    <th className="py-2">IPFS CID</th>
                  </tr>
                </thead>
                <tbody>
                  {anchors.map((a) => (
                    <tr key={a.id} className="border-b last:border-0">
                      <td className="py-2 pr-4 whitespace-nowrap">{new Date(a.anchored_at).toLocaleString()}</td>
                      <td className="py-2 pr-4">{a.version_count}</td>
                      <td className="py-2 pr-4 font-mono text-xs">{a.batch_root_hash.slice(0, 16)}…</td>
                      <td className="py-2">
                        {a.ipfs_gateway_url ? (
                          <a
                            href={a.ipfs_gateway_url}
                            target="_blank"
                            rel="noreferrer"
                            className="inline-flex items-center gap-1 font-mono text-xs underline"
                          >
                            {a.ipfs_cid?.slice(0, 12)}… <ExternalLink className="h-3 w-3" />
                          </a>
                        ) : (
                          "—"
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Moderator adjudication</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-wrap items-end gap-2">
          <div className="flex flex-col gap-1">
            <label className="text-xs font-medium text-muted-foreground">Listing ID</label>
            <Input
              value={scamListingId}
              onChange={(e) => setScamListingId(e.target.value)}
              placeholder="uuid"
              className="w-80"
            />
          </div>
          <Button variant="destructive" onClick={handleConfirmScam} disabled={busy}>
            Confirm as scam
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}

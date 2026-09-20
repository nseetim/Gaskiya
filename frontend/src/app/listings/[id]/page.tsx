"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api";
import type { HistoryResult, ListingDetail } from "@/lib/types";
import { TrustBadge, trustStatusReason } from "@/components/trust-badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { getOrCreateUserId } from "@/lib/identity";
import { contentHash, verifyMerkleProof } from "@/lib/canonical";
import { toast } from "sonner";
import { CheckCircle2, XCircle, ShieldQuestion, ExternalLink } from "lucide-react";

interface VerifyOutcome {
  allHashesMatch: boolean;
  chainValid: boolean;
  anchorStatus: "verified" | "not_yet_anchored" | "failed";
  ipfsGatewayUrl?: string;
  details: string[];
}

export default function ListingDetailPage() {
  const params = useParams<{ id: string }>();
  const listingId = params.id;

  const [detail, setDetail] = useState<ListingDetail | null>(null);
  const [history, setHistory] = useState<HistoryResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [verifyOutcome, setVerifyOutcome] = useState<VerifyOutcome | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [reportReason, setReportReason] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [d, h] = await Promise.all([api.getListing(listingId), api.getHistory(listingId)]);
      setDetail(d);
      setHistory(h);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not load this listing.");
    }
  }, [listingId]);

  useEffect(() => {
    load();
  }, [load]);

  async function handleVerify() {
    if (!history) return;
    setVerifying(true);
    setVerifyOutcome(null);
    try {
      const versions = history.history.map((h) => h.version);
      const details: string[] = [];
      let allHashesMatch = true;
      let chainValid = true;

      for (let i = 0; i < versions.length; i++) {
        const v = versions[i];
        const recomputed = await contentHash({
          title: v.title,
          description: v.description,
          eligibility: v.eligibility,
          deadline: v.deadline,
          application_url: v.application_url,
          contact: v.contact,
        });
        if (recomputed !== v.content_hash) {
          allHashesMatch = false;
          details.push(`Version ${v.version_number}: content_hash does NOT match recomputed hash.`);
        }
        const expectedPrev = i === 0 ? null : versions[i - 1].content_hash;
        if (v.prev_version_hash !== expectedPrev) {
          chainValid = false;
          details.push(`Version ${v.version_number}: prev_version_hash does not chain to the prior version.`);
        }
      }

      // Independently verify the Merkle-anchor inclusion proof for the
      // current version, if one exists yet, against the server-supplied
      // (but not trusted) proof + root.
      const verifyData = await api.verify(listingId);
      const latest = verifyData.versions[verifyData.versions.length - 1];
      let anchorStatus: VerifyOutcome["anchorStatus"] = "not_yet_anchored";
      let ipfsGatewayUrl: string | undefined;

      if (latest?.anchor) {
        const proofOk = await verifyMerkleProof(
          latest.anchor.proof.leaf_hash,
          latest.anchor.proof.siblings,
          latest.anchor.batch_root_hash,
        );
        const leafMatchesVersion = latest.anchor.proof.leaf_hash === latest.stored_content_hash;
        anchorStatus = proofOk && leafMatchesVersion ? "verified" : "failed";
        ipfsGatewayUrl = latest.anchor.ipfs_gateway_url ?? undefined;
        if (!proofOk) details.push("Merkle inclusion proof does NOT verify against the anchored batch root.");
        if (!leafMatchesVersion) details.push("Anchor proof's leaf hash does not match this version's content hash.");
      } else {
        details.push("This version hasn't been included in an IPFS anchor batch yet.");
      }

      setVerifyOutcome({ allHashesMatch, chainValid, anchorStatus, ipfsGatewayUrl, details });
    } catch {
      toast.error("Verification failed to run — see console.");
    } finally {
      setVerifying(false);
    }
  }

  async function handleCorroborate() {
    setBusy(true);
    try {
      await api.corroborate(listingId, getOrCreateUserId());
      toast.success("Corroboration recorded.");
      await load();
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not corroborate.");
    } finally {
      setBusy(false);
    }
  }

  async function handleReport() {
    if (!reportReason.trim()) {
      toast.error("Please describe why you're reporting this listing.");
      return;
    }
    setBusy(true);
    try {
      await api.report(listingId, getOrCreateUserId(), reportReason);
      toast.success("Report submitted.");
      setReportReason("");
      await load();
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not submit report.");
    } finally {
      setBusy(false);
    }
  }

  async function handleWatch() {
    setBusy(true);
    try {
      await api.watch(getOrCreateUserId(), listingId);
      toast.success("Added to your watchlist — you'll see it flagged here if its status changes.");
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not add to watchlist.");
    } finally {
      setBusy(false);
    }
  }

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertTitle>Couldn&apos;t load this listing</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    );
  }

  if (!detail || !history) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-2/3" />
        <Skeleton className="h-32 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  const v = detail.current_version;
  const trust = detail.trust;

  return (
    <div className="space-y-8">
      <div className="rounded-xl border bg-gradient-to-br from-primary/10 via-transparent to-transparent p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h1 className="text-2xl font-semibold tracking-tight">{v.title}</h1>
          {trust && <TrustBadge status={trust.status} />}
        </div>
        <p className="mt-1 text-sm">
          <span className="font-medium text-primary uppercase">{detail.listing.category}</span>
          <span className="text-muted-foreground">
            {" "}
            · {detail.listing.region} · Shard {v.shard_id}
          </span>
        </p>
      </div>

      {trust && (
        <Alert>
          <ShieldQuestion className="h-4 w-4" />
          <AlertTitle>Why this status?</AlertTitle>
          <AlertDescription>
            {trustStatusReason(trust.status, trust.inputs_summary)}
            <details className="mt-2">
              <summary className="cursor-pointer text-xs underline">Full rationale (raw signals)</summary>
              <pre className="mt-2 overflow-x-auto rounded bg-muted p-2 text-xs">
                {JSON.stringify(trust.inputs_summary, null, 2)}
              </pre>
            </details>
          </AlertDescription>
        </Alert>
      )}

      <Card>
        <CardContent className="grid gap-4 pt-6 sm:grid-cols-2">
          <div>
            <p className="text-xs font-medium text-muted-foreground">Description</p>
            <p className="text-sm">{v.description}</p>
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground">Eligibility</p>
            <p className="text-sm">{v.eligibility}</p>
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground">Deadline</p>
            <p className="text-sm">{new Date(v.deadline).toLocaleDateString()}</p>
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground">Apply / Contact</p>
            <p className="text-sm">
              <a href={v.application_url} target="_blank" rel="noreferrer" className="underline">
                {v.application_url}
              </a>
              <br />
              {v.contact}
            </p>
          </div>
        </CardContent>
      </Card>

      <div className="flex flex-wrap gap-2">
        <Button onClick={handleVerify} disabled={verifying}>
          {verifying ? "Verifying..." : "Verify"}
        </Button>
        <Button variant="outline" onClick={handleCorroborate} disabled={busy}>
          Corroborate
        </Button>
        <Button variant="outline" onClick={handleWatch} disabled={busy}>
          Watch this listing
        </Button>
        <Button
          variant="outline"
          nativeButton={false}
          render={<Link href={`/listings/${listingId}/edit`} />}
        >
          Suggest an edit
        </Button>
      </div>

      {verifyOutcome && (
        <Alert variant={verifyOutcome.allHashesMatch && verifyOutcome.chainValid && verifyOutcome.anchorStatus !== "failed" ? "default" : "destructive"}>
          {verifyOutcome.allHashesMatch && verifyOutcome.chainValid && verifyOutcome.anchorStatus !== "failed" ? (
            <CheckCircle2 className="h-4 w-4" />
          ) : (
            <XCircle className="h-4 w-4" />
          )}
          <AlertTitle>Independent client-side verification result</AlertTitle>
          <AlertDescription>
            <ul className="mt-1 list-inside list-disc space-y-1">
              <li>Content hashes recomputed from raw content: {verifyOutcome.allHashesMatch ? "MATCH" : "MISMATCH"}</li>
              <li>Hash chain links (prev_version_hash): {verifyOutcome.chainValid ? "VALID" : "BROKEN"}</li>
              <li>
                Merkle-anchor inclusion proof:{" "}
                {verifyOutcome.anchorStatus === "verified"
                  ? "VERIFIED against IPFS-anchored root"
                  : verifyOutcome.anchorStatus === "failed"
                    ? "FAILED"
                    : "not anchored yet"}
              </li>
              {verifyOutcome.ipfsGatewayUrl && (
                <li>
                  <a
                    href={verifyOutcome.ipfsGatewayUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1 underline"
                  >
                    View anchored batch on IPFS <ExternalLink className="h-3 w-3" />
                  </a>
                </li>
              )}
            </ul>
            {verifyOutcome.details.length > 0 && (
              <ul className="mt-2 list-inside list-disc space-y-1 text-xs opacity-80">
                {verifyOutcome.details.map((d, i) => (
                  <li key={i}>{d}</li>
                ))}
              </ul>
            )}
          </AlertDescription>
        </Alert>
      )}

      <Separator />

      <div>
        <h2 className="text-lg font-semibold">Report suspicious activity</h2>
        <div className="mt-2 flex flex-col gap-2 sm:flex-row">
          <Textarea
            placeholder="What made you suspicious of this listing?"
            value={reportReason}
            onChange={(e) => setReportReason(e.target.value)}
            className="sm:flex-1"
          />
          <Button variant="destructive" onClick={handleReport} disabled={busy} className="sm:self-start">
            Report as scam
          </Button>
        </div>
      </div>

      <Separator />

      <div>
        <h2 className="text-lg font-semibold">Full version history &amp; trust trajectory</h2>
        <p className="text-sm text-muted-foreground">
          Every edit is a new, hash-chained row — nothing here was ever overwritten.
        </p>
        <ol className="mt-4 space-y-4 border-l pl-4">
          {history.history.map((entry) => (
            <li key={entry.version.id} className="relative">
              <div className="absolute -left-[21px] top-1 h-2.5 w-2.5 rounded-full bg-primary" />
              <p className="text-sm font-medium">
                Version {entry.version.version_number} · {new Date(entry.version.created_at).toLocaleString()}
              </p>
              <p className="text-xs text-muted-foreground">
                Author: {entry.version.author_type}
                {entry.version.signature ? " (signed)" : ""}
              </p>
              {entry.changed_fields.length > 0 && (
                <p className="text-xs">
                  Changed:{" "}
                  <span className="font-medium text-amber-700 dark:text-amber-400">
                    {entry.changed_fields.join(", ")}
                  </span>
                </p>
              )}
              <p className="mt-1 font-mono text-[11px] text-muted-foreground break-all">
                hash: {entry.version.content_hash}
              </p>
            </li>
          ))}
        </ol>

        <h3 className="mt-6 text-sm font-semibold">Trust trajectory</h3>
        <ol className="mt-2 space-y-2">
          {history.trust_trajectory.map((snap) => (
            <li key={snap.id} className="flex items-center gap-2 text-sm">
              <TrustBadge status={snap.status} />
              <span className="text-muted-foreground">{new Date(snap.computed_at).toLocaleString()}</span>
              <span className="text-xs text-muted-foreground">— {trustStatusReason(snap.status, snap.inputs_summary)}</span>
            </li>
          ))}
        </ol>
      </div>
    </div>
  );
}

"use client";

import { useEffect, useState, useCallback } from "react";
import { api, ApiError } from "@/lib/api";
import type { SearchResult } from "@/lib/types";
import { ListingCard } from "@/components/listing-card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const CATEGORIES = [
  { value: "all", label: "All categories" },
  { value: "scholarship", label: "Scholarship" },
  { value: "grant", label: "Grant" },
  { value: "subsidy", label: "Subsidy" },
  { value: "job", label: "Job" },
  { value: "training", label: "Training" },
];

const STATUSES = [
  { value: "all", label: "Any trust status" },
  { value: "institutionally_verified", label: "Institutionally Verified" },
  { value: "community_verified", label: "Community Verified" },
  { value: "unverified", label: "Unverified" },
  { value: "flagged", label: "Flagged" },
  { value: "confirmed_scam", label: "Confirmed Scam" },
];

export default function SearchPage() {
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [category, setCategory] = useState("all");
  const [region, setRegion] = useState("");
  const [status, setStatus] = useState("all");

  const runSearch = useCallback(async () => {
    setError(null);
    try {
      const { results } = await api.search({
        category: category === "all" ? undefined : category,
        region: region || undefined,
        status: status === "all" ? undefined : status,
      });
      setResults(results);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not reach the Gaskiya API.");
      setResults([]);
    }
  }, [category, region, status]);

  useEffect(() => {
    runSearch();
  }, [runSearch]);

  return (
    <div className="space-y-6">
      <div className="rounded-xl border bg-gradient-to-br from-primary/10 via-transparent to-transparent p-6">
        <h1 className="text-2xl font-semibold tracking-tight">Find a verified opportunity</h1>
        <p className="mt-1 max-w-2xl text-muted-foreground">
          Every listing here has a tamper-evident edit history. No trust badge is just a claim —
          click through and hit &ldquo;Verify&rdquo; to check it yourself.
        </p>
      </div>

      <div className="flex flex-wrap items-end gap-3">
        <div className="flex flex-col gap-1">
          <label className="text-xs font-medium text-muted-foreground">Category</label>
          <Select value={category} onValueChange={(v) => v && setCategory(v)}>
            <SelectTrigger className="w-44">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {CATEGORIES.map((c) => (
                <SelectItem key={c.value} value={c.value}>
                  {c.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs font-medium text-muted-foreground">Region</label>
          <Input
            placeholder="e.g. Kano, Nigeria"
            value={region}
            onChange={(e) => setRegion(e.target.value)}
            className="w-56"
          />
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs font-medium text-muted-foreground">Trust status</label>
          <Select value={status} onValueChange={(v) => v && setStatus(v)}>
            <SelectTrigger className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {STATUSES.map((s) => (
                <SelectItem key={s.value} value={s.value}>
                  {s.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <Button onClick={runSearch}>Search</Button>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertTitle>Couldn&apos;t load listings</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {results === null && (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-44 w-full" />
          ))}
        </div>
      )}

      {results !== null && results.length === 0 && !error && (
        <p className="text-muted-foreground">No listings match those filters.</p>
      )}

      {results !== null && results.length > 0 && (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {results.map((r) => (
            <ListingCard key={r.version_id} result={r} />
          ))}
        </div>
      )}
    </div>
  );
}

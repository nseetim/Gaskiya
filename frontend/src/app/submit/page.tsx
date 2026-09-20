"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api";
import { dateOnlyToRfc3339 } from "@/lib/canonical";
import { getOrCreateUserId } from "@/lib/identity";
import {
  ListingContentForm,
  EMPTY_LISTING_FORM,
  type ListingFormValues,
} from "@/components/listing-content-form";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Category } from "@/lib/types";

const CATEGORIES: { value: Category; label: string }[] = [
  { value: "scholarship", label: "Scholarship" },
  { value: "grant", label: "Grant" },
  { value: "subsidy", label: "Subsidy" },
  { value: "job", label: "Job" },
  { value: "training", label: "Training" },
];

export default function SubmitPage() {
  const router = useRouter();
  const [values, setValues] = useState<ListingFormValues>(EMPTY_LISTING_FORM);
  const [category, setCategory] = useState<Category>("scholarship");
  const [region, setRegion] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const result = await api.createListing({
        category,
        region,
        content: {
          title: values.title,
          description: values.description,
          eligibility: values.eligibility,
          deadline: dateOnlyToRfc3339(values.deadline),
          application_url: values.application_url,
          contact: values.contact,
        },
        author_type: "user",
        author_id: getOrCreateUserId(),
        signature: null,
      });
      router.push(`/listings/${result.listing_id}`);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not submit this listing.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="max-w-2xl space-y-6">
      <div className="rounded-xl border bg-gradient-to-br from-primary/10 via-transparent to-transparent p-6">
        <h1 className="text-2xl font-semibold tracking-tight">Submit a listing</h1>
        <p className="mt-1 text-muted-foreground">
          Anyone can submit a listing anonymously. It starts as <strong>Unverified</strong> and gains
          trust through independent corroboration — or gets caught by the trust engine if it&apos;s a
          scam. Institutions should use the{" "}
          <a href="/institution" className="text-primary underline">
            Institution Console
          </a>{" "}
          to publish under a verified signature instead.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label>Category</Label>
            <Select value={category} onValueChange={(v) => v && setCategory(v as Category)}>
              <SelectTrigger className="w-full">
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
          <div className="space-y-1.5">
            <Label htmlFor="region">Region</Label>
            <input
              id="region"
              className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={region}
              onChange={(e) => setRegion(e.target.value)}
              placeholder="e.g. Nairobi, Kenya"
              required
            />
          </div>
        </div>

        <ListingContentForm values={values} onChange={setValues} />

        {error && (
          <Alert variant="destructive">
            <AlertTitle>Couldn&apos;t submit</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <Button type="submit" disabled={submitting}>
          {submitting ? "Submitting..." : "Submit listing"}
        </Button>
      </form>
    </div>
  );
}

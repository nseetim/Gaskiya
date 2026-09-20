"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api";
import { dateOnlyToRfc3339, rfc3339ToDateOnly } from "@/lib/canonical";
import { getOrCreateUserId } from "@/lib/identity";
import {
  ListingContentForm,
  EMPTY_LISTING_FORM,
  type ListingFormValues,
} from "@/components/listing-content-form";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";

export default function EditListingPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const [values, setValues] = useState<ListingFormValues | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    api
      .getListing(params.id)
      .then((detail) => {
        const v = detail.current_version;
        setValues({
          title: v.title,
          description: v.description,
          eligibility: v.eligibility,
          deadline: rfc3339ToDateOnly(v.deadline),
          application_url: v.application_url,
          contact: v.contact,
        });
      })
      .catch((e) => setError(e instanceof ApiError ? e.message : "Could not load this listing."));
  }, [params.id]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!values) return;
    setError(null);
    setSubmitting(true);
    try {
      await api.createVersion(params.id, {
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
      router.push(`/listings/${params.id}`);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not save this edit.");
    } finally {
      setSubmitting(false);
    }
  }

  if (error && !values) {
    return (
      <Alert variant="destructive">
        <AlertTitle>Couldn&apos;t load this listing</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    );
  }

  if (!values) {
    return <Skeleton className="h-96 w-full max-w-2xl" />;
  }

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Edit listing</h1>
        <p className="mt-1 text-muted-foreground">
          This creates a new, hash-chained version — the prior version stays visible in the history,
          it&apos;s never overwritten. If this listing had already earned trust and you change the
          application link or contact without touching anything else, the trust engine will flag it.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <ListingContentForm values={values ?? EMPTY_LISTING_FORM} onChange={setValues} />

        {error && (
          <Alert variant="destructive">
            <AlertTitle>Couldn&apos;t save</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <Button type="submit" disabled={submitting}>
          {submitting ? "Saving..." : "Save new version"}
        </Button>
      </form>
    </div>
  );
}

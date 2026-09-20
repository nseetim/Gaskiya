import Link from "next/link";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { TrustBadge, trustStatusReason } from "@/components/trust-badge";
import type { SearchResult } from "@/lib/types";

const CATEGORY_LABEL: Record<string, string> = {
  scholarship: "Scholarship",
  grant: "Grant",
  subsidy: "Subsidy",
  job: "Job",
  training: "Training",
};

export function ListingCard({ result }: { result: SearchResult }) {
  const deadline = new Date(result.deadline);
  const isPast = deadline.getTime() < Date.now();

  return (
    <Link href={`/listings/${result.listing_id}`}>
      <Card className="h-full border-t-2 border-t-primary/70 transition-shadow hover:shadow-md">
        <CardHeader className="flex flex-row items-start justify-between gap-2">
          <div>
            <p className="text-xs font-semibold tracking-wide text-primary uppercase">
              {CATEGORY_LABEL[result.category] ?? result.category}
              <span className="font-normal text-muted-foreground"> · {result.region}</span>
            </p>
            <h3 className="mt-1 font-semibold leading-snug">{result.title}</h3>
          </div>
          <TrustBadge status={result.status} className="shrink-0" />
        </CardHeader>
        <CardContent className="space-y-2">
          <p className="line-clamp-2 text-sm text-muted-foreground">{result.description}</p>
          <p className="text-xs text-muted-foreground">{trustStatusReason(result.status)}</p>
          <p className={`text-xs font-medium ${isPast ? "text-destructive" : "text-foreground"}`}>
            Deadline: {deadline.toLocaleDateString()} {isPast && "(passed)"}
          </p>
          <p className="text-xs text-muted-foreground">
            Last updated {new Date(result.version_created_at).toLocaleDateString()}
          </p>
        </CardContent>
      </Card>
    </Link>
  );
}

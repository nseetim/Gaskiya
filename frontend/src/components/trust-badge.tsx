import { cn } from "@/lib/utils";
import type { TrustStatus } from "@/lib/types";

const STATUS_META: Record<TrustStatus, { label: string; className: string }> = {
  unverified: { label: "Unverified", className: "bg-muted text-muted-foreground border-border" },
  community_verified: {
    label: "Community Verified",
    className: "bg-blue-100 text-blue-800 border-blue-300 dark:bg-blue-950 dark:text-blue-300 dark:border-blue-800",
  },
  institutionally_verified: {
    label: "Institutionally Verified",
    className:
      "bg-green-100 text-green-800 border-green-300 dark:bg-green-950 dark:text-green-300 dark:border-green-800",
  },
  flagged: {
    label: "Flagged",
    className:
      "bg-amber-100 text-amber-900 border-amber-300 dark:bg-amber-950 dark:text-amber-300 dark:border-amber-800",
  },
  confirmed_scam: {
    label: "Confirmed Scam",
    className: "bg-red-100 text-red-800 border-red-300 dark:bg-red-950 dark:text-red-300 dark:border-red-800",
  },
};

export function TrustBadge({ status, className }: { status: TrustStatus; className?: string }) {
  const meta = STATUS_META[status];
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium",
        meta.className,
        className,
      )}
    >
      {meta.label}
    </span>
  );
}

export function trustStatusReason(status: TrustStatus, inputs?: Record<string, unknown>): string {
  switch (status) {
    case "institutionally_verified":
      return "Published under a verified institution's cryptographic signature.";
    case "community_verified":
      return `Corroborated by ${inputs?.corroboration_count ?? "multiple"} independent accounts.`;
    case "flagged": {
      if (inputs?.pivot_detected_at_version_number != null) {
        return `Application link or contact changed in version ${inputs.pivot_detected_at_version_number} after trust was established.`;
      }
      if (typeof inputs?.report_count === "number" && inputs.report_count >= 5) {
        return `Reported by ${inputs.report_count} independent users as suspicious.`;
      }
      if (inputs?.duplicate) {
        return "Matches a previously flagged listing's content.";
      }
      const flags = (inputs?.structural_red_flags as string[]) ?? [];
      if (flags.includes("requests_upfront_payment")) {
        return "Contains language requesting an upfront payment to release funds.";
      }
      return "Flagged by the trust engine — see full rationale below.";
    }
    case "confirmed_scam":
      return "Confirmed as a scam by a moderator.";
    default:
      return "No independent verification yet — proceed with caution and check the details yourself.";
  }
}

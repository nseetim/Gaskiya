import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";

export interface ListingFormValues {
  title: string;
  description: string;
  eligibility: string;
  /** date-only, "YYYY-MM-DD" */
  deadline: string;
  application_url: string;
  contact: string;
}

export const EMPTY_LISTING_FORM: ListingFormValues = {
  title: "",
  description: "",
  eligibility: "",
  deadline: "",
  application_url: "",
  contact: "",
};

export function ListingContentForm({
  values,
  onChange,
}: {
  values: ListingFormValues;
  onChange: (values: ListingFormValues) => void;
}) {
  function set<K extends keyof ListingFormValues>(key: K, value: ListingFormValues[K]) {
    onChange({ ...values, [key]: value });
  }

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <Label htmlFor="title">Title</Label>
        <Input id="title" value={values.title} onChange={(e) => set("title", e.target.value)} required />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="description">Description</Label>
        <Textarea
          id="description"
          value={values.description}
          onChange={(e) => set("description", e.target.value)}
          required
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="eligibility">Eligibility</Label>
        <Textarea
          id="eligibility"
          value={values.eligibility}
          onChange={(e) => set("eligibility", e.target.value)}
          required
        />
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="space-y-1.5">
          <Label htmlFor="deadline">Deadline</Label>
          <Input
            id="deadline"
            type="date"
            value={values.deadline}
            onChange={(e) => set("deadline", e.target.value)}
            required
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="application_url">Application URL</Label>
          <Input
            id="application_url"
            type="url"
            value={values.application_url}
            onChange={(e) => set("application_url", e.target.value)}
            required
          />
        </div>
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="contact">Contact</Label>
        <Input id="contact" value={values.contact} onChange={(e) => set("contact", e.target.value)} required />
      </div>
    </div>
  );
}

"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api";
import { contentHash, dateOnlyToRfc3339 } from "@/lib/canonical";
import { generateKeypair, signHex } from "@/lib/crypto";
import {
  clearInstitutionIdentity,
  getInstitutionIdentity,
  saveInstitutionIdentity,
  type InstitutionIdentity,
} from "@/lib/identity";
import {
  ListingContentForm,
  EMPTY_LISTING_FORM,
  type ListingFormValues,
} from "@/components/listing-content-form";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Category, InstitutionCategory } from "@/lib/types";
import { toast } from "sonner";
import { ShieldCheck } from "lucide-react";

const CATEGORIES: { value: Category; label: string }[] = [
  { value: "scholarship", label: "Scholarship" },
  { value: "grant", label: "Grant" },
  { value: "subsidy", label: "Subsidy" },
  { value: "job", label: "Job" },
  { value: "training", label: "Training" },
];

const INSTITUTION_CATEGORIES: { value: InstitutionCategory; label: string }[] = [
  { value: "govt", label: "Government" },
  { value: "university", label: "University" },
  { value: "ngo", label: "NGO" },
];

export default function InstitutionConsolePage() {
  const router = useRouter();
  const [identity, setIdentity] = useState<InstitutionIdentity | null>(null);
  const [newName, setNewName] = useState("");
  const [newCategory, setNewCategory] = useState<InstitutionCategory>("university");
  const [generating, setGenerating] = useState(false);

  const [category, setCategory] = useState<Category>("scholarship");
  const [region, setRegion] = useState("");
  const [values, setValues] = useState<ListingFormValues>(EMPTY_LISTING_FORM);
  const [editListingId, setEditListingId] = useState("");
  const [publishing, setPublishing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setIdentity(getInstitutionIdentity());
  }, []);

  async function handleGenerateAndRegister() {
    if (!newName.trim()) {
      toast.error("Enter an institution name first.");
      return;
    }
    setGenerating(true);
    try {
      // The private key is generated and stays in this browser — only the
      // public key is ever sent to the server.
      const { privateKeyHex, publicKeyHex } = await generateKeypair();
      const result = await api.createInstitution(newName, newCategory, publicKeyHex);
      const newIdentity: InstitutionIdentity = {
        institutionId: result.institution.id,
        name: newName,
        privateKeyHex,
        publicKeyHex,
      };
      saveInstitutionIdentity(newIdentity);
      setIdentity(newIdentity);
      toast.success(`Registered "${newName}" as a verified institution.`);
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "Could not register this institution.");
    } finally {
      setGenerating(false);
    }
  }

  function handleClearIdentity() {
    clearInstitutionIdentity();
    setIdentity(null);
    toast("Institution identity cleared from this browser.");
  }

  async function handlePublish(e: React.FormEvent) {
    e.preventDefault();
    if (!identity?.institutionId) return;
    setError(null);
    setPublishing(true);
    try {
      const content = {
        title: values.title,
        description: values.description,
        eligibility: values.eligibility,
        deadline: dateOnlyToRfc3339(values.deadline),
        application_url: values.application_url,
        contact: values.contact,
      };
      // Real client-side signing: hash the content exactly as the server
      // will, then sign that hash with the private key held only here.
      const hash = await contentHash(content);
      const signature = await signHex(identity.privateKeyHex, hash);

      if (editListingId.trim()) {
        await api.createVersion(editListingId.trim(), {
          content,
          author_type: "institution",
          author_id: identity.institutionId,
          signature,
        });
        router.push(`/listings/${editListingId.trim()}`);
      } else {
        const result = await api.createListing({
          category,
          region,
          content,
          author_type: "institution",
          author_id: identity.institutionId,
          signature,
        });
        router.push(`/listings/${result.listing_id}`);
      }
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not publish this listing.");
    } finally {
      setPublishing(false);
    }
  }

  return (
    <div className="max-w-2xl space-y-8">
      <div className="rounded-xl border bg-gradient-to-br from-primary/10 via-transparent to-transparent p-6">
        <h1 className="text-2xl font-semibold tracking-tight">Institution Console</h1>
        <p className="mt-1 text-muted-foreground">
          Publish listings under a real Ed25519 signature. The private key is generated in your
          browser and never sent to the server — only the public key is registered.
        </p>
      </div>

      {!identity ? (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Register as a verified institution</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="inst-name">Institution name</Label>
              <Input id="inst-name" value={newName} onChange={(e) => setNewName(e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Category</Label>
              <Select value={newCategory} onValueChange={(v) => v && setNewCategory(v as InstitutionCategory)}>
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {INSTITUTION_CATEGORIES.map((c) => (
                    <SelectItem key={c.value} value={c.value}>
                      {c.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button onClick={handleGenerateAndRegister} disabled={generating}>
              {generating ? "Generating keypair..." : "Generate keypair & register"}
            </Button>
            <p className="text-xs text-muted-foreground">
              This is a demo onboarding flow (spec Section 3): registering here is itself the
              &quot;verification&quot; step, standing in for a manual admin allowlist.
            </p>
          </CardContent>
        </Card>
      ) : (
        <>
          <Alert>
            <ShieldCheck className="h-4 w-4" />
            <AlertTitle>Signed in as {identity.name}</AlertTitle>
            <AlertDescription>
              <p className="font-mono text-xs break-all">Public key: {identity.publicKeyHex}</p>
              <Button variant="link" className="h-auto p-0 text-xs" onClick={handleClearIdentity}>
                Clear this identity from browser
              </Button>
            </AlertDescription>
          </Alert>

          <form onSubmit={handlePublish} className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="edit-id">Existing listing ID to edit (leave blank to publish new)</Label>
              <Input
                id="edit-id"
                value={editListingId}
                onChange={(e) => setEditListingId(e.target.value)}
                placeholder="uuid"
              />
            </div>

            {!editListingId.trim() && (
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
                  <Input id="region" value={region} onChange={(e) => setRegion(e.target.value)} />
                </div>
              </div>
            )}

            <ListingContentForm values={values} onChange={setValues} />

            {error && (
              <Alert variant="destructive">
                <AlertTitle>Couldn&apos;t publish</AlertTitle>
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}

            <Button type="submit" disabled={publishing}>
              {publishing ? "Signing & publishing..." : "Sign & publish"}
            </Button>
          </form>
        </>
      )}
    </div>
  );
}

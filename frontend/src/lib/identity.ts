// Client-generated opaque UUID as a capability token — no login system by
// design (spec Section 4: citizens are "anonymous or lightly registered").
// See DECISIONS.md "Operating constraints" for the privacy rationale.
const USER_ID_KEY = "gaskiya:user_id";
const INSTITUTION_IDENTITY_KEY = "gaskiya:institution_identity";

function randomUuid(): string {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  // Fallback for non-secure-context environments without crypto.randomUUID.
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export function getOrCreateUserId(): string {
  if (typeof window === "undefined") return "";
  let id = window.localStorage.getItem(USER_ID_KEY);
  if (!id) {
    id = randomUuid();
    window.localStorage.setItem(USER_ID_KEY, id);
  }
  return id;
}

export interface InstitutionIdentity {
  institutionId: string | null;
  name: string;
  privateKeyHex: string;
  publicKeyHex: string;
}

export function getInstitutionIdentity(): InstitutionIdentity | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(INSTITUTION_IDENTITY_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as InstitutionIdentity;
  } catch {
    return null;
  }
}

export function saveInstitutionIdentity(identity: InstitutionIdentity) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(INSTITUTION_IDENTITY_KEY, JSON.stringify(identity));
}

export function clearInstitutionIdentity() {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(INSTITUTION_IDENTITY_KEY);
}

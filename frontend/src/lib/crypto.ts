// Real in-browser Ed25519 (spec Section 3/DECISIONS.md: signatures are real
// crypto, not a placeholder). The private key never leaves the browser —
// only the public key is ever sent to the server, at institution
// onboarding. Verified byte-for-byte interoperable with the Rust engine's
// ed25519-dalek implementation (see DECISIONS.md).
import * as ed from "@noble/ed25519";

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.trim();
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export interface Keypair {
  privateKeyHex: string;
  publicKeyHex: string;
}

export async function generateKeypair(): Promise<Keypair> {
  const { secretKey, publicKey } = await ed.keygenAsync();
  return { privateKeyHex: bytesToHex(secretKey), publicKeyHex: bytesToHex(publicKey) };
}

export async function publicKeyFromPrivate(privateKeyHex: string): Promise<string> {
  const publicKey = await ed.getPublicKeyAsync(hexToBytes(privateKeyHex));
  return bytesToHex(publicKey);
}

/** Signs `message` (typically a content_hash hex string, matching the
 * server's `verify_hex(pk, content_hash.as_bytes(), sig)`). */
export async function signHex(privateKeyHex: string, message: string): Promise<string> {
  const signature = await ed.signAsync(new TextEncoder().encode(message), hexToBytes(privateKeyHex));
  return bytesToHex(signature);
}

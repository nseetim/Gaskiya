//! Dev/demo utility: signs a listing content JSON (read from stdin, the
//! same shape sent as `content` in `POST /api/listings`) with an
//! institution's private key, and prints the resulting content_hash +
//! signature to attach to that request. Reused by the seed script.
//!
//! Usage: cargo run -p engine --example sign_content -- <private_key_hex> < content.json

use std::io::Read;

fn main() {
    let private_key_hex = std::env::args()
        .nth(1)
        .expect("usage: sign_content <private_key_hex> < content.json");

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).expect("failed to read stdin");
    let content: serde_json::Value = serde_json::from_str(&input).expect("stdin must be valid JSON");

    let content_hash = engine::merkle_chain::content_hash(&content).expect("hashing cannot fail for a Value");
    let signature = engine::signature::sign_hex(&private_key_hex, content_hash.as_bytes())
        .expect("signing failed: check the private key is valid hex");

    println!(
        "{}",
        serde_json::json!({ "content_hash": content_hash, "signature": signature })
    );
}

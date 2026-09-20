//! Gaskiya DMC-Btree-lite engine: the hand-rolled, standalone technical core.
//!
//! Deliberately has no web-framework or database dependencies (see
//! DECISIONS.md / spec Section 9) so it can be built, tested, and presented
//! as an independent artifact.

pub mod cuckoo;
pub mod merkle_chain;
pub mod merkle_tree;
pub mod signature;

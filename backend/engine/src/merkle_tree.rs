//! Batch Merkle tree for periodic anchoring: a set of `ListingVersion`
//! content hashes are combined into one root per anchor batch (see
//! `MerkleAnchor` in the data model). A client can independently verify
//! that a specific version was included in an anchored batch by
//! recomputing a [`MerkleProof`] against the batch's root hash, without
//! trusting the server.

use crate::merkle_chain::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: String,
    pub leaf_index: usize,
    pub siblings: Vec<(String, Side)>,
}

impl MerkleProof {
    pub fn verify(&self, expected_root: &str) -> bool {
        let mut current = self.leaf_hash.clone();
        for (sibling, side) in &self.siblings {
            current = match side {
                Side::Right => hash_pair(&current, sibling),
                Side::Left => hash_pair(sibling, &current),
            };
        }
        current == expected_root
    }
}

fn hash_pair(left: &str, right: &str) -> String {
    let mut data = Vec::with_capacity(left.len() + right.len());
    data.extend_from_slice(left.as_bytes());
    data.extend_from_slice(right.as_bytes());
    sha256_hex(&data)
}

pub struct MerkleTree {
    /// layers[0] = leaves, layers[last] = [root]
    layers: Vec<Vec<String>>,
}

impl MerkleTree {
    /// Builds a tree over `leaves` (in stable, caller-determined order —
    /// callers should sort by e.g. version id first so the tree is
    /// reproducible). An odd node at any level is promoted by hashing it
    /// with itself, rather than left dangling.
    pub fn build(leaves: Vec<String>) -> Self {
        assert!(!leaves.is_empty(), "cannot build a Merkle tree with no leaves");
        let mut layers = vec![leaves];
        while layers.last().unwrap().len() > 1 {
            let prev = layers.last().unwrap();
            let mut next = Vec::with_capacity(prev.len().div_ceil(2));
            for pair in prev.chunks(2) {
                let combined = if pair.len() == 2 {
                    hash_pair(&pair[0], &pair[1])
                } else {
                    hash_pair(&pair[0], &pair[0])
                };
                next.push(combined);
            }
            layers.push(next);
        }
        MerkleTree { layers }
    }

    pub fn root(&self) -> &str {
        &self.layers.last().unwrap()[0]
    }

    pub fn leaf_count(&self) -> usize {
        self.layers[0].len()
    }

    pub fn proof(&self, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.layers[0].len() {
            return None;
        }
        let mut idx = leaf_index;
        let mut siblings = Vec::new();
        for layer in &self.layers[..self.layers.len() - 1] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let sibling_hash = layer.get(sibling_idx).cloned().unwrap_or_else(|| layer[idx].clone());
            let side = if idx % 2 == 0 { Side::Right } else { Side::Left };
            siblings.push((sibling_hash, side));
            idx /= 2;
        }
        Some(MerkleProof {
            leaf_hash: self.layers[0][leaf_index].clone(),
            leaf_index,
            siblings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(n: usize) -> Vec<String> {
        (0..n).map(|i| sha256_hex(format!("leaf-{i}").as_bytes())).collect()
    }

    #[test]
    fn every_leaf_proves_inclusion_in_power_of_two_tree() {
        let tree = MerkleTree::build(leaves(8));
        for i in 0..8 {
            let proof = tree.proof(i).unwrap();
            assert!(proof.verify(tree.root()));
        }
    }

    #[test]
    fn every_leaf_proves_inclusion_with_odd_leaf_count() {
        let tree = MerkleTree::build(leaves(5));
        for i in 0..5 {
            let proof = tree.proof(i).unwrap();
            assert!(proof.verify(tree.root()));
        }
    }

    #[test]
    fn single_leaf_tree_is_its_own_root() {
        let l = leaves(1);
        let tree = MerkleTree::build(l.clone());
        assert_eq!(tree.root(), l[0]);
        let proof = tree.proof(0).unwrap();
        assert!(proof.verify(tree.root()));
    }

    #[test]
    fn tampered_leaf_fails_verification() {
        let tree = MerkleTree::build(leaves(4));
        let mut proof = tree.proof(2).unwrap();
        proof.leaf_hash = sha256_hex(b"tampered");
        assert!(!proof.verify(tree.root()));
    }
}

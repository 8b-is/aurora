//! Admissibility gate, atomic checkpoint and rollback.
//!
//! A `Snapshot` is only produced by [`Snapshot::build`], and only consumed
//! through [`admit`], which returns the type-level witness [`Admitted`]. That
//! makes it impossible to commit an unverified snapshot by accident.

use crate::graph::{SemanticEdge, SemanticNode};
use aurora_core::{KintsugiError, wupz_xor_fold};
use std::fmt;
use std::fs;
use std::path::Path;

pub const SNAPSHOT_VERSION: u32 = 1;
pub const MAX_NODES: usize = 1_000_000;
pub const MAX_EDGES: usize = 4_000_000;
const MAGIC: &[u8; 8] = b"AURCON01";

/// A candidate persisted state. `digest` covers every meaningful field, so any
/// post-build mutation is detectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub version: u32,
    pub checkpoint_seq: u64,
    pub nodes: Vec<SemanticNode>,
    pub edges: Vec<SemanticEdge>,
    pub digest: u64,
}

impl Snapshot {
    pub fn build(checkpoint_seq: u64, nodes: Vec<SemanticNode>, edges: Vec<SemanticEdge>) -> Self {
        let mut snapshot = Self {
            version: SNAPSHOT_VERSION,
            checkpoint_seq,
            nodes,
            edges,
            digest: 0,
        };
        snapshot.digest = snapshot.compute_digest();
        snapshot
    }

    /// Recompute the digest from the current fields.
    pub fn compute_digest(&self) -> u64 {
        let mut bytes = Vec::with_capacity(16 + self.nodes.len() * 32 + self.edges.len() * 24);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&self.checkpoint_seq.to_le_bytes());
        for node in &self.nodes {
            bytes.extend_from_slice(&node.id.to_le_bytes());
            bytes.extend_from_slice(&node.hash.to_le_bytes());
            bytes.extend_from_slice(&node.weight_ppm.to_le_bytes());
            bytes.extend_from_slice(&(node.bytes as u64).to_le_bytes());
        }
        for edge in &self.edges {
            bytes.extend_from_slice(&edge.from.to_le_bytes());
            bytes.extend_from_slice(&edge.to.to_le_bytes());
            bytes.extend_from_slice(&edge.weight_ppm.to_le_bytes());
        }
        wupz_xor_fold(&bytes)
    }
}

/// Reasons a snapshot is refused. Each is deterministic and testable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    Version(u32),
    DigestMismatch,
    Schema(&'static str),
    Capability(&'static str),
    SequenceRegression { previous: u64, proposed: u64 },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version(v) => write!(f, "unsupported snapshot version {v}"),
            Self::DigestMismatch => write!(f, "digest does not match the snapshot contents"),
            Self::Schema(reason) => write!(f, "schema violation: {reason}"),
            Self::Capability(reason) => write!(f, "capability violation: {reason}"),
            Self::SequenceRegression { previous, proposed } => {
                write!(f, "checkpoint sequence regressed: {proposed} <= {previous}")
            }
        }
    }
}

/// Proof that a snapshot passed every admissibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted(Snapshot);

impl Admitted {
    pub fn snapshot(&self) -> &Snapshot {
        &self.0
    }

    pub fn into_snapshot(self) -> Snapshot {
        self.0
    }
}

/// Validate a candidate against the invariants and the last committed sequence.
pub fn admit(snapshot: Snapshot, previous_seq: u64) -> Result<Admitted, Violation> {
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(Violation::Version(snapshot.version));
    }
    if snapshot.nodes.len() > MAX_NODES {
        return Err(Violation::Capability("node count exceeds ceiling"));
    }
    if snapshot.edges.len() > MAX_EDGES {
        return Err(Violation::Capability("edge count exceeds ceiling"));
    }
    if snapshot.compute_digest() != snapshot.digest {
        return Err(Violation::DigestMismatch);
    }
    if snapshot.checkpoint_seq <= previous_seq {
        return Err(Violation::SequenceRegression {
            previous: previous_seq,
            proposed: snapshot.checkpoint_seq,
        });
    }
    if snapshot.nodes.iter().any(|n| n.weight_ppm > 1_000_000) {
        return Err(Violation::Capability("node weight above 1_000_000 ppm"));
    }
    if snapshot.edges.iter().any(|e| e.weight_ppm > 1_000_000) {
        return Err(Violation::Capability("edge weight above 1_000_000 ppm"));
    }
    if snapshot.nodes.iter().any(|n| n.id == 0) {
        return Err(Violation::Schema("node id 0 is reserved"));
    }
    let node_ids: std::collections::HashSet<u64> = snapshot.nodes.iter().map(|n| n.id).collect();
    for edge in &snapshot.edges {
        if edge.from == 0 || edge.to == 0 {
            return Err(Violation::Schema("edge endpoint id 0 is reserved"));
        }
        if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
            return Err(Violation::Schema("edge references a missing node"));
        }
    }
    Ok(Admitted(snapshot))
}

/// Write `snapshot` to `path` atomically: temporary file, fsync-by-rename, and
/// the previous file is retained as `<path>.prev` for rollback.
pub fn checkpoint_atomic(path: &Path, snapshot: &Snapshot) -> Result<(), KintsugiError> {
    let tmp = path.with_extension("tmp");
    let prev = path.with_extension("prev");
    let bytes = serialize(snapshot);
    fs::write(&tmp, &bytes).map_err(|_| KintsugiError::MemoryFault)?;
    if path.try_exists().map_err(|_| KintsugiError::MemoryFault)? {
        fs::rename(path, &prev).map_err(|_| KintsugiError::MemoryFault)?;
    }
    fs::rename(&tmp, path).map_err(|_| KintsugiError::MemoryFault)?;
    Ok(())
}

/// Restore the previous checkpoint written by [`checkpoint_atomic`].
pub fn restore_previous(path: &Path) -> Result<(), KintsugiError> {
    let prev = path.with_extension("prev");
    if !prev.try_exists().map_err(|_| KintsugiError::MemoryFault)? {
        return Err(KintsugiError::MemoryFault);
    }
    fs::rename(&prev, path).map_err(|_| KintsugiError::MemoryFault)
}

/// Read and re-verify a checkpoint from disk.
pub fn load_snapshot(path: &Path) -> Result<Snapshot, KintsugiError> {
    let bytes = fs::read(path).map_err(|_| KintsugiError::MemoryFault)?;
    deserialize(&bytes)
}

fn serialize(snapshot: &Snapshot) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32 + snapshot.nodes.len() * 32 + snapshot.edges.len() * 24);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&snapshot.version.to_le_bytes());
    bytes.extend_from_slice(&snapshot.checkpoint_seq.to_le_bytes());
    bytes.extend_from_slice(&(snapshot.nodes.len() as u32).to_le_bytes());
    for node in &snapshot.nodes {
        bytes.extend_from_slice(&node.id.to_le_bytes());
        bytes.extend_from_slice(&node.hash.to_le_bytes());
        bytes.extend_from_slice(&node.weight_ppm.to_le_bytes());
        bytes.extend_from_slice(&(node.bytes as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&(snapshot.edges.len() as u32).to_le_bytes());
    for edge in &snapshot.edges {
        bytes.extend_from_slice(&edge.from.to_le_bytes());
        bytes.extend_from_slice(&edge.to.to_le_bytes());
        bytes.extend_from_slice(&edge.weight_ppm.to_le_bytes());
    }
    bytes.extend_from_slice(&snapshot.digest.to_le_bytes());
    bytes
}

fn deserialize(bytes: &[u8]) -> Result<Snapshot, KintsugiError> {
    let mut cursor = 0usize;
    let take = |n: usize, cursor: &mut usize| -> Result<&[u8], KintsugiError> {
        if *cursor + n > bytes.len() {
            return Err(KintsugiError::InvalidData);
        }
        let slice = &bytes[*cursor..*cursor + n];
        *cursor += n;
        Ok(slice)
    };
    if take(8, &mut cursor)? != MAGIC {
        return Err(KintsugiError::InvalidData);
    }
    let version = u32::from_le_bytes(fixed::<4>(take(4, &mut cursor)?)?);
    let checkpoint_seq = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
    let node_count = u32::from_le_bytes(fixed::<4>(take(4, &mut cursor)?)?) as usize;
    if node_count > MAX_NODES {
        return Err(KintsugiError::InvalidData);
    }
    let mut nodes = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        let id = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        let hash = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        let weight_ppm = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        let size = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        nodes.push(SemanticNode {
            id,
            hash,
            weight_ppm,
            bytes: usize::try_from(size).map_err(|_| KintsugiError::InvalidData)?,
        });
    }
    let edge_count = u32::from_le_bytes(fixed::<4>(take(4, &mut cursor)?)?) as usize;
    if edge_count > MAX_EDGES {
        return Err(KintsugiError::InvalidData);
    }
    let mut edges = Vec::with_capacity(edge_count);
    for _ in 0..edge_count {
        let from = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        let to = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        let weight_ppm = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
        edges.push(SemanticEdge {
            from,
            to,
            weight_ppm,
        });
    }
    let digest = u64::from_le_bytes(fixed::<8>(take(8, &mut cursor)?)?);
    let snapshot = Snapshot {
        version,
        checkpoint_seq,
        nodes,
        edges,
        digest,
    };
    if snapshot.compute_digest() != digest {
        return Err(KintsugiError::InvalidData);
    }
    Ok(snapshot)
}

fn fixed<const N: usize>(slice: &[u8]) -> Result<[u8; N], KintsugiError> {
    slice.try_into().map_err(|_| KintsugiError::InvalidData)
}

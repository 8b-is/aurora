//! State graph: semantic nodes, weighted edges, deduplication and pruning.
//!
//! All weights are integer parts-per-million. Renormalisation distributes the
//! rounding remainder so the edge weights always sum to exactly 1_000_000,
//! which keeps downstream accounting exact.
//!
//! `prune_with` works on a local copy and assigns the result only on success,
//! so an aborted pass can never leave the live graph half-rewritten.

/// A semantic state node. `hash` is the identity used for deduplication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticNode {
    pub id: u64,
    pub hash: u64,
    pub weight_ppm: u64,
    pub bytes: usize,
}

/// A directed, weighted transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticEdge {
    pub from: u64,
    pub to: u64,
    pub weight_ppm: u64,
}

/// Local state graph.
#[derive(Debug, Default, Clone)]
pub struct StateGraph {
    nodes: Vec<SemanticNode>,
    edges: Vec<SemanticEdge>,
}

impl StateGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn nodes(&self) -> &[SemanticNode] {
        &self.nodes
    }

    pub fn edges(&self) -> &[SemanticEdge] {
        &self.edges
    }

    pub fn add_node(&mut self, node: SemanticNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: SemanticEdge) {
        self.edges.push(edge);
    }

    pub fn node(&self, id: u64) -> Option<&SemanticNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Approximate footprint in bytes.
    pub fn bytes(&self) -> usize {
        self.nodes
            .iter()
            .map(|n| n.bytes.saturating_add(32))
            .sum::<usize>()
            .saturating_add(self.edges.len().saturating_mul(24))
    }

    /// Deduplicate nodes by semantic hash, drop orphan nodes, drop self-loops,
    /// merge parallel edges and renormalise the surviving weights.
    ///
    /// Returns `(merged, pruned)`.
    pub fn prune(&mut self) -> (usize, usize) {
        self.prune_with(&|| false).unwrap_or((0, 0))
    }

    /// Interruptible prune. `abort` is polled between phases; when it returns
    /// true the method returns `None` and the live graph is untouched.
    pub fn prune_with(&mut self, abort: &dyn Fn() -> bool) -> Option<(usize, usize)> {
        let mut nodes = self.nodes.clone();
        let mut edges = self.edges.clone();

        // 1. Deduplicate by (hash, id); the first id in each hash group is canonical.
        if abort() {
            return None;
        }
        nodes.sort_by_key(|n| (n.hash, n.id));
        let mut canonical: Vec<SemanticNode> = Vec::with_capacity(nodes.len());
        let mut remap: Vec<(u64, u64)> = Vec::with_capacity(nodes.len());
        for node in nodes {
            match canonical.last_mut() {
                Some(head) if head.hash == node.hash => {
                    head.weight_ppm = head
                        .weight_ppm
                        .saturating_add(node.weight_ppm)
                        .min(1_000_000);
                    head.bytes = head.bytes.saturating_add(node.bytes);
                    remap.push((node.id, head.id));
                }
                _ => {
                    let id = node.id;
                    canonical.push(node);
                    remap.push((id, id));
                }
            }
        }
        let merged = remap.iter().filter(|(old, new)| old != new).count();

        // 2. Rewrite edges through the remap, dropping self-loops.
        if abort() {
            return None;
        }
        remap.sort_by_key(|(old, _)| *old);
        let resolve = |id: u64| -> u64 {
            match remap.binary_search_by_key(&id, |(old, _)| *old) {
                Ok(index) => remap[index].1,
                Err(_) => id,
            }
        };
        let mut rewritten: Vec<SemanticEdge> = Vec::with_capacity(edges.len());
        for edge in edges.drain(..) {
            let from = resolve(edge.from);
            let to = resolve(edge.to);
            if from != to {
                rewritten.push(SemanticEdge {
                    from,
                    to,
                    weight_ppm: edge.weight_ppm,
                });
            }
        }

        // 3. Merge parallel edges, keeping the strongest weight.
        if abort() {
            return None;
        }
        rewritten.sort_by_key(|e| (e.from, e.to));
        let mut merged_edges: Vec<SemanticEdge> = Vec::with_capacity(rewritten.len());
        for edge in rewritten {
            match merged_edges.last_mut() {
                Some(head) if head.from == edge.from && head.to == edge.to => {
                    if edge.weight_ppm > head.weight_ppm {
                        head.weight_ppm = edge.weight_ppm;
                    }
                }
                _ => merged_edges.push(edge),
            }
        }

        // 4. Prune orphans: nodes with neither an outgoing nor an incoming edge.
        if abort() {
            return None;
        }
        let mut referenced = vec![false; canonical.len()];
        let index_of = |id: u64| canonical.iter().position(|n| n.id == id);
        for edge in &merged_edges {
            if let Some(i) = index_of(edge.from) {
                referenced[i] = true;
            }
            if let Some(i) = index_of(edge.to) {
                referenced[i] = true;
            }
        }
        let mut pruned = 0usize;
        let mut survivors: Vec<SemanticNode> = Vec::with_capacity(canonical.len());
        for (i, node) in canonical.into_iter().enumerate() {
            if referenced[i] {
                survivors.push(node);
            } else {
                pruned += 1;
            }
        }

        if abort() {
            return None;
        }
        renormalize(&mut merged_edges);
        self.nodes = survivors;
        self.edges = merged_edges;
        Some((merged, pruned))
    }
}

/// Scale edge weights so they sum to exactly 1_000_000 ppm.
fn renormalize(edges: &mut [SemanticEdge]) {
    let total: u128 = edges.iter().map(|e| u128::from(e.weight_ppm)).sum();
    if total == 0 {
        for edge in edges.iter_mut() {
            edge.weight_ppm = 0;
        }
        return;
    }
    let mut assigned: u64 = 0;
    for edge in edges.iter_mut() {
        let scaled = (u128::from(edge.weight_ppm) * 1_000_000u128 / total) as u64;
        edge.weight_ppm = scaled;
        assigned = assigned.saturating_add(scaled);
    }
    let mut remainder = 1_000_000u64.saturating_sub(assigned);
    for edge in edges.iter_mut() {
        if remainder == 0 {
            break;
        }
        edge.weight_ppm = edge.weight_ppm.saturating_add(1);
        remainder -= 1;
    }
}

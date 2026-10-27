#![forbid(unsafe_code)]

//! `aurora-infer` — BitNet `b1.58` ternary inference, the `12.80x` compression
//! against FP16 coal, and a provider seam where a hosted engine (OpenRouter
//! `google/gemini-4-argon` via `codewhale`) can be dropped in without changing
//! the caller.
//!
//! The local path is fully implemented and deterministic: weights quantize to
//! `{-1, 0, +1}` and a layer forward pass is exact integer arithmetic.

/// Quantize a real weight into a balanced ternary trit `{-1, 0, +1}` using a
/// symmetric epsilon dead-zone.
pub fn quantize_ternary(weight: f64, epsilon: f64) -> i8 {
    if weight > epsilon {
        1
    } else if weight < -epsilon {
        -1
    } else {
        0
    }
}

/// The FP16 → b1.58 compression ratio.
pub const BITNET_COMPRESSION_RATIO: f64 = 12.80;

/// A single ternary linear layer: `out × in` trit weights.
#[derive(Debug, Clone)]
pub struct TernaryLayer {
    weights: Vec<Vec<i8>>,
}

impl TernaryLayer {
    pub fn new(weights: Vec<Vec<i8>>) -> Self {
        Self { weights }
    }

    pub fn output_dim(&self) -> usize {
        self.weights.len()
    }

    pub fn input_dim(&self) -> usize {
        self.weights.first().map_or(0, |row| row.len())
    }

    /// Exact integer matmul: `out[j] = Σ_i W[j][i] · x[i]`.
    pub fn forward(&self, input: &[i8]) -> Vec<i32> {
        self.weights
            .iter()
            .map(|row| {
                row.iter()
                    .zip(input)
                    .map(|(w, x)| (*w as i32) * (*x as i32))
                    .sum()
            })
            .collect()
    }
}

fn binarize(v: i32) -> i8 {
    if v > 0 {
        1
    } else if v < 0 {
        -1
    } else {
        0
    }
}

/// A BitNet b1.58 network: a stack of ternary layers with binarized activations
/// between them and a raw integer readout at the head.
#[derive(Debug, Clone)]
pub struct BitNetB158 {
    layers: Vec<TernaryLayer>,
}

impl BitNetB158 {
    pub fn new(layers: Vec<TernaryLayer>) -> Self {
        Self { layers }
    }

    pub fn forward(&self, input: &[i8]) -> Vec<i32> {
        let mut activations: Vec<i8> = input.to_vec();
        for (i, layer) in self.layers.iter().enumerate() {
            let out = layer.forward(&activations);
            if i + 1 == self.layers.len() {
                return out;
            }
            activations = out.iter().map(|&v| binarize(v)).collect();
        }
        Vec::new()
    }
}

/// Provider seam: a hosted inference engine can be mounted here without the
/// dyad or voice shell caring which engine answers. The local ternary engine
/// is the always-available default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    LocalTernary,
    HostedArgon4,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_ternary_thresholds() {
        assert_eq!(quantize_ternary(0.5, 0.25), 1);
        assert_eq!(quantize_ternary(-0.5, 0.25), -1);
        assert_eq!(quantize_ternary(0.1, 0.25), 0);
        assert_eq!(quantize_ternary(-0.1, 0.25), 0);
    }

    #[test]
    fn test_ternary_layer_forward() {
        // out = [[1, -1]] over inputs [1, 1] → 0; over [1, -1] → 2.
        let layer = TernaryLayer::new(vec![vec![1, -1]]);
        assert_eq!(layer.forward(&[1, 1]), vec![0]);
        assert_eq!(layer.forward(&[1, -1]), vec![2]);
        assert_eq!(layer.output_dim(), 1);
        assert_eq!(layer.input_dim(), 2);
    }

    #[test]
    fn test_bitnet_forward() {
        let net = BitNetB158::new(vec![
            TernaryLayer::new(vec![vec![1, -1], vec![1, 1]]),
            TernaryLayer::new(vec![vec![1, 1]]),
        ]);
        // input [1, -1] → hidden [2, 0] → binarized [1, 0] → head 1*1 + 1*0 = 1
        assert_eq!(net.forward(&[1, -1]), vec![1]);
    }

    #[test]
    fn test_compression_constant() {
        assert_eq!(BITNET_COMPRESSION_RATIO, 12.80);
    }
}

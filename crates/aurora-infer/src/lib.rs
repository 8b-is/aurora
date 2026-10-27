#![forbid(unsafe_code)]
//! Integer ternary primitives. These are not a pretrained language model.
//! Real local language-model inference is provided by the Go loopback adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceError {
    InvalidTrit,
    InvalidShape,
    Overflow,
}
pub fn quantize_ternary(weight: i64, epsilon: u64) -> i8 {
    if weight.unsigned_abs() <= epsilon {
        0
    } else if weight < 0 {
        -1
    } else {
        1
    }
}
/// Pack five balanced trits per byte, base three. No theoretical compression claim.
pub fn pack_trits(trits: &[i8]) -> Result<Vec<u8>, InferenceError> {
    let mut bytes = Vec::new();
    for chunk in trits.chunks(5) {
        let (mut byte, mut power) = (0u16, 1u16);
        for trit in chunk {
            if !(-1..=1).contains(trit) {
                return Err(InferenceError::InvalidTrit);
            }
            byte += (*trit + 1) as u16 * power;
            power *= 3;
        }
        bytes.push(byte as u8);
    }
    Ok(bytes)
}
pub fn unpack_trits(bytes: &[u8], count: usize) -> Result<Vec<i8>, InferenceError> {
    if count.div_ceil(5) != bytes.len() {
        return Err(InferenceError::InvalidShape);
    }
    let mut out = Vec::with_capacity(count);
    for byte in bytes {
        let mut n = *byte;
        for _ in 0..5.min(count - out.len()) {
            out.push((n % 3) as i8 - 1);
            n /= 3;
        }
        if n != 0 {
            return Err(InferenceError::InvalidTrit);
        }
    }
    Ok(out)
}
#[derive(Debug, Clone)]
pub struct TernaryLayer {
    weights: Vec<Vec<i8>>,
    width: usize,
}
impl TernaryLayer {
    pub fn new(weights: Vec<Vec<i8>>) -> Result<Self, InferenceError> {
        let width = weights.first().map_or(0, Vec::len);
        if width == 0 || weights.iter().any(|r| r.len() != width) {
            return Err(InferenceError::InvalidShape);
        }
        if weights.iter().flatten().any(|w| !(-1..=1).contains(w)) {
            return Err(InferenceError::InvalidTrit);
        }
        Ok(Self { weights, width })
    }
    pub fn output_dim(&self) -> usize {
        self.weights.len()
    }
    pub fn input_dim(&self) -> usize {
        self.width
    }
    pub fn forward(&self, input: &[i8]) -> Result<Vec<i64>, InferenceError> {
        if input.len() != self.width {
            return Err(InferenceError::InvalidShape);
        }
        self.weights
            .iter()
            .map(|row| {
                row.iter().zip(input).try_fold(0i64, |sum, (w, x)| {
                    sum.checked_add(i64::from(*w) * i64::from(*x))
                        .ok_or(InferenceError::Overflow)
                })
            })
            .collect()
    }
}
#[derive(Debug, Clone)]
pub struct BitNetB158 {
    layers: Vec<TernaryLayer>,
}
impl BitNetB158 {
    pub fn new(layers: Vec<TernaryLayer>) -> Result<Self, InferenceError> {
        if layers.is_empty()
            || layers
                .windows(2)
                .any(|w| w[0].output_dim() != w[1].input_dim())
        {
            return Err(InferenceError::InvalidShape);
        }
        Ok(Self { layers })
    }
    pub fn forward(&self, input: &[i8]) -> Result<Vec<i64>, InferenceError> {
        let mut act = input.to_vec();
        for (i, layer) in self.layers.iter().enumerate() {
            let out = layer.forward(&act)?;
            if i + 1 == self.layers.len() {
                return Ok(out);
            }
            act = out.iter().map(|v| v.signum() as i8).collect();
        }
        Err(InferenceError::InvalidShape)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_quantization_and_shapes() {
        assert_eq!(quantize_ternary(i64::MIN, 25), -1);
        assert_eq!(quantize_ternary(25, 25), 0);
        assert!(TernaryLayer::new(vec![vec![1], vec![1, 0]]).is_err());
        let net = BitNetB158::new(vec![
            TernaryLayer::new(vec![vec![1, -1], vec![1, 1]]).unwrap(),
            TernaryLayer::new(vec![vec![1, 1]]).unwrap(),
        ])
        .unwrap();
        assert_eq!(net.forward(&[1, -1]).unwrap(), vec![1]);
        assert_eq!(net.forward(&[1]), Err(InferenceError::InvalidShape));
    }
    #[test]
    fn packed_roundtrip_all_five_trit_words() {
        for byte in 0..243u8 {
            let t = unpack_trits(&[byte], 5).unwrap();
            assert_eq!(pack_trits(&t).unwrap(), vec![byte]);
        }
        for count in 0..17 {
            let t = vec![-1; count];
            assert_eq!(unpack_trits(&pack_trits(&t).unwrap(), count).unwrap(), t);
        }
        assert!(unpack_trits(&[255], 5).is_err());
    }
}

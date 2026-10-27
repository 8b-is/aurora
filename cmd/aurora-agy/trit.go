package main

// Balanced ternary spine (BitNet b1.58 {-1, 0, +1}) and the Dawn Aurora
// palette mapping — the Go mirror of `crates/aurora-core`.

// Trit is one balanced ternary qutrit.
type Trit int8

const (
	// TritNeg is -1: Dawn rose, entropy released (the -1 gate).
	TritNeg Trit = -1
	// TritZero is 0: Chenrezig cyan, stillness (the 0 gate).
	TritZero Trit = 0
	// TritPos is +1: Kintsugi gold, resonance (the +1 gate).
	TritPos Trit = 1
)

func (t Trit) String() string {
	switch t {
	case TritNeg:
		return "-"
	case TritZero:
		return "0"
	default:
		return "+"
	}
}

// color returns the Dawn Aurora RGB bound to this trit.
func (t Trit) color() (uint8, uint8, uint8) {
	switch t {
	case TritNeg:
		return 0xff, 0x6b, 0x8b // #ff6b8b dawn rose
	case TritZero:
		return 0x4f, 0xd1, 0xc5 // #4fd1c5 chenrezig cyan
	default:
		return 0xe8, 0xb0, 0x5c // #e8b05c kintsugi gold
	}
}

// quantizeTrit collapses a real weight into a balanced ternary trit with a
// symmetric epsilon dead-zone.
func quantizeTrit(w, epsilon float64) Trit {
	switch {
	case w > epsilon:
		return TritPos
	case w < -epsilon:
		return TritNeg
	default:
		return TritZero
	}
}

// binarize collapses a raw activation into the nearest trit.
func binarize(v int32) Trit {
	switch {
	case v > 0:
		return TritPos
	case v < 0:
		return TritNeg
	default:
		return TritZero
	}
}

// TernaryLayer is one out×in trit weight matrix.
type TernaryLayer struct {
	weights [][]Trit
}

func newTernaryLayer(weights [][]Trit) TernaryLayer {
	return TernaryLayer{weights: weights}
}

// forward performs exact integer matmul: out[j] = Σᵢ W[j][i] · x[i].
func (l TernaryLayer) forward(in []Trit) []int32 {
	out := make([]int32, len(l.weights))
	for i, row := range l.weights {
		var sum int32
		for j, w := range row {
			sum += int32(w) * int32(in[j])
		}
		out[i] = sum
	}
	return out
}

// BitNetB158 is a stack of ternary layers with binarized activations between
// them and a raw integer readout at the head.
type BitNetB158 struct {
	layers []TernaryLayer
}

func newBitNetB158(layers []TernaryLayer) BitNetB158 {
	return BitNetB158{layers: layers}
}

// forward runs the network and returns the head logits.
func (n BitNetB158) forward(input []Trit) []int32 {
	act := append([]Trit(nil), input...)
	for i, layer := range n.layers {
		out := layer.forward(act)
		if i+1 == len(n.layers) {
			return out
		}
		act = make([]Trit, len(out))
		for k, v := range out {
			act[k] = binarize(v)
		}
	}
	return nil
}

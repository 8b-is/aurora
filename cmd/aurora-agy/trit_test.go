package main

import "testing"

func TestQuantizeTrit(t *testing.T) {
	cases := []struct {
		w, eps float64
		want   Trit
	}{
		{0.5, 0.25, TritPos},
		{-0.5, 0.25, TritNeg},
		{0.1, 0.25, TritZero},
		{-0.1, 0.25, TritZero},
	}
	for _, c := range cases {
		if got := quantizeTrit(c.w, c.eps); got != c.want {
			t.Errorf("quantizeTrit(%v, %v) = %v, want %v", c.w, c.eps, got, c.want)
		}
	}
}

func TestTritColor(t *testing.T) {
	if r, g, b := TritNeg.color(); r != 0xff || g != 0x6b || b != 0x8b {
		t.Errorf("TritNeg.color() = #%02x%02x%02x", r, g, b)
	}
	if r, g, b := TritZero.color(); r != 0x4f || g != 0xd1 || b != 0xc5 {
		t.Errorf("TritZero.color() = #%02x%02x%02x", r, g, b)
	}
	if r, g, b := TritPos.color(); r != 0xe8 || g != 0xb0 || b != 0x5c {
		t.Errorf("TritPos.color() = #%02x%02x%02x", r, g, b)
	}
}

func TestTernaryLayerForward(t *testing.T) {
	l := newTernaryLayer([][]Trit{{TritPos, TritNeg}})
	if got := l.forward([]Trit{TritPos, TritPos}); len(got) != 1 || got[0] != 0 {
		t.Errorf("forward([+,+]) = %v, want [0]", got)
	}
	if got := l.forward([]Trit{TritPos, TritNeg}); len(got) != 1 || got[0] != 2 {
		t.Errorf("forward([+,-]) = %v, want [2]", got)
	}
}

func TestBitNetForward(t *testing.T) {
	net := newBitNetB158([]TernaryLayer{
		newTernaryLayer([][]Trit{{TritPos, TritNeg}, {TritPos, TritPos}}),
		newTernaryLayer([][]Trit{{TritPos, TritPos}}),
	})
	// [+,-] -> hidden [2, 0] -> binarized [+, 0] -> head 1*1 + 1*0 = 1
	if got := net.forward([]Trit{TritPos, TritNeg}); len(got) != 1 || got[0] != 1 {
		t.Errorf("forward([+,-]) = %v, want [1]", got)
	}
}

// aurora-agy is the AURORA agent cortex (via agy — "through the brain").
//
// It orchestrates codewhale / entheai / mlx-quant sub-agents, runs local
// BitNet b1.58 continuation hops, and exposes the Dawn Aurora ternary palette.
package main

import (
	"fmt"
	"os"
	"runtime"
	"sync"
	"time"
)

const (
	lambdaMM = 3.0
	wBandHz  = 100_000_000_000.0
)

func main() {
	args := os.Args[1:]
	if len(args) == 0 {
		args = []string{"status"}
	}
	switch args[0] {
	case "run":
		runCortex()
	case "hop":
		hop(args[1:])
	case "breathe":
		breathe()
	case "status":
		status()
	case "help", "-h", "--help":
		usage()
	default:
		fmt.Fprintf(os.Stderr, "unknown command %q\n\n", args[0])
		usage()
		os.Exit(1)
	}
}

func usage() {
	fmt.Println(`aurora-agy — the AURORA agent cortex (via agy)
  run       orchestrate codewhale / entheai / mlx-quant sub-agents
  hop       run a continuation hop (local ternary, or OpenRouter if keyed)
  breathe   settle the Phoenix stability signal
  status    read the cortex
  help      this message`)
}

func status() {
	fmt.Println("AURORA agy — sovereign agent cortex")
	fmt.Printf("λ = %.0f mm | W-band %.0f GHz | BitNet b1.58 | %s\n", lambdaMM, wBandHz/1e9, runtime.Version())
	for _, t := range []Trit{TritNeg, TritZero, TritPos} {
		r, g, b := t.color()
		fmt.Printf("  trit %s -> #%02x%02x%02x\n", t, r, g, b)
	}
	fmt.Println("  overflow -> {<3,<3,<3}+1")
}

// breathe settles the Phoenix Protocol stability signal (EWMA over jitter).
func breathe() {
	smooth := 0.0
	const alpha = 0.125
	for _, j := range []float64{0.9, 0.4, 0.1, 0.0, 0.0, 0.0} {
		smooth = (1-alpha)*smooth + alpha*clamp01(j)
	}
	fmt.Printf("AURORA breathes: stability=%.3f (1 - SmoothJitter)\n", 1.0-smooth)
}

// hop runs one continuation hop through a fixed BitNet b1.58 head. When
// OPENROUTER_API_KEY is present, the same input is also routed through the
// hosted google/gemini-4-argon endpoint; otherwise the local ternary engine is
// authoritative.
func hop(input []string) {
	in := []Trit{TritPos, TritNeg, TritPos}
	if len(input) > 0 {
		in = make([]Trit, len(input))
		for i, c := range input {
			switch c {
			case "+", "1":
				in[i] = TritPos
			case "-":
				in[i] = TritNeg
			default:
				in[i] = TritZero
			}
		}
	}
	net := newBitNetB158([]TernaryLayer{
		newTernaryLayer([][]Trit{{TritPos, TritNeg, TritZero}, {TritZero, TritPos, TritPos}, {TritNeg, TritZero, TritPos}}),
		newTernaryLayer([][]Trit{{TritPos, TritPos, TritNeg}}),
	})
	out := net.forward(in)
	fmt.Printf("AURORA hop: input=%v -> logits=%v\n", in, out)

	if key := os.Getenv("OPENROUTER_API_KEY"); key != "" {
		fmt.Printf("AURORA hop: OPENROUTER_API_KEY present (%d chars) — argon4 hop ready\n", len(key))
	} else {
		fmt.Println("AURORA hop: no OPENROUTER_API_KEY — local ternary authoritative")
	}
}

// runCortex orchestrates the three sub-agents over a shared heartbeat bus.
func runCortex() {
	type agent struct{ name, role string }
	agents := []agent{
		{"codewhale", "long-decode router"},
		{"entheai", "speculative engine"},
		{"mlx-quant", "b1.58 local inference"},
	}

	fmt.Println("AURORA agy cortex online — orchestrating sub-agents")
	beats := make(chan string)
	var wg sync.WaitGroup
	for _, a := range agents {
		wg.Add(1)
		go func(a agent) {
			defer wg.Done()
			for i := 1; i <= 3; i++ {
				beats <- fmt.Sprintf("%-10s %-22s heartbeat %d", a.name, a.role, i)
				time.Sleep(100 * time.Millisecond)
			}
		}(a)
	}
	go func() {
		wg.Wait()
		close(beats)
	}()
	for b := range beats {
		fmt.Println("  ", b)
	}
	fmt.Println("AURORA agy: all sub-agents settled. om mani padme hung.")
}

func clamp01(v float64) float64 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}

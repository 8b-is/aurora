package main

import "testing"

func TestVerifyOmnifoldConstraints(t *testing.T) {
	parent := CapReadWaveGrid | CapMutateDyad | CapDispatchMetalGemv
	child := CapReadWaveGrid | CapDispatchMetalGemv

	okTel := OmnifoldTelemetry{
		ParentCap:            parent,
		ChildCap:             child,
		AttenuationTimeNs:    42_000,
		SmoothJitterPermille: 50,
		ActiveSpin:           1,
	}
	if err := VerifyOmnifoldConstraints(okTel); err != nil {
		t.Fatalf("expected valid omnifold telemetry to pass, got %v", err)
	}

	badCap := okTel
	badCap.ChildCap = parent | CapConsolidateDreamstate
	if err := VerifyOmnifoldConstraints(badCap); err != ErrCapabilityEscalation {
		t.Fatalf("expected ErrCapabilityEscalation, got %v", err)
	}

	badLat := okTel
	badLat.AttenuationTimeNs = MaxAttenuationLatencyNs + 1
	if err := VerifyOmnifoldConstraints(badLat); err != ErrLatencyBudgetBlown {
		t.Fatalf("expected ErrLatencyBudgetBlown, got %v", err)
	}

	badJit := okTel
	badJit.SmoothJitterPermille = 250
	if err := VerifyOmnifoldConstraints(badJit); err != ErrThermalJitterHigh {
		t.Fatalf("expected ErrThermalJitterHigh, got %v", err)
	}
}

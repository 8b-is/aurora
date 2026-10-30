package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Real Cargo fixture: root tests succeed even after an excluded sibling loses a
// field still used by its own code. No network or third-party crates required.
func TestRustValidationScope(t *testing.T) {
	if _, err := exec.LookPath("cargo"); err != nil {
		t.Skip("Cargo required for real workspace regression")
	}
	tree := t.TempDir()
	put := func(path, body string) {
		t.Helper()
		full := filepath.Join(tree, path)
		if err := os.MkdirAll(filepath.Dir(full), 0700); err != nil {
			t.Fatal(err)
		}
		writeTest(t, full, body, 0600)
	}
	put("Cargo.toml", "[workspace]\nmembers = [\"active\"]\nexclude = [\"retired\"]\nresolver = \"2\"\n")
	put("active/Cargo.toml", "[package]\nname = \"active\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
	put("active/src/lib.rs", "pub fn value() -> u32 { 1 }\n")
	put("retired/Cargo.toml", "[package]\nname = \"retired\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
	old := "pub struct ResourceSample { pub cpu_pct: u32 }\npub fn value(s: ResourceSample) -> u32 { s.cpu_pct }\n#[cfg(test)] mod tests { #[test] fn sample() { let _ = super::ResourceSample { cpu_pct: 1 }; } }\n"
	put("retired/src/lib.rs", old)
	put("orphan.rs", "pub fn orphan() {}\n")
	j := job{dir: t.TempDir()}
	if _, err := j.rustOwner(tree, "active/src/lib.rs"); err != nil {
		t.Fatal(err)
	}
	for _, source := range []string{"retired/src/lib.rs", "orphan.rs"} {
		if _, err := j.rustOwner(tree, source); err == nil {
			t.Fatalf("accepted uncovered %s", source)
		}
	}
	if err := j.validateRustCandidate(tree, "active/src/lib.rs", "candidate"); err != nil {
		t.Fatal(err)
	}
	if len(j.receipt.RustChecks) != 1 || !j.receipt.RustChecks[0].Passed || j.receipt.RustChecks[0].Manifest != "active/Cargo.toml" || j.receipt.RustChecks[0].Package != "active" {
		t.Fatalf("missing package evidence: %+v", j.receipt)
	}
	raw, err := json.Marshal(j.receipt)
	if err != nil || !strings.Contains(string(raw), `"manifest":"active/Cargo.toml"`) {
		t.Fatalf("receipt: %s %v", raw, err)
	}
	j.repo.StandaloneRustManifests = []string{"retired/Cargo.toml"}
	if err := j.validateRustCandidate(tree, "retired/src/lib.rs", "baseline"); err != nil {
		t.Fatal(err)
	}
	p := proposal{Path: "retired/src/lib.rs", Old: "pub cpu_pct: u32", New: "", Reason: "fixture field deletion"}
	if err := applyProposal(tree, p, map[string]string{p.Path: old}); err != nil {
		t.Fatal(err)
	}
	if _, err := j.command(tree, 300, "", "cargo", "test", "--workspace", "--offline"); err != nil {
		t.Fatalf("root workspace should still pass: %v", err)
	}
	j.repo.StandaloneRustManifests = nil
	if err := j.validateRustCandidate(tree, p.Path, "candidate"); err == nil {
		t.Fatal("root success validated excluded field deletion")
	}
	j.repo.StandaloneRustManifests = []string{"retired/Cargo.toml"}
	if err := j.validateRustCandidate(tree, p.Path, "candidate"); err == nil {
		t.Fatal("standalone plan accepted broken package")
	}
	last := j.receipt.RustChecks[len(j.receipt.RustChecks)-1]
	if last.Passed || last.Package != "retired" || last.Phase != "candidate" {
		t.Fatalf("failed package check misreported: %+v", last)
	}
	put("retired/Cargo.toml", "invalid manifest")
	if _, err := j.rustOwner(tree, p.Path); err == nil {
		t.Fatal("malformed owner fell back to root")
	}
}

func TestSafeStandaloneManifest(t *testing.T) {
	for _, path := range []string{"../Cargo.toml", "/Cargo.toml", "a/../Cargo.toml", "a\\Cargo.toml", "a/not.toml"} {
		if safeManifest(path) {
			t.Fatalf("unsafe manifest accepted: %s", path)
		}
	}
	for _, path := range []string{"Cargo.toml", "retired/Cargo.toml"} {
		if !safeManifest(path) {
			t.Fatalf("safe manifest rejected: %s", path)
		}
	}
}

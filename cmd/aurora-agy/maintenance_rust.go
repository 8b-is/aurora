package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// Plans are operator configuration, never model-supplied commands. Standalone
// entries opt in to the same offline, package-specific Cargo test command.
type rustOwner struct {
	Manifest string `json:"manifest"`
	Package  string `json:"package"`
	ID       string `json:"package_id"`
}
type rustCheck struct {
	rustOwner
	Phase   string   `json:"phase"`
	Command []string `json:"command"`
	Passed  bool     `json:"passed"`
}
type rustMetadata struct {
	Packages []struct {
		ID       string `json:"id"`
		Name     string `json:"name"`
		Manifest string `json:"manifest_path"`
	} `json:"packages"`
	Members []string `json:"workspace_members"`
}

func safeManifest(p string) bool {
	return !filepath.IsAbs(p) && filepath.Clean(p) == p && p != ".." && !strings.HasPrefix(p, "../") && !strings.Contains(p, "\\") && filepath.Base(p) == "Cargo.toml"
}

func (j *job) loadRustPlanMetadata(tree, plan, targetManifest string) (*rustMetadata, bool) {
	planPath := filepath.Join(tree, plan)
	targetPath := filepath.Join(tree, targetManifest)
	planBytes, err := os.ReadFile(planPath)
	if err != nil {
		return nil, false
	}
	targetBytes, err := os.ReadFile(targetPath)
	if err != nil {
		return nil, false
	}
	h := sha256.New()
	_, _ = h.Write([]byte(planPath + "\x00"))
	_, _ = h.Write(planBytes)
	_, _ = h.Write([]byte("\x00" + targetPath + "\x00"))
	_, _ = h.Write(targetBytes)
	cacheKey := hex.EncodeToString(h.Sum(nil))
	if j.rustMeta != nil {
		if cached, ok := j.rustMeta[cacheKey]; ok {
			return cached, cached != nil
		}
	}
	out, err := j.command(tree, 60, "", "cargo", "metadata", "--format-version", "1", "--no-deps", "--offline", "--manifest-path", plan)
	if j.rustMeta == nil {
		j.rustMeta = map[string]*rustMetadata{}
	}
	if err != nil {
		j.rustMeta[cacheKey] = nil
		return nil, false
	}
	var metadata rustMetadata
	if json.Unmarshal([]byte(out), &metadata) != nil {
		j.rustMeta[cacheKey] = nil
		return nil, false
	}
	j.rustMeta[cacheKey] = &metadata
	return &metadata, true
}

func (j *job) rustOwner(tree, source string) (rustOwner, error) {
	var owner rustOwner
	if !editable(source) || filepath.Ext(source) != ".rs" {
		return owner, fmt.Errorf("invalid Rust candidate %q", source)
	}
	// The nearest manifest is authoritative; never fall back to a parent package
	// when an excluded or malformed nested package cannot resolve.
	manifest := ""
	for dir := filepath.Dir(source); ; dir = filepath.Dir(dir) {
		p := filepath.Join(dir, "Cargo.toml")
		if exists(filepath.Join(tree, p)) {
			manifest = p
			break
		}
		if dir == "." {
			break
		}
	}
	if manifest == "" {
		return owner, fmt.Errorf("unresolved Rust owner: %s", source)
	}
	standalone := false
	for _, p := range j.repo.StandaloneRustManifests {
		if safeManifest(p) && p == manifest {
			standalone = true
		}
	}
	plans := []string{"Cargo.toml", "rust/Cargo.toml", "core/Cargo.toml", "wupz-rs/Cargo.toml"}
	if standalone {
		plans = []string{manifest}
	}
	for _, plan := range plans {
		if !exists(filepath.Join(tree, plan)) {
			continue
		}
		metadata, ok := j.loadRustPlanMetadata(tree, plan, manifest)
		if !ok {
			continue
		}
		for _, pkg := range metadata.Packages {
			if filepath.Clean(pkg.Manifest) != filepath.Join(tree, manifest) || pkg.ID == "" || pkg.Name == "" {
				continue
			}
			for _, member := range metadata.Members {
				if member == pkg.ID {
					return rustOwner{Manifest: manifest, Package: pkg.Name, ID: pkg.ID}, nil
				}
			}
		}
	}
	return owner, fmt.Errorf("Rust package %s is outside validation plans or unresolved", manifest)
}
func (j *job) validateRustCandidate(tree, source, phase string) error {
	owner, err := j.rustOwner(tree, source)
	if err != nil {
		return err
	}
	args := []string{"test", "--manifest-path", owner.Manifest, "--package", owner.Package, "--offline"}
	check := rustCheck{rustOwner: owner, Phase: phase, Command: append([]string{"cargo"}, args...)}
	j.receipt.Attempted++
	_, err = j.command(tree, 300, "", "cargo", args...)
	check.Passed = err == nil
	j.receipt.RustChecks = append(j.receipt.RustChecks, check)
	if err != nil {
		return fmt.Errorf("%s Rust package validation (%s, %s): %w", phase, owner.Manifest, owner.Package, err)
	}
	j.receipt.Passed++
	return nil
}

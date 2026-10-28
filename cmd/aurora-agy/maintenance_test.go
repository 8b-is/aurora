package main

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func gitTest(t *testing.T, dir string, args ...string) string {
	t.Helper()
	c := exec.Command("git", args...)
	c.Dir = dir
	b, e := c.CombinedOutput()
	if e != nil {
		t.Fatalf("git %v: %s %v", args, b, e)
	}
	return strings.TrimSpace(string(b))
}
func writeTest(t *testing.T, path, body string, mode os.FileMode) {
	t.Helper()
	if e := os.WriteFile(path, []byte(body), mode); e != nil {
		t.Fatal(e)
	}
}

func TestWritableCache(t *testing.T) {
	dir := t.TempDir()
	if !writableCache(filepath.Join(dir, "cache")) {
		t.Fatal("writable cache rejected")
	}
	blocked := filepath.Join(dir, "file")
	writeTest(t, blocked, "not a directory", 0600)
	if writableCache(filepath.Join(blocked, "cache")) {
		t.Fatal("unusable cache accepted")
	}
}

// Real local Git repositories and actual Go tests; only GitHub and inference are
// substituted. Verify the default branch and the user's checkout are untouched.
func TestMaintenancePublishesOnlyValidatedBranch(t *testing.T) {
	for _, broken := range []bool{false, true} {
		name := "valid"
		if broken {
			name = "failing_candidate"
		}
		t.Run(name, func(t *testing.T) {
			dir := t.TempDir()
			source := filepath.Join(dir, "source")
			remote := filepath.Join(dir, "remote.git")
			tools := filepath.Join(dir, "tools")
			for _, p := range []string{source, tools} {
				if e := os.Mkdir(p, 0700); e != nil {
					t.Fatal(e)
				}
			}
			gitTest(t, source, "init", "-b", "main")
			gitTest(t, source, "config", "user.name", "Fixture")
			gitTest(t, source, "config", "user.email", "fixture@localhost")
			writeTest(t, filepath.Join(source, "go.mod"), "module fixture\n\ngo 1.22\n", 0600)
			old := "package fixture\nfunc Value(x int) int { return x + 1 }\n"
			writeTest(t, filepath.Join(source, "value.go"), old, 0600)
			writeTest(t, filepath.Join(source, "value_test.go"), "package fixture\nimport \"testing\"\nfunc TestValue(t *testing.T) { if Value(1) != 2 { t.Fatal(\"regression\") } }\n", 0600)
			gitTest(t, source, "add", ".")
			gitTest(t, source, "commit", "-m", "fixture")
			gitTest(t, dir, "clone", "--bare", source, remote)
			base := gitTest(t, source, "rev-parse", "HEAD")
			gitTest(t, source, "remote", "add", "origin", "git@github.com:8b-is/aurora-fixture.git")
			t.Setenv("GIT_CONFIG_COUNT", "1")
			t.Setenv("GIT_CONFIG_KEY_0", "url.file://"+remote+".insteadOf")
			t.Setenv("GIT_CONFIG_VALUE_0", "git@github.com:8b-is/aurora-fixture.git")
			writeTest(t, filepath.Join(tools, "gh"), "#!/bin/sh\nif [ \"$2\" = list ]; then echo '[]'; else echo 'https://github.com/8b-is/aurora-fixture/pull/1'; fi\n", 0700)
			memory := filepath.Join(tools, "memory")
			writeTest(t, memory, "#!/bin/sh\nif [ \"$2\" = append ]; then cat >/dev/null; echo 1; fi\n", 0700)
			t.Setenv("PATH", tools+string(os.PathListSeparator)+os.Getenv("PATH"))
			replacement := "func Value(x int) int { if x < 0 { return 0 }; return x + 1 }"
			if broken {
				replacement = "func Value(x int) int { return 99 }"
			}
			p := proposal{Path: "value.go", Old: "func Value(x int) int { return x + 1 }", New: replacement, Reason: "Handle a negative input boundary"}
			text, _ := json.Marshal(p)
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path == "/api/tags" {
					_, _ = w.Write([]byte(`{"models":[{"name":"granite4.2:30b","size":123}]}`))
					return
				}
				_ = json.NewEncoder(w).Encode(completion{Response: string(text), Done: true, EvalCount: 10, EvalDuration: 100})
			}))
			defer server.Close()
			jobDir := filepath.Join(dir, "job")
			_ = os.Mkdir(jobDir, 0700)
			j := job{cfg: maintenanceConfig{Root: dir, MemoryBinary: memory, Endpoint: server.URL, Model: "granite4.2:30b"}, repo: repository{Name: "aurora-fixture", Path: source, Base: "main"}, dir: jobDir, receipt: receipt{Repo: "aurora-fixture"}}
			err := j.process(0)
			if broken {
				if err == nil {
					t.Fatal("failing tests published")
				}
				if !strings.Contains(err.Error(), "candidate validation") {
					t.Fatalf("wrong failure: %v", err)
				}
				if j.receipt.Commit != "" {
					t.Fatal("committed failing candidate")
				}
			} else {
				if err != nil {
					t.Fatal(err)
				}
				if j.receipt.Status != "pull_request_open" || j.receipt.PR == "" {
					t.Fatalf("bad receipt: %+v", j.receipt)
				}
				if gitTest(t, remote, "rev-parse", j.receipt.Branch) != j.receipt.Commit {
					t.Fatal("agent branch differs")
				}
				// Simulate interruption after push, before PR creation. Recovery must
				// use the recorded commit, without calling the model again.
				server.Close()
				saved := j.receipt
				saved.Status = "branch_pushed"
				saved.PR = ""
				encoded, _ := json.Marshal(saved)
				recovery := filepath.Join(dir, "recovery.jsonl")
				writeTest(t, recovery, string(encoded)+"\n", 0600)
				writeTest(t, memory, "#!/bin/sh\nif [ \"$2\" = recall ]; then cat '"+recovery+"'; else cat >/dev/null; echo 1; fi\n", 0700)
				resumedDir := filepath.Join(dir, "resumed")
				_ = os.Mkdir(resumedDir, 0700)
				resumed := job{cfg: j.cfg, repo: j.repo, dir: resumedDir, receipt: receipt{Repo: j.repo.Name}}
				if e := resumed.process(0); e != nil {
					t.Fatal(e)
				}
				if resumed.receipt.Status != "pull_request_open" || resumed.receipt.Commit != saved.Commit {
					t.Fatalf("recovery lost commit: %+v", resumed.receipt)
				}
			}
			if gitTest(t, remote, "rev-parse", "main") != base || gitTest(t, source, "rev-parse", "HEAD") != base {
				t.Fatal("changed default branch")
			}
			raw, _ := os.ReadFile(filepath.Join(source, "value.go"))
			if string(raw) != old {
				t.Fatal("changed original checkout")
			}
		})
	}
}
func TestProposalsCannotModifyTestsOrEscape(t *testing.T) {
	dir := t.TempDir()
	original := "fn f() {}\n#[cfg(test)]\nmod tests {}\n"
	writeTest(t, filepath.Join(dir, "lib.rs"), original, 0600)
	allowed := map[string]string{"lib.rs": original}
	for _, p := range []proposal{
		{Path: "../other.rs", Old: "a", New: "b", Reason: "escape"},
		{Path: "lib.rs", Old: "mod tests {}", New: "", Reason: "remove tests"},
		{Path: "lib.rs", Old: "missing", New: "new", Reason: "not exact"},
	} {
		if applyProposal(dir, p, allowed) == nil {
			t.Fatal("unsafe proposal accepted")
		}
	}
	if _, e := parseProposal(`{"no_change":true} {}`); e == nil {
		t.Fatal("trailing JSON accepted")
	}
	p, e := parseProposal("<think>\nreasoning\n</think>\n```json\n{\"no_change\":true,\"reason\":\"ok\"}\n```")
	if e != nil || !p.NoChange || p.Reason != "ok" {
		t.Fatalf("failed to unwrap think block: %+v %v", p, e)
	}
	p2, e := parseProposal("Thinking without opening tag because template prefilled it.\n</think>\nTo maintain invariants:\n```rust\nfn demo() {}\n```\n```json\n{\"no_change\":true,\"reason\":\"prefilled\"}\n```")
	if e != nil || !p2.NoChange || p2.Reason != "prefilled" {
		t.Fatalf("failed to unwrap prefilled think block: %+v %v", p2, e)
	}
	p3, e := parseProposal("Analysis:\n```rust\nconst PHI_NUM: i64 = 1618034;\n```\nResult:\n```\n{\"no_change\":true,\"reason\":\"after_rust_fence\"}\n```")
	if e != nil || !p3.NoChange || p3.Reason != "after_rust_fence" {
		t.Fatalf("failed to unwrap proposal after rust fence: %+v %v", p3, e)
	}
}

func TestTriageNextAndPruneJobDirs(t *testing.T) {
	dir := t.TempDir()
	for i := 0; i < 4; i++ {
		sub, err := os.MkdirTemp(dir, "job-demo-")
		if err != nil {
			t.Fatal(err)
		}
		_ = sub
	}
	pruneJobDirs(dir, "job-demo-", 2)
	entries, _ := os.ReadDir(dir)
	if len(entries) != 2 {
		t.Fatalf("expected 2 pruned job dirs, got %d", len(entries))
	}

	repoDir := filepath.Join(dir, "sg-demo")
	if err := os.MkdirAll(repoDir, 0700); err != nil {
		t.Fatal(err)
	}
	writeTest(t, filepath.Join(repoDir, "README.md"), "# SG Demo\nInteger state transitions.\n", 0600)
	queuePath := filepath.Join(dir, "queue.jsonl")
	writeTest(t, queuePath, `{"name":"sg-demo","source":"`+repoDir+`","shard":1,"status":"needs_source_review"}`+"\n", 0600)

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/tags" {
			_, _ = w.Write([]byte(`{"models":[{"name":"aye-deepseek:32b","size":123}]}`))
			return
		}
		resp := "<think>\nchecking\n</think>\n{\"decision\":\"reference\",\"destination\":\"/data/8b-is/sphered\",\"license\":\"MIT\",\"mechanism\":\"Integer state transitions with explicit guards.\",\"next_action\":\"Write a bounded sphered design note.\"}"
		_ = json.NewEncoder(w).Encode(completion{Response: resp, Done: true, EvalCount: 42, EvalDuration: 1000})
	}))
	defer server.Close()

	cfg := maintenanceConfig{
		Root:     dir,
		Endpoint: server.URL,
		Model:    "aye-deepseek:32b",
	}
	if err := triageNextFromConfig(cfg, queuePath); err != nil {
		t.Fatal(err)
	}
	done := loadCompletedTriageSet(filepath.Join(dir, "triage-reviews.jsonl"))
	if !done["sg-demo"] {
		t.Fatal("expected sg-demo to be recorded in triage-reviews.jsonl")
	}
}

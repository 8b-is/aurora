package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const defaultTriageQueuePath = "/data/.agent-state/research/standardgalactic-20261028/pass-02/aggregate-02/next-source-review.jsonl"
const defaultTriageOutputPath = "/data/.agent-state/research/standardgalactic-20261028/pass-02/review-aurora.jsonl"

type triageQueueRow struct {
	Name   string `json:"name"`
	Source string `json:"source"`
	Shard  int    `json:"shard"`
	Status string `json:"status"`
}

type triageModelDecision struct {
	Decision    string `json:"decision"`
	Destination string `json:"destination"`
	License     string `json:"license"`
	Mechanism   string `json:"mechanism"`
	NextAction  string `json:"next_action"`
}

type triageReceipt struct {
	Name        string `json:"name"`
	Source      string `json:"source"`
	Head        string `json:"head"`
	License     string `json:"license"`
	Decision    string `json:"decision"`
	Destination string `json:"destination"`
	Mechanism   string `json:"mechanism"`
	NextAction  string `json:"next_action"`
	Model       string `json:"model"`
	Tokens      uint64 `json:"tokens_generated"`
	EvalNS      uint64 `json:"eval_ns"`
	Timestamp   int64  `json:"timestamp_secs"`
}

func pruneJobDirs(root, prefix string, keep int) {
	if keep < 1 {
		return
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return
	}
	type dirItem struct {
		path string
		mod  int64
	}
	var matched []dirItem
	for _, e := range entries {
		if !e.IsDir() || !strings.HasPrefix(e.Name(), prefix) {
			continue
		}
		info, err := e.Info()
		if err != nil {
			continue
		}
		matched = append(matched, dirItem{path: filepath.Join(root, e.Name()), mod: info.ModTime().UnixNano()})
	}
	if len(matched) <= keep {
		return
	}
	sort.Slice(matched, func(i, j int) bool { return matched[i].mod > matched[j].mod })
	for _, item := range matched[keep:] {
		_ = os.RemoveAll(item.path)
	}
}

func parseTriageDecision(text string) (triageModelDecision, error) {
	var d triageModelDecision
	cleaned := unwrapModelJSON(text)
	dec := json.NewDecoder(strings.NewReader(cleaned))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&d); err != nil {
		return d, err
	}
	var extra any
	if err := dec.Decode(&extra); err != io.EOF {
		return d, fmt.Errorf("trailing triage data")
	}
	switch d.Decision {
	case "adapt", "reference", "defer", "reject":
	default:
		return d, fmt.Errorf("invalid decision %q", d.Decision)
	}
	if d.Mechanism == "" || d.NextAction == "" || d.Destination == "" {
		return d, fmt.Errorf("triage decision missing required fields")
	}
	return d, nil
}

func loadCompletedTriageSet(paths ...string) map[string]bool {
	done := map[string]bool{}
	for _, p := range paths {
		f, err := os.Open(p)
		if err != nil {
			continue
		}
		scanner := bufio.NewScanner(f)
		for scanner.Scan() {
			var row struct {
				Name string `json:"name"`
			}
			if json.Unmarshal(scanner.Bytes(), &row) == nil && row.Name != "" {
				done[row.Name] = true
			}
		}
		_ = f.Close()
	}
	return done
}

func collectRepoSnapshot(repoDir string) (string, string) {
	head := "unknown"
	cmd := exec.Command("git", "-C", repoDir, "rev-parse", "HEAD")
	if out, err := cmd.Output(); err == nil {
		head = strings.TrimSpace(string(out))
	}
	var summary strings.Builder
	entries, err := os.ReadDir(repoDir)
	if err != nil {
		return head, "unreadable repository"
	}
	var names []string
	for _, e := range entries {
		if e.Name() == ".git" {
			continue
		}
		names = append(names, e.Name())
	}
	sort.Strings(names)
	if len(names) > 24 {
		names = names[:24]
	}
	summary.WriteString("Top-level entries: " + strings.Join(names, ", ") + "\n")
	for _, doc := range []string{"README.md", "README.txt", "README.rst", "LICENSE", "Cargo.toml", "pyproject.toml", "package.json", "go.mod"} {
		p := filepath.Join(repoDir, doc)
		info, err := os.Lstat(p)
		if err != nil || !info.Mode().IsRegular() {
			continue
		}
		f, err := os.Open(p)
		if err != nil {
			continue
		}
		b, _ := io.ReadAll(io.LimitReader(f, 2048))
		_ = f.Close()
		if len(b) > 0 {
			summary.WriteString("\n--- " + doc + " ---\n" + string(b) + "\n")
		}
	}
	return head, summary.String()
}

func triageNextFromConfig(cfg maintenanceConfig, queuePath string) error {
	localLog := filepath.Join(cfg.Root, "triage-reviews.jsonl")
	done := loadCompletedTriageSet(localLog, defaultTriageOutputPath)

	qf, err := os.Open(queuePath)
	if err != nil {
		return err
	}
	defer func() { _ = qf.Close() }()

	var chosen *triageQueueRow
	scanner := bufio.NewScanner(qf)
	for scanner.Scan() {
		var row triageQueueRow
		if json.Unmarshal(scanner.Bytes(), &row) != nil || row.Name == "" || done[row.Name] {
			continue
		}
		chosen = &row
		break
	}
	if chosen == nil {
		return nil
	}

	head, snapshot := collectRepoSnapshot(chosen.Source)
	prompt := "Review StandardGalactic repository " + chosen.Name + " (" + chosen.Source + ", commit " + head + ") for the 8b-is family.\n" +
		"Classify its value for 8b-is using exact integer or p/q rational discipline. " +
		"Return ONLY valid JSON with exact keys: " +
		"{\"decision\":\"adapt|reference|defer|reject\",\"destination\":\"/data/8b-is/<repo>\",\"license\":\"detected SPDX or unknown\",\"mechanism\":\"1-2 sentence concrete mechanism summary\",\"next_action\":\"1 sentence bounded next action\"}.\n\n" +
		snapshot
	if len(prompt) > 24000 {
		prompt = prompt[:24000]
	}

	client, err := newLocalClient(cfg.Endpoint)
	if err != nil {
		return err
	}
	resp, err := client.complete(context.Background(), cfg.Model, prompt)
	if err != nil {
		return err
	}
	decision, err := parseTriageDecision(resp.Response)
	if err != nil {
		return fmt.Errorf("triage proposal: %w", err)
	}

	rec := triageReceipt{
		Name:        chosen.Name,
		Source:      chosen.Source,
		Head:        head,
		License:     decision.License,
		Decision:    decision.Decision,
		Destination: decision.Destination,
		Mechanism:   decision.Mechanism,
		NextAction:  decision.NextAction,
		Model:       cfg.Model,
		Tokens:      resp.EvalCount,
		EvalNS:      resp.EvalDuration,
		Timestamp:   time.Now().Unix(),
	}
	raw, err := json.Marshal(rec)
	if err != nil {
		return err
	}

	if cfg.MemoryBinary != "" && exists(cfg.MemoryBinary) {
		memPath := filepath.Join(cfg.Root, "memory", "standardgalactic-triage.m8")
		cmd := exec.Command(cfg.MemoryBinary, "memory", "append", memPath, "1", "1")
		cmd.Stdin = strings.NewReader(string(raw))
		_ = cmd.Run()
	}

	appendLine := func(path string) error {
		if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
			return err
		}
		f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
		if err != nil {
			return err
		}
		defer func() { _ = f.Close() }()
		_, err = f.Write(append(raw, '\n'))
		return err
	}
	if err := appendLine(localLog); err != nil {
		return err
	}
	if exists(filepath.Dir(defaultTriageOutputPath)) || queuePath == defaultTriageQueuePath {
		_ = appendLine(defaultTriageOutputPath)
	}
	fmt.Println(string(raw))
	return nil
}

func runTriageCLI(queuePath string) error {
	endpoint := os.Getenv("AURORA_ENDPOINT")
	if endpoint == "" {
		endpoint = "http://127.0.0.1:11434"
	}
	model := os.Getenv("AURORA_MODEL")
	if model == "" {
		model = "aye-deepseek:32b"
	}
	root := os.Getenv("AURORA_ROOT")
	if root == "" {
		root = "/var/lib/aurora"
	}
	memBin := "/usr/local/bin/aurora"
	cfg := maintenanceConfig{
		Root:         root,
		MemoryBinary: memBin,
		Endpoint:     endpoint,
		Model:        model,
	}
	return triageNextFromConfig(cfg, queuePath)
}

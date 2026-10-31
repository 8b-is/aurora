package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"testing"
	"time"
)

func TestPruneJobDirsRemovalOutcomes(t *testing.T) {
	denied := errors.New("permission denied")
	busy := errors.New("directory busy")
	for _, tc := range []struct {
		name  string
		keep  int
		fails map[string]error
		want  []string
	}{
		{name: "successful deletion", keep: 4, want: []string{"job-demo-c"}},
		{name: "one failed deletion", keep: 4, fails: map[string]error{"job-demo-c": denied}, want: []string{"job-demo-c"}},
		{name: "all successful", keep: 2, want: []string{"job-demo-e", "job-demo-a", "job-demo-c"}},
		{name: "failure between successes", keep: 2, fails: map[string]error{"job-demo-a": denied}, want: []string{"job-demo-e", "job-demo-a", "job-demo-c"}},
		{name: "multiple failures", keep: 2, fails: map[string]error{"job-demo-e": denied, "job-demo-c": busy}, want: []string{"job-demo-e", "job-demo-a", "job-demo-c"}},
		{name: "negative retention", keep: -1},
		{name: "zero retention", keep: 0},
		{name: "at retention limit", keep: 5},
		{name: "below retention limit", keep: 6},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			// Newest first, deliberately different from name/ReadDir ordering.
			names := []string{"job-demo-d", "job-demo-b", "job-demo-e", "job-demo-a", "job-demo-c"}
			for i, name := range append(names, "other-job") {
				path := filepath.Join(root, name)
				if err := os.Mkdir(path, 0700); err != nil {
					t.Fatal(err)
				}
				mod := time.Unix(1700000000, 0).Add(-time.Duration(i) * time.Hour)
				if err := os.Chtimes(path, mod, mod); err != nil {
					t.Fatal(err)
				}
			}
			// Matching files and directory symlinks must not count toward retention.
			writeTest(t, filepath.Join(root, "job-demo-file"), "keep", 0600)
			if err := os.Symlink(filepath.Join(root, "other-job"), filepath.Join(root, "job-demo-link")); err != nil {
				t.Fatal(err)
			}
			var calls []string
			var diagnostics bytes.Buffer
			remove := func(path string) error {
				calls = append(calls, path)
				return tc.fails[filepath.Base(path)]
			}
			pruneJobDirsWithRemove(root, "job-demo-", tc.keep, remove, &diagnostics)

			var wantCalls []string
			var wantDiagnostics bytes.Buffer
			for _, name := range tc.want {
				path := filepath.Join(root, name)
				wantCalls = append(wantCalls, path)
				if err := tc.fails[name]; err != nil {
					fmt.Fprintf(&wantDiagnostics, "prune job directory %q: %v\n", path, err)
				}
			}
			if !reflect.DeepEqual(calls, wantCalls) {
				t.Errorf("removal calls = %v, want %v", calls, wantCalls)
			}
			if diagnostics.String() != wantDiagnostics.String() {
				t.Errorf("diagnostics = %q, want %q", diagnostics.String(), wantDiagnostics.String())
			}
		})
	}
}

func TestPruneJobDirsNoCandidates(t *testing.T) {
	root := t.TempDir()
	for _, tc := range []struct {
		name string
		root string
	}{
		{name: "empty root", root: root},
		{name: "missing root", root: filepath.Join(root, "missing")},
	} {
		t.Run(tc.name, func(t *testing.T) {
			var diagnostics bytes.Buffer
			pruneJobDirsWithRemove(tc.root, "job-demo-", 2, func(path string) error {
				t.Errorf("unexpected removal: %s", path)
				return nil
			}, &diagnostics)
			if diagnostics.Len() != 0 {
				t.Errorf("unexpected diagnostic: %s", &diagnostics)
			}
		})
	}
}

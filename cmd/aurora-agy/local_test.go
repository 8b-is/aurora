package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestLoopbackGuard(t *testing.T) {
	for _, url := range []string{"https://example.com:443", "http://127.0.0.1.evil:80", "http://127.0.0.1:80@evil:80", "http://10.0.0.1:11434", "http://127.0.0.1:80?remote=1", "http://127.0.0.1:0"} {
		if _, err := newLocalClient(url); err == nil {
			t.Fatalf("accepted %s", url)
		}
	}
}
func TestLocalInferenceUsesMeasuredIntegers(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/tags" {
			_, _ = w.Write([]byte(`{"models":[{"name":"granite4.2:30b","size":123}]}`))
			return
		}
		var request struct {
			Options struct {
				NumCtx      int `json:"num_ctx"`
				Temperature int `json:"temperature"`
			} `json:"options"`
		}
		if err := json.NewDecoder(r.Body).Decode(&request); err != nil {
			t.Error(err)
		}
		if request.Options.NumCtx != 8192 || request.Options.Temperature != 0 {
			t.Error("unexpected inference options")
		}
		_, _ = w.Write([]byte(`{"done":true,"response":"ready","eval_count":7,"eval_duration":99}`))
	}))
	defer server.Close()
	client, err := newLocalClient(server.URL)
	if err != nil {
		t.Fatal(err)
	}
	r, err := client.complete(context.Background(), "granite4.2:30b", "hello")
	if err != nil || r.EvalCount != 7 || r.EvalDuration != 99 {
		t.Fatalf("%+v %v", r, err)
	}
	if _, err = client.complete(context.Background(), "model:cloud", "hello"); err == nil {
		t.Fatal("cloud accepted")
	}
}
func TestRedirectCannotOpenRemoteSocket(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "http://example.com:80", http.StatusFound)
	}))
	defer server.Close()
	client, err := newLocalClient(server.URL)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = client.models(context.Background()); err == nil {
		t.Fatal("redirect followed")
	}
}
func TestNumericGuard(t *testing.T) {
	for _, s := range []string{"let x: f64 = 1;", "let x = 1.25;", "let x = 1e3;"} {
		if scanSource("test.rs", s) == nil {
			t.Fatal(s)
		}
	}
	for _, s := range []string{"// f64\nlet x = 22 / 9;", "let s = r##\"f64 1.25\"##;", "for x in 0..10 { x.abs(); }", "let b = b'\\n';"} {
		if e := scanSource("test.rs", s); e != nil {
			t.Fatal(e)
		}
	}
	if scanSource("test.go", "package x; var x float64") == nil {
		t.Fatal("missed Go type")
	}
}

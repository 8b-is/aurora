package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

type localClient struct {
	base string
	http *http.Client
}
type localModel struct {
	Name string `json:"name"`
	Size int64  `json:"size"`
}
type completion struct {
	Response     string `json:"response"`
	Done         bool   `json:"done"`
	Error        string `json:"error"`
	EvalCount    uint64 `json:"eval_count"`
	EvalDuration uint64 `json:"eval_duration"`
}

func newLocalClient(endpoint string) (*localClient, error) {
	u, err := url.Parse(endpoint)
	if err != nil {
		return nil, err
	}
	host := u.Hostname()
	if host == "localhost" {
		host = "127.0.0.1"
	}
	ip := net.ParseIP(host)
	port, err := strconv.ParseUint(u.Port(), 10, 16)
	if err != nil || port == 0 || u.Scheme != "http" || ip == nil || !ip.IsLoopback() || u.User != nil || (u.Path != "" && u.Path != "/") || u.RawQuery != "" || u.Fragment != "" {
		return nil, fmt.Errorf("sovereignty violation: expected http://literal-loopback:port")
	}
	addr := net.JoinHostPort(ip.String(), u.Port())
	transport := &http.Transport{
		Proxy: nil,
		DialContext: func(ctx context.Context, network, requested string) (net.Conn, error) {
			if requested != addr {
				return nil, fmt.Errorf("sovereignty violation: unexpected destination")
			}
			return (&net.Dialer{Timeout: 5 * time.Second}).DialContext(ctx, network, addr)
		},
		ResponseHeaderTimeout: 180 * time.Second,
	}
	return &localClient{base: "http://" + addr, http: &http.Client{Transport: transport, Timeout: 180 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return fmt.Errorf("redirects are forbidden") }}}, nil
}
func (c *localClient) request(ctx context.Context, path string, body any, out any) error {
	method := "GET"
	var reader io.Reader
	if body != nil {
		b, err := json.Marshal(body)
		if err != nil {
			return err
		}
		reader = bytes.NewReader(b)
		method = "POST"
	}
	req, err := http.NewRequestWithContext(ctx, method, c.base+path, reader)
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := c.http.Do(req)
	if err != nil {
		return err
	}
	defer func() { _ = resp.Body.Close() }()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("local inference HTTP %d", resp.StatusCode)
	}
	raw, err := io.ReadAll(io.LimitReader(resp.Body, 2*1024*1024+1))
	if err != nil {
		return err
	}
	if len(raw) > 2*1024*1024 {
		return fmt.Errorf("inference response too large")
	}
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	return dec.Decode(out)
}
func (c *localClient) models(ctx context.Context) ([]localModel, error) {
	var catalog struct {
		Models []localModel `json:"models"`
	}
	err := c.request(ctx, "/api/tags", nil, &catalog)
	return catalog.Models, err
}
func (c *localClient) complete(ctx context.Context, model, prompt string) (completion, error) {
	var result completion
	models, err := c.models(ctx)
	if err != nil {
		return result, err
	}
	found := false
	for _, m := range models {
		if m.Name == model && m.Size > 0 && !strings.Contains(strings.ToLower(model), "cloud") {
			found = true
		}
	}
	if !found {
		return result, fmt.Errorf("model %q must have locally installed weights; no downloads or cloud fallback", model)
	}
	body := map[string]any{"model": model, "prompt": prompt, "stream": false, "think": false, "keep_alive": -1, "options": map[string]int{"num_ctx": 8192, "num_predict": 4096, "temperature": 0, "seed": 432}}
	if strings.Contains(prompt, "Return only JSON") || strings.Contains(prompt, "Return ONLY valid JSON") {
		body["format"] = "json"
	}
	if err = c.request(ctx, "/api/generate", body, &result); err != nil {
		return result, err
	}
	if result.Error != "" || !result.Done || result.Response == "" {
		return result, fmt.Errorf("local generation incomplete: %s", result.Error)
	}
	return result, nil
}

// AURORA cortex: loopback inference, persistent memory, validated maintenance.
package main

import (
	"context"
	"fmt"
	"os"
	"strings"
)

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
func run(args []string) error {
	if len(args) == 0 {
		args = []string{"status"}
	}
	switch args[0] {
	case "audit":
		if len(args) != 2 {
			return fmt.Errorf("audit requires a source directory")
		}
		if err := scanTree(args[1]); err != nil {
			return err
		}
		fmt.Println("source numeric guard passed")
	case "help", "--help", "-h":
		fmt.Println("aurora-agy status | chat PROMPT | hop +-+ | cortex | omnifold | audit PATH | maintain CONFIG [--once]\nAll inference uses literal loopback addresses. Maintenance creates agent branches and PRs.")
	case "cortex":
		fmt.Println("via agy cortex: subagents=[codewhale,entheai] continuation=google/gemini-4-argon quant=mlx-quant-b158 overflow={<3,<3,<3}+1")
	case "omnifold":
		return runOmnifold()
	case "hop":
		input, err := parseTrits(strings.Join(args[1:], ""))
		if err != nil {
			return err
		}
		net := newBitNetB158([]TernaryLayer{newTernaryLayer([][]Trit{{1, -1, 0}, {0, 1, 1}, {-1, 0, 1}}), newTernaryLayer([][]Trit{{1, 1, -1}})})
		out, err := net.forward(input)
		if err != nil {
			return err
		}
		fmt.Printf("ternary demonstration input=%v logits=%v\n", input, out)
	case "status", "chat":
		endpoint := os.Getenv("AURORA_ENDPOINT")
		if endpoint == "" {
			endpoint = "http://127.0.0.1:11434"
		}
		model := os.Getenv("AURORA_MODEL")
		if model == "" {
			model = "granite4.2:30b"
		}
		client, err := newLocalClient(endpoint)
		if err != nil {
			return err
		}
		if args[0] == "status" {
			models, err := client.models(context.Background())
			if err != nil {
				return err
			}
			for _, m := range models {
				fmt.Printf("%s bytes=%d\n", m.Name, m.Size)
			}
			return nil
		}
		if len(args) < 2 {
			return fmt.Errorf("chat requires a prompt")
		}
		response, err := client.complete(context.Background(), model, strings.Join(args[1:], " "))
		if err != nil {
			return err
		}
		fmt.Println(response.Response)
		fmt.Fprintf(os.Stderr, "tokens=%d eval_ns=%d (measured)\n", response.EvalCount, response.EvalDuration)
	case "maintain":
		if len(args) < 2 {
			return fmt.Errorf("maintain requires a configuration path")
		}
		return maintenanceLoop(args[1], len(args) == 3 && args[2] == "--once")
	default:
		return fmt.Errorf("unknown command %q", args[0])
	}
	return nil
}
func parseTrits(text string) ([]Trit, error) {
	if text == "" {
		text = "+-+"
	}
	var out []Trit
	for _, c := range text {
		switch c {
		case '+', '1':
			out = append(out, 1)
		case '-':
			out = append(out, -1)
		case '0':
			out = append(out, 0)
		case ' ', ',':
		default:
			return nil, fmt.Errorf("invalid trit %q", c)
		}
	}
	if len(out) != 3 {
		return nil, fmt.Errorf("hop requires exactly three trits")
	}
	return out, nil
}

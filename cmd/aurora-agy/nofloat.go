package main

import (
	"fmt"
	"go/scanner"
	"go/token"
	"os"
	"path/filepath"
	"strings"
	"unicode"
)

func floatType(s string) bool {
	switch s {
	case "f16", "f32", "f64", "f128", "float32", "float64", "complex64", "complex128", "float", "double", "Float", "Double":
		return true
	}
	return false
}
func scanSource(path, text string) error {
	if filepath.Ext(path) == ".go" {
		var s scanner.Scanner
		fset := token.NewFileSet()
		f := fset.AddFile(path, -1, len(text))
		s.Init(f, []byte(text), nil, 0)
		for {
			pos, tok, lit := s.Scan()
			if tok == token.EOF {
				break
			}
			if tok == token.FLOAT || tok == token.IMAG || tok == token.IDENT && floatType(lit) {
				return fmt.Errorf("floating-point token at %s", fset.Position(pos))
			}
		}
		if s.ErrorCount > 0 {
			return fmt.Errorf("invalid Go tokens in %s", path)
		}
		return nil
	}
	// Rust/JS lexical guard. Strings and nested comments are not executable tokens.
	b := []byte(text)
	for i := 0; i < len(b); {
		if strings.HasPrefix(string(b[i:]), "//") {
			for i < len(b) && b[i] != '\n' {
				i++
			}
			continue
		}
		if strings.HasPrefix(string(b[i:]), "/*") {
			depth := 1
			i += 2
			for i < len(b) && depth > 0 {
				if strings.HasPrefix(string(b[i:]), "/*") {
					depth++
					i += 2
				} else if strings.HasPrefix(string(b[i:]), "*/") {
					depth--
					i += 2
				} else {
					i++
				}
			}
			if depth != 0 {
				return fmt.Errorf("unterminated comment")
			}
			continue
		}
		if b[i] == 'r' {
			j := i + 1
			for j < len(b) && b[j] == '#' {
				j++
			}
			if j < len(b) && b[j] == '"' {
				end := "\"" + strings.Repeat("#", j-i-1)
				n := strings.Index(string(b[j+1:]), end)
				if n < 0 {
					return fmt.Errorf("unterminated raw string")
				}
				i = j + 1 + n + len(end)
				continue
			}
		}
		quoted := b[i] == '"' || b[i] == '`' || (b[i] == '\'' && (filepath.Ext(path) == ".js" || i+2 < len(b) && b[i+2] == '\'' || i+3 < len(b) && b[i+1] == '\\' && b[i+3] == '\''))
		if quoted {
			q := b[i]
			i++
			closed := false
			for i < len(b) {
				if b[i] == '\\' {
					i += 2
				} else if b[i] == q {
					i++
					closed = true
					break
				} else {
					i++
				}
			}
			if !closed {
				return fmt.Errorf("unterminated string")
			}
			continue
		}
		if unicode.IsLetter(rune(b[i])) || b[i] == '_' {
			start := i
			for i < len(b) && (unicode.IsLetter(rune(b[i])) || b[i] >= '0' && b[i] <= '9' || b[i] == '_') {
				i++
			}
			if floatType(string(b[start:i])) {
				return fmt.Errorf("floating-point type in %s at byte %d", path, start)
			}
			continue
		}
		if b[i] >= '0' && b[i] <= '9' {
			start := i
			hex := strings.HasPrefix(string(b[i:]), "0x") || strings.HasPrefix(string(b[i:]), "0X")
			i++
			for i < len(b) && (unicode.IsLetter(rune(b[i])) || b[i] >= '0' && b[i] <= '9' || b[i] == '_') {
				i++
			}
			lit := string(b[start:i])
			trimmed := strings.TrimSuffix(strings.TrimSuffix(lit, "usize"), "isize")
			floating := !hex && strings.ContainsAny(trimmed, "eE") || strings.HasSuffix(lit, "f32") || strings.HasSuffix(lit, "f64")
			if i < len(b) && b[i] == '.' && (i+1 == len(b) || b[i+1] != '.' && (b[i+1] >= '0' && b[i+1] <= '9' || !unicode.IsLetter(rune(b[i+1])) && b[i+1] != '_')) {
				floating = true
			}
			if floating {
				return fmt.Errorf("floating-point literal in %s at byte %d", path, start)
			}
			continue
		}
		i++
	}
	return nil
}
func scanTree(tree string) error {
	return filepath.WalkDir(tree, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			switch d.Name() {
			case ".git", "target", "node_modules", "vendor", ".st":
				return filepath.SkipDir
			}
			return nil
		}
		ext := filepath.Ext(path)
		if ext != ".go" && ext != ".rs" && ext != ".js" && ext != ".html" {
			return nil
		}
		if d.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("source symlink requires review: %s", path)
		}
		raw, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		text := string(raw)
		if ext == ".html" {
			for {
				start := strings.Index(text, "<script>")
				if start < 0 {
					return nil
				}
				text = text[start+len("<script>"):]
				end := strings.Index(text, "</script>")
				if end < 0 {
					return fmt.Errorf("unterminated script")
				}
				if err = scanSource(path+".js", text[:end]); err != nil {
					return err
				}
				text = text[end+len("</script>"):]
			}
		}
		return scanSource(path, text)
	})
}

func byteLine(text string, offset int) int {
	if offset > len(text) {
		offset = len(text)
	}
	return strings.Count(text[:offset], "\n") + 1
}

func collectFloatViolationLines(path, text string) []int {
	lines := strings.Split(text, "\n")
	var badLines []int
	seen := map[int]bool{}
	addLine := func(ln int) {
		if ln >= 1 && ln <= len(lines) && !seen[ln] {
			seen[ln] = true
			badLines = append(badLines, ln)
		}
	}
	if filepath.Ext(path) == ".go" {
		var s scanner.Scanner
		fset := token.NewFileSet()
		f := fset.AddFile(path, -1, len(text))
		s.Init(f, []byte(text), nil, 0)
		for {
			pos, tok, lit := s.Scan()
			if tok == token.EOF {
				break
			}
			if tok == token.FLOAT || tok == token.IMAG || tok == token.IDENT && floatType(lit) {
				addLine(fset.Position(pos).Line)
			}
		}
	} else {
		b := []byte(text)
		for i := 0; i < len(b); {
			if strings.HasPrefix(string(b[i:]), "//") {
				for i < len(b) && b[i] != '\n' {
					i++
				}
				continue
			}
			if strings.HasPrefix(string(b[i:]), "/*") {
				depth := 1
				i += 2
				for i < len(b) && depth > 0 {
					if strings.HasPrefix(string(b[i:]), "/*") {
						depth++
						i += 2
					} else if strings.HasPrefix(string(b[i:]), "*/") {
						depth--
						i += 2
					} else {
						i++
					}
				}
				continue
			}
			if b[i] == 'r' {
				j := i + 1
				for j < len(b) && b[j] == '#' {
					j++
				}
				if j < len(b) && b[j] == '"' {
					end := "\"" + strings.Repeat("#", j-i-1)
					n := strings.Index(string(b[j+1:]), end)
					if n >= 0 {
						i = j + 1 + n + len(end)
						continue
					}
				}
			}
			quoted := b[i] == '"' || b[i] == '`' || (b[i] == '\'' && (filepath.Ext(path) == ".js" || i+2 < len(b) && b[i+2] == '\'' || i+3 < len(b) && b[i+1] == '\\' && b[i+3] == '\''))
			if quoted {
				q := b[i]
				i++
				for i < len(b) {
					if b[i] == '\\' {
						i += 2
					} else if b[i] == q {
						i++
						break
					} else {
						i++
					}
				}
				continue
			}
			if unicode.IsLetter(rune(b[i])) || b[i] == '_' {
				start := i
				for i < len(b) && (unicode.IsLetter(rune(b[i])) || b[i] >= '0' && b[i] <= '9' || b[i] == '_') {
					i++
				}
				if floatType(string(b[start:i])) {
					addLine(byteLine(text, start))
				}
				continue
			}
			if b[i] >= '0' && b[i] <= '9' {
				start := i
				hex := strings.HasPrefix(string(b[i:]), "0x") || strings.HasPrefix(string(b[i:]), "0X")
				i++
				for i < len(b) && (unicode.IsLetter(rune(b[i])) || b[i] >= '0' && b[i] <= '9' || b[i] == '_') {
					i++
				}
				lit := string(b[start:i])
				litCore := strings.TrimSuffix(strings.TrimSuffix(lit, "usize"), "isize")
				floating := !hex && strings.ContainsAny(litCore, "eE") || strings.HasSuffix(lit, "f32") || strings.HasSuffix(lit, "f64")
				if i < len(b) && b[i] == '.' && (i+1 == len(b) || b[i+1] != '.' && (b[i+1] >= '0' && b[i+1] <= '9' || !unicode.IsLetter(rune(b[i+1])) && b[i+1] != '_')) {
					floating = true
				}
				if floating {
					addLine(byteLine(text, start))
				}
				continue
			}
			i++
		}
	}
	return badLines
}

func countFloatViolations(path, text string) int {
	return len(collectFloatViolationLines(path, text))
}

func describeFloatViolations(path, text string) string {
	lines := collectFloatViolationLines(path, text)
	if len(lines) == 0 {
		return ""
	}
	allLines := strings.Split(text, "\n")
	var b strings.Builder
	if len(allLines) <= 220 {
		b.WriteString("Offending floating-point lines in " + path + " (update all callers/callees in this file so types match):\n")
		for _, ln := range lines {
			if ln >= 1 && ln <= len(allLines) {
				b.WriteString(fmt.Sprintf("  line %d: %s\n", ln, strings.TrimSpace(allLines[ln-1])))
			}
		}
		return b.String()
	}
	b.WriteString("Target floating-point cluster in " + path + " (fix this function/section first; copy exact original lines including any comments into \"old\"):\n")
	firstLine := lines[0]
	shown := 0
	for _, ln := range lines {
		if ln-firstLine > 35 && shown >= 1 {
			break
		}
		if ln >= 1 && ln <= len(allLines) {
			b.WriteString(fmt.Sprintf("  line %d: %s\n", ln, strings.TrimSpace(allLines[ln-1])))
			shown++
		}
	}
	return b.String()
}

func excerptForPrompt(path, text string) string {
	if len(text) <= 7500 {
		return text
	}
	lines := collectFloatViolationLines(path, text)
	allLines := strings.Split(text, "\n")
	if len(lines) == 0 {
		End := 160
		if End > len(allLines) {
			End = len(allLines)
		}
		return strings.Join(allLines[:End], "\n")
	}
	first := lines[0]
	last := first
	for _, ln := range lines {
		if ln-first <= 35 {
			last = ln
		}
	}
	startIdx := first - 35
	if startIdx < 0 {
		startIdx = 0
	}
	endIdx := last + 45
	if endIdx > len(allLines) {
		endIdx = len(allLines)
	}
	return strings.Join(allLines[startIdx:endIdx], "\n")
}

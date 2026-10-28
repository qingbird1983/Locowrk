package adapter

import (
	"bufio"
	"strings"
	"testing"
)

func TestReadLineRejectsOversized(t *testing.T) {
	r := bufio.NewReaderSize(strings.NewReader(strings.Repeat("x", 1<<20+10)+"\n"), 64*1024)
	if _, err := readLine(r, maxLineBytes); err == nil {
		t.Fatal("oversized line accepted")
	} else {
		t.Logf("error: %v", err)
	}
}

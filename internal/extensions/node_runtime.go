package extensions

import (
	"context"
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// PinnedNodeVersion is the bundled Node runtime version (ADR-0015).
// scripts/fetch-node-runtime.mjs installs exactly this version and records its
// checksum in runtime/README.md.
const PinnedNodeVersion = "v26.13.0"

// CheckNodeVersion runs `<nodePath> --version` and requires the pinned version.
// ADR-0015 §3: a mismatched runtime disables the A track with a diagnostic
// instead of attempting an upgrade or silently using a different Node.
func CheckNodeVersion(ctx context.Context, nodePath, want string) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	out, err := exec.CommandContext(ctx, nodePath, "--version").Output()
	if err != nil {
		return fmt.Errorf("Node runtime %s did not report a version: %w", filepath.Base(nodePath), err)
	}
	got := strings.TrimSpace(string(out))
	if got != want {
		return fmt.Errorf("Node runtime is %s, this build needs %s", got, want)
	}
	return nil
}

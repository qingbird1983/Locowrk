//go:build windows

package adapter

import (
	"os/exec"
	"strconv"
)

// prepareCommand is a no-op on Windows; process-tree cleanup uses taskkill /T.
func prepareCommand(_ *exec.Cmd) {}

// killTree terminates the process and its descendants.
func killTree(pid int) {
	cmd := exec.Command("taskkill", "/T", "/F", "/PID", strconv.Itoa(pid))
	_ = cmd.Run()
}

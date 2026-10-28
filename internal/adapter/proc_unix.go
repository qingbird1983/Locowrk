//go:build !windows

package adapter

import (
	"os/exec"
	"syscall"
)

// prepareCommand puts the plugin in its own process group so the whole tree can
// be signalled at once.
func prepareCommand(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

// killTree terminates the process group.
func killTree(pid int) {
	_ = syscall.Kill(-pid, syscall.SIGKILL)
}

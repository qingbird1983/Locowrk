//go:build !windows

package adapter

import (
	"os/exec"
	"syscall"
)

// processTree is the plugin's process group; signalling it reaches every
// descendant that stayed in the group.
type processTree struct {
	pid int
}

// prepareCommand puts the plugin in its own process group so the whole tree can
// be signalled at once.
func prepareCommand(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

// attachProcessTree records the group to signal later.
func attachProcessTree(cmd *exec.Cmd) *processTree {
	if cmd == nil || cmd.Process == nil {
		return nil
	}
	return &processTree{pid: cmd.Process.Pid}
}

// kill signals the whole process group, including members whose leader exited.
func (t *processTree) kill() {
	if t == nil || t.pid <= 0 {
		return
	}
	killTree(t.pid)
}

// close is a no-op: the group has no handle to release.
func (t *processTree) close() {}

// killTree terminates the process group.
func killTree(pid int) {
	_ = syscall.Kill(-pid, syscall.SIGKILL)
}

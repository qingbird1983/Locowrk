package adapter

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"time"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// osExecExitError aliases the standard exit error for errors.As.
type osExecExitError = exec.ExitError

// waitForRegistrations gives the plugin a short window to register tools and
// commands after the handshake; late registrations are refused.
func (a *Adapter) waitForRegistrations(ctx context.Context) {
	deadline := time.Now().Add(a.opts.ReadyGrace)
	quiet := time.NewTimer(a.opts.ReadyQuiet)
	defer quiet.Stop()
	for {
		if time.Now().After(deadline) {
			return
		}
		select {
		case <-a.regCh:
			if !quiet.Stop() {
				select {
				case <-quiet.C:
				default:
				}
			}
			quiet.Reset(a.opts.ReadyQuiet)
		case <-quiet.C:
			return
		case <-ctx.Done():
			return
		}
	}
}

func (a *Adapter) setFailed(err error) {
	a.mu.Lock()
	a.state = StateFailed
	a.lastErr = err
	a.mu.Unlock()
}

func (a *Adapter) lastError() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.lastErr
}

func (a *Adapter) watchExit(p *peer) {
	err := <-p.waitCh
	// The plugin is gone; take any process it spawned with it.
	p.kill()
	a.mu.Lock()
	if a.stopping || a.peer != p || a.state == StateStopped {
		a.mu.Unlock()
		return
	}
	exit := &ExitInfo{At: time.Now().UTC(), Err: errorString(err), Code: exitCode(err), Stderr: p.stderrText()}
	a.exit = exit
	a.state = StateFailed
	if readErr := p.readError(); readErr != nil {
		// A protocol violation caused the kill; keep that as the cause.
		a.lastErr = readErr
	} else {
		a.lastErr = fmt.Errorf("%w: exit code %d", ErrCrashed, exit.Code)
	}
	restarts := a.restarts
	a.restarts++
	maxRestarts := a.opts.MaxRestarts
	manifest := a.manifest
	a.mu.Unlock()

	a.addDiagnostic(Diagnostic{
		Plugin:   a.pluginName,
		Member:   "process",
		Category: CategoryUnsupported,
		Code:     "crashed",
		Message:  fmt.Sprintf("plugin exited with code %d", exit.Code),
	})
	if restarts >= maxRestarts || manifest == nil {
		return
	}
	delay := a.opts.RestartBackoff[len(a.opts.RestartBackoff)-1]
	if restarts < len(a.opts.RestartBackoff) {
		delay = a.opts.RestartBackoff[restarts]
	}
	time.AfterFunc(delay, func() {
		if a.State() == StateStopped {
			return
		}
		ctx, cancel := context.WithTimeout(context.Background(), a.opts.HandshakeTimeout)
		defer cancel()
		if err := a.Start(ctx, manifest); err != nil {
			a.setFailed(err)
		}
	})
}

func errorString(err error) string {
	if err == nil {
		return ""
	}
	return redact(err.Error())
}

// exitCode extracts a process exit code, or -1 when it was signalled.
func exitCode(err error) int {
	if err == nil {
		return 0
	}
	var exitErr *osExecExitError
	if errors.As(err, &exitErr) {
		return exitErr.ExitCode()
	}
	return -1
}

func (a *Adapter) startHeartbeat(p *peer) {
	stop := make(chan struct{})
	done := make(chan struct{})
	a.mu.Lock()
	a.hbStop = stop
	a.hbDone = done
	a.mu.Unlock()
	go func() {
		defer close(done)
		ticker := time.NewTicker(a.opts.HeartbeatInterval)
		defer ticker.Stop()
		missed := 0
		for {
			select {
			case <-stop:
				return
			case <-ticker.C:
				ctx, cancel := context.WithTimeout(context.Background(), a.opts.HeartbeatTimeout)
				_, err := p.call(ctx, a.opts.HeartbeatTimeout, "ping", map[string]any{})
				cancel()
				if err == nil {
					missed = 0
					continue
				}
				missed++
				if missed >= a.opts.MaxMissedPings {
					a.handleHeartbeatLoss(p)
					return
				}
			}
		}
	}()
}

func (a *Adapter) handleHeartbeatLoss(p *peer) {
	a.addDiagnostic(Diagnostic{
		Plugin:   a.pluginName,
		Member:   "heartbeat",
		Category: CategoryUnsupported,
		Code:     "timeout",
		Message:  "plugin missed heartbeats and was terminated",
	})
	a.setFailed(fmt.Errorf("%w: heartbeat lost", ErrTimeout))
	p.kill()
}

// addDiagnostic records a host-side diagnostic (plugin-reported ones go through
// handleDiagnostic).
func (a *Adapter) addDiagnostic(d Diagnostic) {
	if d.Plugin == "" {
		d.Plugin = a.pluginName
	}
	entry, first := a.diagnostics.add(d)
	if first {
		a.publish(eventbus.KindPluginDiagnostic, entry)
	}
}

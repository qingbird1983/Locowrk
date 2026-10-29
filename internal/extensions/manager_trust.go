package extensions

import (
	"context"
	"errors"
	"fmt"
)

// Trust records a trust decision and starts a newly trusted extension.
func (m *Manager) Trust(name string) error {
	return m.SetTrusted(name, true)
}

// SetTrusted records or revokes trust. Revoking stops the process immediately.
func (m *Manager) SetTrusted(name string, trusted bool) error {
	m.mu.Lock()
	c := m.candidate(name)
	info := m.infos[name]
	a := m.adapters[name]
	m.mu.Unlock()
	if c == nil || c.Manifest == nil {
		return fmt.Errorf("extensions: unknown extension %q", name)
	}
	if m.opts.Store == nil {
		return errors.New("extensions: no trust store")
	}
	enabled := c.Enabled
	if !trusted {
		enabled = false
	} else if !c.Trusted {
		enabled = true
	}
	if err := m.opts.Store.SaveExtension(name, c.Dir, c.SHA, enabled, trusted); err != nil {
		return err
	}
	c.Enabled, c.Trusted = enabled, trusted
	if info != nil {
		info.Enabled, info.Trusted = enabled, trusted
	}
	if !trusted {
		if a != nil {
			_ = a.Close()
			m.mu.Lock()
			delete(m.adapters, name)
			m.mu.Unlock()
		}
		if info != nil {
			info.State = "untrusted"
			info.Reason = "extension is not trusted"
		}
		return nil
	}
	c.State = "ready"
	if info != nil {
		info.State, info.Reason = "stopped", ""
	}
	return m.startIfNeeded(context.Background(), c)
}

// SetEnabled persists an enable toggle and starts/stops the process.
func (m *Manager) SetEnabled(name string, on bool) error {
	m.mu.Lock()
	c := m.candidate(name)
	info := m.infos[name]
	a := m.adapters[name]
	m.mu.Unlock()
	if c == nil || c.Manifest == nil {
		return fmt.Errorf("extensions: unknown extension %q", name)
	}
	if m.opts.Store == nil {
		return errors.New("extensions: no trust store")
	}
	path, sha, _, trusted, ok := m.opts.Store.Extension(name)
	if !ok {
		path, sha, trusted = c.Dir, c.SHA, false
	}
	if err := m.opts.Store.SaveExtension(name, path, sha, on, trusted); err != nil {
		return err
	}
	c.Enabled = on
	if info != nil {
		info.Enabled = on
	}
	if !on {
		if a != nil {
			_ = a.Close()
			m.mu.Lock()
			delete(m.adapters, name)
			m.mu.Unlock()
		}
		if info != nil {
			info.State = "disabled"
		}
		return nil
	}
	return m.startIfNeeded(context.Background(), c)
}

func (m *Manager) startIfNeeded(ctx context.Context, c *Candidate) error {
	if err := m.checkRuntime(ctx); err != nil {
		m.fail(c, err)
		return err
	}
	m.mu.Lock()
	_, running := m.adapters[c.Name]
	m.mu.Unlock()
	if running {
		return nil
	}
	if err := m.decide(c); err != nil {
		return err
	}
	if c.State != "ready" {
		return nil
	}
	if err := m.start(ctx, c); err != nil {
		m.fail(c, err)
		return err
	}
	return nil
}

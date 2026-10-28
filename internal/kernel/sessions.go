package kernel

import (
	"github.com/sky-valley/pi/coding"
)

// SessionSummary is one stored session, as shown in a session list.
type SessionSummary struct {
	ID        string `json:"id"`
	Cwd       string `json:"cwd"`
	Path      string `json:"path"`
	Timestamp string `json:"timestamp"`
	Messages  int    `json:"messages"`
}

// ListSessions returns the stored sessions for cwd, newest first, using
// upstream's session store. It includes sessions this process has not opened.
func (s *Service) ListSessions(cwd string) []SessionSummary {
	infos := coding.ListSessions(cwd, "")
	out := make([]SessionSummary, 0, len(infos))
	for _, info := range infos {
		out = append(out, SessionSummary{
			ID:        info.ID,
			Cwd:       info.Cwd,
			Path:      info.Path,
			Timestamp: info.Timestamp,
			Messages:  info.Messages,
		})
	}
	return out
}

package kernel

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/qingbird1983/locowrk/pi/agent"
	"github.com/qingbird1983/locowrk/pi/ai"
)

// ToolResult is what an externally registered tool returns to the agent.
type ToolResult struct {
	// Text is the model-facing output.
	Text string
	// Details is structured data for logs and UI; may be nil.
	Details json.RawMessage
	// IsError reports a tool failure without a transport error.
	IsError bool
}

// ToolSpec describes a tool owned outside the kernel (for example an
// orchestration plugin). Parameters is a JSON Schema document: it is handed to
// the agent loop unchanged, which validates model-supplied arguments against it
// before Execute runs.
type ToolSpec struct {
	Name        string
	Label       string
	Description string
	Parameters  json.RawMessage
	Execute     func(ctx context.Context, toolCallID string, args json.RawMessage) (ToolResult, error)
}

// agentTool converts the project spec into the upstream tool type. Upstream
// types stay inside this package (architecture/03 seam #12).
func (spec ToolSpec) agentTool() (agent.AgentTool, error) {
	if spec.Name == "" {
		return agent.AgentTool{}, fmt.Errorf("kernel: tool needs a name")
	}
	if spec.Execute == nil {
		return agent.AgentTool{}, fmt.Errorf("kernel: tool %s needs Execute", spec.Name)
	}
	schema := &ai.Schema{Type: "object", Properties: map[string]*ai.Schema{}}
	if len(spec.Parameters) > 0 {
		if err := json.Unmarshal(spec.Parameters, schema); err != nil {
			return agent.AgentTool{}, fmt.Errorf("kernel: tool %s parameters: %w", spec.Name, err)
		}
		if schema.Type == "" {
			schema.Type = "object"
		}
	}
	label := spec.Label
	if label == "" {
		label = spec.Name
	}
	execute := spec.Execute
	return agent.AgentTool{
		Name:        spec.Name,
		Label:       label,
		Description: spec.Description,
		Parameters:  schema,
		Execute: func(ctx context.Context, toolCallID string, params map[string]any, _ agent.ToolUpdateFunc) (agent.AgentToolResult, error) {
			raw, err := json.Marshal(params)
			if err != nil {
				return agent.AgentToolResult{}, fmt.Errorf("kernel: tool %s arguments: %w", spec.Name, err)
			}
			result, err := execute(ctx, toolCallID, raw)
			if err != nil {
				return agent.AgentToolResult{}, err
			}
			out := agent.AgentToolResult{
				Content: ai.ContentList{ai.TextContent{Text: result.Text}},
				IsError: result.IsError,
			}
			if len(result.Details) > 0 {
				var details any
				if err := json.Unmarshal(result.Details, &details); err == nil {
					out.Details = details
				}
			}
			return out, nil
		},
	}, nil
}

// agentTools converts every extra tool, failing on the first invalid spec.
func agentTools(specs []ToolSpec) ([]agent.AgentTool, error) {
	out := make([]agent.AgentTool, 0, len(specs))
	for _, spec := range specs {
		tool, err := spec.agentTool()
		if err != nil {
			return nil, err
		}
		out = append(out, tool)
	}
	return out, nil
}

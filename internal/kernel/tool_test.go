package kernel

import (
	"context"
	"encoding/json"
	"strings"
	"testing"

	"github.com/qingbird1983/locowrk/pi/ai"
	"github.com/qingbird1983/locowrk/pi/ai/providers"
)

func TestExtraToolRunsFromFauxTurn(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()
	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxToolCall("plugin_echo", map[string]any{"text": "from-plugin"}),
		}, ai.StopToolUse)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText("done"),
		}, ai.StopStop)),
	})

	var gotArgs string
	pluginTool := ToolSpec{
		Name:        "plugin_echo",
		Description: "Echo text from an orchestration plugin.",
		Parameters:  json.RawMessage(`{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}`),
		Execute: func(_ context.Context, _ string, args json.RawMessage) (ToolResult, error) {
			var in struct {
				Text string `json:"text"`
			}
			if err := json.Unmarshal(args, &in); err != nil {
				return ToolResult{}, err
			}
			gotArgs = in.Text
			return ToolResult{Text: "echo:" + in.Text, Details: json.RawMessage(`{"ok":true}`)}, nil
		},
	}

	svc := serviceFor(t, reg.GetModel())
	id, err := svc.CreateSession(context.Background(), SessionOptions{
		Cwd:        cwd,
		Model:      ref,
		Tools:      []string{"plugin_echo"},
		ExtraTools: []ToolSpec{pluginTool},
	})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	if err := svc.Prompt(context.Background(), id, "call the plugin tool"); err != nil {
		t.Fatalf("Prompt: %v", err)
	}
	if gotArgs != "from-plugin" {
		t.Fatalf("tool args = %q", gotArgs)
	}
	history, err := svc.History(context.Background(), id)
	if err != nil {
		t.Fatalf("History: %v", err)
	}
	found := false
	for _, msg := range history {
		if strings.Contains(msg.Text, "echo:from-plugin") {
			found = true
		}
	}
	if !found {
		t.Fatalf("tool result not in history: %+v", history)
	}
}

func TestToolSpecRejectsBadSpecs(t *testing.T) {
	if _, err := (ToolSpec{}).agentTool(); err == nil {
		t.Fatal("empty spec accepted")
	}
	if _, err := (ToolSpec{Name: "x"}).agentTool(); err == nil {
		t.Fatal("spec without Execute accepted")
	}
	if _, err := (ToolSpec{Name: "x", Execute: func(context.Context, string, json.RawMessage) (ToolResult, error) {
		return ToolResult{}, nil
	}, Parameters: json.RawMessage(`not json`)}).agentTool(); err == nil {
		t.Fatal("invalid schema accepted")
	}
}

func TestAgentToolMapsDetailsAndErrors(t *testing.T) {
	spec := ToolSpec{
		Name: "plugin_fail",
		Execute: func(context.Context, string, json.RawMessage) (ToolResult, error) {
			return ToolResult{Text: "nope", Details: json.RawMessage(`{"reason":"fixture"}`), IsError: true}, nil
		},
	}
	tool, err := spec.agentTool()
	if err != nil {
		t.Fatalf("agentTool: %v", err)
	}
	if tool.Label != "plugin_fail" {
		t.Fatalf("label = %q", tool.Label)
	}
	result, err := tool.Execute(context.Background(), "call-1", map[string]any{}, nil)
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if !result.IsError {
		t.Fatal("IsError not mapped")
	}
	details, ok := result.Details.(map[string]any)
	if !ok || details["reason"] != "fixture" {
		t.Fatalf("details = %#v", result.Details)
	}
	if len(result.Content) != 1 || result.Content[0].(ai.TextContent).Text != "nope" {
		t.Fatalf("content = %#v", result.Content)
	}
}

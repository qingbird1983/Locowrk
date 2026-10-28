package desktop

import (
	"go/parser"
	"go/token"
	"reflect"
	"strings"
	"testing"

	"github.com/egoist/mygo/ui"
)

// TestPluginsHoldNoElements keeps the frame discipline: plugin state may not
// retain a ui.Element, which is only valid until the next frame.
func TestPluginsHoldNoElements(t *testing.T) {
	elementType := reflect.TypeOf(ui.Element{})
	for _, plugin := range defaultPlugins() {
		if holdsType(reflect.TypeOf(plugin), elementType, map[reflect.Type]bool{}) {
			t.Errorf("%s holds a ui.Element", plugin.Name())
		}
	}
}

// holdsType walks a type, following pointers, slices, arrays, maps and
// struct fields, looking for target.
func holdsType(t reflect.Type, target reflect.Type, seen map[reflect.Type]bool) bool {
	for t != nil && (t.Kind() == reflect.Pointer || t.Kind() == reflect.Slice || t.Kind() == reflect.Array) {
		t = t.Elem()
	}
	if t == nil {
		return false
	}
	if t == target {
		return true
	}
	if seen[t] {
		return false
	}
	seen[t] = true
	switch t.Kind() {
	case reflect.Struct:
		for i := 0; i < t.NumField(); i++ {
			if holdsType(t.Field(i).Type, target, seen) {
				return true
			}
		}
	case reflect.Map:
		return holdsType(t.Key(), target, seen) || holdsType(t.Elem(), target, seen)
	}
	return false
}

// TestPluginsStayOffKernel keeps the import boundary: plugins read project
// DTOs and MyGo, never the kernel or a session file.
func TestPluginsStayOffKernel(t *testing.T) {
	files := []string{
		"plugin.go", "host.go", "shell.go",
		"plugins_tree.go", "plugins_timeline.go", "plugins_tools.go",
	}
	fset := token.NewFileSet()
	for _, name := range files {
		file, err := parser.ParseFile(fset, name, nil, parser.ImportsOnly)
		if err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		for _, spec := range file.Imports {
			path := strings.Trim(spec.Path.Value, `"`)
			if strings.Contains(path, "/internal/kernel") || strings.Contains(path, "/pi/") {
				t.Errorf("%s imports %s", name, path)
			}
		}
	}
}

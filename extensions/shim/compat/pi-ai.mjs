// Compatibility surface for extensions written against pi's package layout.
// These modules stand in for the upstream packages an extension imports; they
// are bundled into the package by tools/extension-build. Members exist so a
// real extension can load and run; terminal rendering is inert because the
// desktop never calls the TUI renderers.

export function StringEnum(values, options = {}) {
  return { type: "string", enum: [...values], ...options };
}

export function getModel() {
  return undefined;
}

export function getSupportedModels() {
  return [];
}

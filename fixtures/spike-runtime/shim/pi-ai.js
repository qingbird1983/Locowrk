import "./runtime.js";
// Stand-in for @earendil-works/pi-ai (only the helpers examples import).

export function StringEnum(values, options) {
  return { type: "string", enum: [...values], ...(options || {}) };
}

export function getModel() {
  return undefined;
}


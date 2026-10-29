// Stand-in for typebox: extensions build JSON Schema for tool parameters. The
// kernel validates tool arguments against the produced schema, so these
// builders emit plain JSON Schema instead of typebox's symbolic types.

function schema(type, extra = {}) {
  return { type, ...extra };
}

function withDescription(extra) {
  const out = {};
  if (extra && typeof extra === "object") {
    if (extra.description) out.description = extra.description;
    if (Array.isArray(extra.enum)) out.enum = extra.enum;
    if (extra.default !== undefined) out.default = extra.default;
    if (extra.minimum !== undefined) out.minimum = extra.minimum;
    if (extra.maximum !== undefined) out.maximum = extra.maximum;
    if (extra.minLength !== undefined) out.minLength = extra.minLength;
    if (extra.maxLength !== undefined) out.maxLength = extra.maxLength;
    if (extra.pattern) out.pattern = extra.pattern;
    if (Array.isArray(extra.required)) out.required = extra.required;
  }
  return out;
}

export const Type = {
  Object(properties = {}, extra = {}) {
    const required = Array.isArray(extra.required) ? extra.required : [];
    const withoutRequired = { ...extra };
    delete withoutRequired.required;
    return schema("object", { properties: properties ?? {}, ...(required.length ? { required } : {}), ...withoutRequired });
  },
  String(extra) {
    return schema("string", withDescription(extra));
  },
  Number(extra) {
    return schema("number", withDescription(extra));
  },
  Integer(extra) {
    return schema("integer", withDescription(extra));
  },
  Boolean(extra) {
    return schema("boolean", withDescription(extra));
  },
  Array(items, extra = {}) {
    return schema("array", { items, ...withDescription(extra) });
  },
  Record(value, extra = {}) {
    return schema("object", { additionalProperties: value, ...withDescription(extra) });
  },
  Any(extra) {
    return { ...withDescription(extra) };
  },
  Union(values, extra = {}) {
    return { anyOf: values, ...withDescription(extra) };
  },
  Literal(value, extra = {}) {
    return { const: value, ...withDescription(extra) };
  },
  Optional(value) {
    // JSON Schema marks optionality on the parent's "required" list; keep the
    // marker so callers that introspect it still see the intent.
    return { ...value, optional: true };
  },
};

export const Static = undefined;

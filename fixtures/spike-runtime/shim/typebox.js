import "./runtime.js";
// Stand-in for typebox. Schemas are returned as plain descriptions; the spike
// host does not validate tool arguments, it only checks that registration works.

function schema(type, extra) {
  return { type, ...(extra || {}) };
}

export const Type = {
  Object(properties, extra) {
    return schema("object", { properties: properties || {}, ...(extra || {}) });
  },
  String(extra) {
    return schema("string", extra);
  },
  Number(extra) {
    return schema("number", extra);
  },
  Boolean(extra) {
    return schema("boolean", extra);
  },
  Array(items, extra) {
    return schema("array", { items, ...(extra || {}) });
  },
  Optional(value) {
    return { ...value, optional: true };
  },
  Union(values) {
    return schema("union", { anyOf: values });
  },
};

export const Static = undefined;


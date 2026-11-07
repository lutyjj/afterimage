import openapi from "../../../../gen/management-api.openapi.json";

/** A JSON Schema node, as the OpenAPI document generated from the model writes it. */
export type Schema = {
  $ref?: string;
  type?: string;
  title?: string;
  description?: string;
  default?: unknown;
  properties?: Record<string, Schema>;
  required?: string[];
  additionalProperties?: Schema | boolean;
  items?: Schema;
  oneOf?: Schema[];
  allOf?: Schema[];
  enum?: unknown[];
};

const schemas = openapi.components.schemas as unknown as Record<string, Schema>;

/** Every object with named members refuses others: the client's serializer drops
 * members the model does not have, so the editor must flag them rather than let
 * them vanish on save. */
function closed(schema: unknown): unknown {
  if (Array.isArray(schema)) return schema.map(closed);
  if (schema && typeof schema === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(schema)) {
      out[key] =
        key === "properties"
          ? Object.fromEntries(Object.entries(value as object).map(([name, member]) => [name, closed(member)]))
          : closed(value);
    }
    if ("properties" in out && !("additionalProperties" in out)) out.additionalProperties = false;
    return out;
  }
  return schema;
}

/** The schema of a screen document, for the editor's completion, hover and lint. */
export const documentSchema = {
  $ref: "#/components/schemas/ScreenDocument",
  components: { schemas: closed(schemas) },
};

function resolve(schema: Schema | undefined): Schema | undefined {
  if (!schema) return undefined;
  if (schema.$ref) return resolve(schemas[schema.$ref.split("/").pop() ?? ""]);
  if (schema.allOf) {
    return schema.allOf.reduce<Schema>((merged, part) => ({ ...merged, ...resolve(part) }), {
      ...schema,
      allOf: undefined,
    });
  }
  return schema;
}

function defaultOf(schema: Schema | undefined): unknown {
  if (!schema) return undefined;
  if (schema.default !== undefined) return schema.default;
  return schema.allOf?.find((part) => part.default !== undefined)?.default;
}

/** The document without the members that equal their defaults, which the server
 * fills in when it stores a document: what the person wrote, give or take. */
export function withoutDefaults(
  value: unknown,
  schema: Schema | undefined = { $ref: "#/components/schemas/ScreenDocument" },
): unknown {
  const resolved = resolve(schema);
  if (!resolved) return value;
  if (Array.isArray(value)) return value.map((item) => withoutDefaults(item, resolved.items));
  if (!value || typeof value !== "object") return value;
  const object = value as Record<string, unknown>;
  if (resolved.oneOf) {
    const key = Object.keys(object)[0];
    const variant = resolved.oneOf.find((candidate) => key && candidate.properties?.[key]);
    return variant ? withoutDefaults(object, variant) : value;
  }
  const out: Record<string, unknown> = {};
  for (const [key, member] of Object.entries(object)) {
    const memberSchema =
      resolved.properties?.[key] ??
      (typeof resolved.additionalProperties === "object" ? resolved.additionalProperties : undefined);
    const fallback = defaultOf(memberSchema);
    if (fallback !== undefined && JSON.stringify(fallback) === JSON.stringify(member)) continue;
    out[key] = withoutDefaults(member, memberSchema);
  }
  return out;
}

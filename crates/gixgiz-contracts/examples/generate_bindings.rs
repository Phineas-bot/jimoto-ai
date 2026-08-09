//! Generates the machine-readable transport schema and checked Dart bindings.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fmt::{self, Write as _},
    fs,
    path::Path,
};

use gixgiz_contracts::{
    BootstrapReady, BootstrapRequest, CancelOperationRequest, CancelOperationResponse, ClientHello,
    CoreHello, HardwareScanEvent, HardwareScanStartRequest, HardwareScanStartResponse,
    HealthRequest, HealthResponse, RecommendationRequest, RecommendationResponse, SafeErrorPayload,
    ShutdownRequest, ShutdownResponse, TestOperationEvent, TestOperationStartRequest,
    TestOperationStartResponse,
};
use schemars::{JsonSchema, schema_for};
use serde_json::{Map, Value};

#[derive(JsonSchema)]
#[allow(dead_code)]
struct TransportContractDocument {
    bootstrap_request: BootstrapRequest,
    bootstrap_ready: BootstrapReady,
    client_hello: ClientHello,
    core_hello: CoreHello,
    health_request: HealthRequest,
    health_response: HealthResponse,
    hardware_scan_start_request: HardwareScanStartRequest,
    hardware_scan_start_response: HardwareScanStartResponse,
    hardware_scan_event: HardwareScanEvent,
    recommendation_request: RecommendationRequest,
    recommendation_response: RecommendationResponse,
    test_operation_start_request: TestOperationStartRequest,
    test_operation_start_response: TestOperationStartResponse,
    test_operation_event: TestOperationEvent,
    cancel_operation_request: CancelOperationRequest,
    cancel_operation_response: CancelOperationResponse,
    shutdown_request: ShutdownRequest,
    shutdown_response: ShutdownResponse,
    safe_error_payload: SafeErrorPayload,
}

#[derive(Debug)]
struct GenerationError(String);

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for GenerationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Write,
    Check,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DefinitionKind {
    Alias,
    Enum,
    Object,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = match env::args().nth(1).as_deref() {
        Some("--write") => Mode::Write,
        Some("--check") => Mode::Check,
        _ => {
            return Err(Box::new(GenerationError(
                "usage: cargo run -p gixgiz-contracts --example generate_bindings -- --write|--check"
                    .to_owned(),
            )));
        }
    };

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| GenerationError("contracts crate is not inside the workspace".to_owned()))?;
    let schema_path = workspace.join("schemas/gixgiz-transport.schema.json");
    let dart_path = workspace.join("apps/desktop/lib/core/generated/core_contracts.g.dart");

    let schema = serde_json::to_value(schema_for!(TransportContractDocument))?;
    let schema_output = format!("{}\n", serde_json::to_string_pretty(&schema)?);
    let dart_output = render_dart(&schema)?;

    match mode {
        Mode::Write => {
            write_generated(&schema_path, &schema_output)?;
            write_generated(&dart_path, &dart_output)?;
            println!("updated {}", schema_path.display());
            println!("updated {}", dart_path.display());
        }
        Mode::Check => {
            check_generated(&schema_path, &schema_output)?;
            check_generated(&dart_path, &dart_output)?;
            println!("transport schema and Dart bindings are current");
        }
    }

    Ok(())
}

fn write_generated(path: &Path, content: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn check_generated(path: &Path, expected: &str) -> Result<(), Box<dyn Error>> {
    let actual = fs::read_to_string(path).map_err(|error| {
        GenerationError(format!(
            "{} is missing or unreadable: {error}; run the generator with --write",
            path.display()
        ))
    })?;
    if actual != expected {
        return Err(Box::new(GenerationError(format!(
            "{} is stale; run the generator with --write",
            path.display()
        ))));
    }
    Ok(())
}

fn render_dart(schema: &Value) -> Result<String, GenerationError> {
    let definitions = schema
        .get("$defs")
        .and_then(Value::as_object)
        .ok_or_else(|| GenerationError("generated schema has no $defs object".to_owned()))?;
    let kinds = definition_kinds(definitions)?;
    let mut names = definitions.keys().collect::<Vec<_>>();
    names.sort_unstable();

    let mut output = String::from(
        "// GENERATED CODE - DO NOT MODIFY BY HAND.\n\
         // Source: crates/gixgiz-contracts and schemas/gixgiz-transport.schema.json\n\
         // Regenerate: cargo run -p gixgiz-contracts --example generate_bindings -- --write\n\n\
         // ignore_for_file: unnecessary_lambdas\n\n",
    );

    for name in names {
        let definition = definitions
            .get(name)
            .ok_or_else(|| GenerationError(format!("missing definition {name}")))?;
        match kinds
            .get(name)
            .ok_or_else(|| GenerationError(format!("missing kind for {name}")))?
        {
            DefinitionKind::Alias => render_alias(&mut output, name, definition)?,
            DefinitionKind::Enum => render_enum(&mut output, name, definition)?,
            DefinitionKind::Object => {
                render_object(&mut output, name, definition, definitions, &kinds)?;
            }
        }
    }

    output.push_str(
        "Map<String, dynamic> _contractMap(Object? value, String path) {\n\
         \x20 if (value is Map<String, dynamic>) {\n\
         \x20   return value;\n\
         \x20 }\n\
         \x20 throw FormatException('$path must be a JSON object.');\n\
         }\n\n\
         List<dynamic> _contractList(Object? value, String path) {\n\
         \x20 if (value is List<dynamic>) {\n\
         \x20   return value;\n\
         \x20 }\n\
         \x20 throw FormatException('$path must be a JSON array.');\n\
         }\n\n\
         String _contractString(Object? value, String path) {\n\
         \x20 if (value is String) {\n\
         \x20   return value;\n\
         \x20 }\n\
         \x20 throw FormatException('$path must be a string.');\n\
         }\n\n\
         int _contractInt(Object? value, String path) {\n\
         \x20 if (value is int) {\n\
         \x20   return value;\n\
         \x20 }\n\
         \x20 throw FormatException('$path must be an integer.');\n\
         }\n\n\
         bool _contractBool(Object? value, String path) {\n\
         \x20 if (value is bool) {\n\
         \x20   return value;\n\
         \x20 }\n\
         \x20 throw FormatException('$path must be a boolean.');\n\
         }\n",
    );
    Ok(output)
}

fn definition_kinds(
    definitions: &Map<String, Value>,
) -> Result<BTreeMap<String, DefinitionKind>, GenerationError> {
    definitions
        .iter()
        .map(|(name, definition)| {
            let kind = if enum_values(definition).is_some() {
                DefinitionKind::Enum
            } else if has_type(definition, "object") {
                DefinitionKind::Object
            } else if has_type(definition, "string") {
                DefinitionKind::Alias
            } else {
                return Err(GenerationError(format!(
                    "unsupported definition shape for {name}: {definition}"
                )));
            };
            Ok((name.clone(), kind))
        })
        .collect()
}

fn render_alias(
    output: &mut String,
    name: &str,
    definition: &Value,
) -> Result<(), GenerationError> {
    if !has_type(definition, "string") {
        return Err(GenerationError(format!(
            "only string aliases are supported for {name}"
        )));
    }
    writeln!(output, "typedef {name} = String;\n").map_err(write_error)
}

fn render_enum(output: &mut String, name: &str, definition: &Value) -> Result<(), GenerationError> {
    let values = enum_values(definition)
        .ok_or_else(|| GenerationError(format!("enum {name} has no values")))?;
    writeln!(output, "enum {name} {{").map_err(write_error)?;
    for wire in &values {
        writeln!(output, "  {}('{wire}'),", lower_camel(wire)).map_err(write_error)?;
    }
    output.push_str(
        "  ;\n\n\
         \x20 const ",
    );
    write!(output, "{name}(this.wireValue);\n\n").map_err(write_error)?;
    output.push_str("  final String wireValue;\n\n");
    writeln!(output, "  static {name} fromJson(Object? value) {{").map_err(write_error)?;
    output.push_str(
        "    for (final candidate in values) {\n\
         \x20     if (candidate.wireValue == value) {\n\
         \x20       return candidate;\n\
         \x20     }\n\
         \x20   }\n",
    );
    if values.contains(&"unknown") {
        output.push_str("    return unknown;\n");
    } else {
        writeln!(
            output,
            "    throw FormatException('Unknown {name} value: $value');"
        )
        .map_err(write_error)?;
    }
    output.push_str("  }\n\n  String toJson() => wireValue;\n}\n\n");
    Ok(())
}

fn render_object(
    output: &mut String,
    name: &str,
    definition: &Value,
    definitions: &Map<String, Value>,
    kinds: &BTreeMap<String, DefinitionKind>,
) -> Result<(), GenerationError> {
    let properties = definition
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| GenerationError(format!("object {name} has no properties")))?;
    let required = definition
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    writeln!(output, "class {name} {{").map_err(write_error)?;
    writeln!(output, "  const {name}({{").map_err(write_error)?;
    for wire_name in properties.keys() {
        writeln!(output, "    required this.{},", lower_camel(wire_name)).map_err(write_error)?;
    }
    output.push_str("  });\n\n");

    for (wire_name, property) in properties {
        let field_name = lower_camel(wire_name);
        let field_type = dart_type(property, definitions, kinds)?;
        let nullable = !required.contains(wire_name) || is_nullable(property);
        writeln!(
            output,
            "  final {field_type}{} {field_name};",
            if nullable { "?" } else { "" }
        )
        .map_err(write_error)?;
    }

    writeln!(
        output,
        "\n  factory {name}.fromJson(Map<String, dynamic> json) {{"
    )
    .map_err(write_error)?;
    writeln!(output, "    return {name}(").map_err(write_error)?;
    for (wire_name, property) in properties {
        let field_name = lower_camel(wire_name);
        let path = format!("{name}.{wire_name}");
        let raw = format!("json['{wire_name}']");
        let nullable = !required.contains(wire_name) || is_nullable(property);
        let schema = non_null_schema(property);
        let decoded = decode_expression(schema, &raw, &path, kinds)?;
        if nullable {
            writeln!(
                output,
                "      {field_name}: {raw} == null ? null : {decoded},"
            )
            .map_err(write_error)?;
        } else {
            writeln!(output, "      {field_name}: {decoded},").map_err(write_error)?;
        }
    }
    output.push_str("    );\n  }\n\n");

    output.push_str("  Map<String, dynamic> toJson() {\n    return {\n");
    for (wire_name, property) in properties {
        let field_name = lower_camel(wire_name);
        let nullable = !required.contains(wire_name) || is_nullable(property);
        if nullable {
            let encoded =
                encode_nullable_expression(non_null_schema(property), &field_name, kinds)?;
            writeln!(output, "      '{wire_name}': {encoded},").map_err(write_error)?;
        } else {
            let encoded = encode_expression(non_null_schema(property), &field_name, kinds)?;
            writeln!(output, "      '{wire_name}': {encoded},").map_err(write_error)?;
        }
    }
    output.push_str("    };\n  }\n}\n\n");
    Ok(())
}

fn dart_type(
    schema: &Value,
    definitions: &Map<String, Value>,
    kinds: &BTreeMap<String, DefinitionKind>,
) -> Result<String, GenerationError> {
    let schema = non_null_schema(schema);
    if let Some(reference) = reference_name(schema) {
        if !definitions.contains_key(reference) || !kinds.contains_key(reference) {
            return Err(GenerationError(format!(
                "unknown schema reference {reference}"
            )));
        }
        return Ok(reference.to_owned());
    }
    if has_type(schema, "string") {
        return Ok("String".to_owned());
    }
    if has_type(schema, "integer") {
        return Ok("int".to_owned());
    }
    if has_type(schema, "boolean") {
        return Ok("bool".to_owned());
    }
    if has_type(schema, "array") {
        let items = schema
            .get("items")
            .ok_or_else(|| GenerationError("array schema has no items".to_owned()))?;
        return Ok(format!("List<{}>", dart_type(items, definitions, kinds)?));
    }
    Err(GenerationError(format!(
        "unsupported Dart property schema: {schema}"
    )))
}

fn decode_expression(
    schema: &Value,
    raw: &str,
    path: &str,
    kinds: &BTreeMap<String, DefinitionKind>,
) -> Result<String, GenerationError> {
    if let Some(reference) = reference_name(schema) {
        return match kinds.get(reference) {
            Some(DefinitionKind::Alias) => Ok(format!("_contractString({raw}, '{path}')")),
            Some(DefinitionKind::Enum) => Ok(format!("{reference}.fromJson({raw})")),
            Some(DefinitionKind::Object) => Ok(format!(
                "{reference}.fromJson(_contractMap({raw}, '{path}'))"
            )),
            None => Err(GenerationError(format!(
                "unknown schema reference {reference}"
            ))),
        };
    }
    if has_type(schema, "string") {
        return Ok(format!("_contractString({raw}, '{path}')"));
    }
    if has_type(schema, "integer") {
        return Ok(format!("_contractInt({raw}, '{path}')"));
    }
    if has_type(schema, "boolean") {
        return Ok(format!("_contractBool({raw}, '{path}')"));
    }
    if has_type(schema, "array") {
        let items = schema
            .get("items")
            .ok_or_else(|| GenerationError("array schema has no items".to_owned()))?;
        let item = decode_expression(items, "item", &format!("{path}[]"), kinds)?;
        return Ok(format!(
            "_contractList({raw}, '{path}').map((item) => {item}).toList(growable: false)"
        ));
    }
    Err(GenerationError(format!(
        "unsupported decode schema: {schema}"
    )))
}

fn encode_expression(
    schema: &Value,
    value: &str,
    kinds: &BTreeMap<String, DefinitionKind>,
) -> Result<String, GenerationError> {
    if let Some(reference) = reference_name(schema) {
        return match kinds.get(reference) {
            Some(DefinitionKind::Alias) => Ok(value.to_owned()),
            Some(DefinitionKind::Enum | DefinitionKind::Object) => Ok(format!("{value}.toJson()")),
            None => Err(GenerationError(format!(
                "unknown schema reference {reference}"
            ))),
        };
    }
    if has_type(schema, "string") || has_type(schema, "integer") || has_type(schema, "boolean") {
        return Ok(value.to_owned());
    }
    if has_type(schema, "array") {
        let items = schema
            .get("items")
            .ok_or_else(|| GenerationError("array schema has no items".to_owned()))?;
        let item = encode_expression(items, "item", kinds)?;
        return Ok(format!(
            "{value}.map((item) => {item}).toList(growable: false)"
        ));
    }
    Err(GenerationError(format!(
        "unsupported encode schema: {schema}"
    )))
}

fn encode_nullable_expression(
    schema: &Value,
    value: &str,
    kinds: &BTreeMap<String, DefinitionKind>,
) -> Result<String, GenerationError> {
    if let Some(reference) = reference_name(schema) {
        return match kinds.get(reference) {
            Some(DefinitionKind::Alias) => Ok(value.to_owned()),
            Some(DefinitionKind::Enum | DefinitionKind::Object) => Ok(format!("{value}?.toJson()")),
            None => Err(GenerationError(format!(
                "unknown schema reference {reference}"
            ))),
        };
    }
    if has_type(schema, "string") || has_type(schema, "integer") || has_type(schema, "boolean") {
        return Ok(value.to_owned());
    }
    if has_type(schema, "array") {
        let items = schema
            .get("items")
            .ok_or_else(|| GenerationError("array schema has no items".to_owned()))?;
        let item = encode_expression(items, "item", kinds)?;
        return Ok(format!(
            "{value}?.map((item) => {item}).toList(growable: false)"
        ));
    }
    Err(GenerationError(format!(
        "unsupported nullable encode schema: {schema}"
    )))
}

fn non_null_schema(schema: &Value) -> &Value {
    schema
        .get("anyOf")
        .and_then(Value::as_array)
        .and_then(|variants| variants.iter().find(|variant| !has_type(variant, "null")))
        .unwrap_or(schema)
}

fn is_nullable(schema: &Value) -> bool {
    schema
        .get("anyOf")
        .and_then(Value::as_array)
        .is_some_and(|variants| variants.iter().any(|variant| has_type(variant, "null")))
        || schema
            .get("type")
            .and_then(Value::as_array)
            .is_some_and(|types| types.iter().any(|value| value.as_str() == Some("null")))
}

fn has_type(schema: &Value, expected: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(value)) => value == expected,
        Some(Value::Array(values)) => values.iter().any(|value| value.as_str() == Some(expected)),
        _ => false,
    }
}

fn reference_name(schema: &Value) -> Option<&str> {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
}

fn enum_values(schema: &Value) -> Option<Vec<&str>> {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values.iter().map(Value::as_str).collect();
    }

    schema
        .get("oneOf")
        .and_then(Value::as_array)
        .filter(|variants| !variants.is_empty())
        .and_then(|variants| {
            variants
                .iter()
                .map(|variant| variant.get("const").and_then(Value::as_str))
                .collect()
        })
}

fn lower_camel(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut uppercase_next = false;
    for character in value.chars() {
        if character == '_' || character == '-' {
            uppercase_next = true;
        } else if uppercase_next {
            output.extend(character.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(character);
        }
    }
    output
}

fn write_error(_: fmt::Error) -> GenerationError {
    GenerationError("failed to render generated Dart source".to_owned())
}

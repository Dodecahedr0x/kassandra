use serde_json::Value;

/// The structured-output JSON Schema forcing `{ "option_index": <integer> }`.
///
/// Stable identity is pinned by [`crate::hashing::OUTPUT_SCHEMA_ID`] /
/// [`crate::hashing::OUTPUT_SCHEMA_VERSION`]. The schema constrains
/// `option_index` to an integer in `[0, options_count)` via `minimum`/`maximum`,
/// requires the field, and forbids any other field (`additionalProperties:
/// false`) so the provider returns a clean, parse-robust answer. The only
/// input-dependent value is `maximum` (= `options_count - 1`), derived from the
/// on-chain `options_count`; the schema's SHAPE is what the version pins.
pub fn output_schema(options_count: u8) -> Value {
    let max_index = options_count.saturating_sub(1);
    serde_json::json!({
        "type": "object",
        "properties": {
            "option_index": {
                "type": "integer",
                "minimum": 0,
                "maximum": max_index
            }
        },
        "required": ["option_index"],
        "additionalProperties": false
    })
}

/// Error parsing a model's structured-output response into an `option_index`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    /// The raw response was not valid JSON.
    #[error("response is not valid JSON: {0}")]
    InvalidJson(String),
    /// The JSON was valid but not a JSON object.
    #[error("response JSON is not an object")]
    NotAnObject,
    /// The required `option_index` field was absent.
    #[error("response is missing the required `option_index` field")]
    MissingField,
    /// `option_index` was present but not a non-negative integer (it was a
    /// float, string, negative, boolean, etc.).
    #[error("`option_index` must be a non-negative integer, got `{0}`")]
    NotAnUnsignedInteger(String),
    /// `option_index` is a valid integer but outside `[0, options_count)`.
    #[error("`option_index` {got} is out of range: must be 0..{count} ({count} options)")]
    OutOfRange {
        /// The parsed (in-range-for-u64) index that failed the bound check.
        got: u64,
        /// The number of options (`option_index` must be `< count`).
        count: u8,
    },
}

/// Parse a model's raw structured-output JSON string into a validated
/// `option_index`.
///
/// Accepts the verbatim `raw_response` (the JSON the provider returned) and the
/// oracle's `options_count`. Returns the chosen index on success, or a
/// [`ParseError`] if the response is malformed, missing the field, the wrong
/// type, negative, or out of range (`index >= options_count`).
///
/// **Extra-field policy: lenient.** Only `option_index` is read; any additional
/// fields are ignored (a schema-compliant provider sends none thanks to
/// `additionalProperties: false`, but a stray field never breaks parsing).
pub fn parse_option_index(raw_response: &str, options_count: u8) -> Result<u8, ParseError> {
    let value: Value =
        serde_json::from_str(raw_response).map_err(|e| ParseError::InvalidJson(e.to_string()))?;

    let obj = value.as_object().ok_or(ParseError::NotAnObject)?;

    let field = obj.get("option_index").ok_or(ParseError::MissingField)?;

    // `as_u64` is true ONLY for a JSON integer >= 0; it rejects floats (incl.
    // `1.0`), negatives, strings, booleans, and null — exactly what we want.
    let raw = field
        .as_u64()
        .ok_or_else(|| ParseError::NotAnUnsignedInteger(field.to_string()))?;

    if raw >= options_count as u64 {
        return Err(ParseError::OutOfRange {
            got: raw,
            count: options_count,
        });
    }

    // raw < options_count <= u8::MAX, so this never truncates.
    Ok(raw as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- output schema ------------------------------------------------------

    #[test]
    fn output_schema_shape() {
        let schema = output_schema(3);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["option_index"]["type"], "integer");
        assert_eq!(schema["properties"]["option_index"]["minimum"], 0);
        assert_eq!(schema["properties"]["option_index"]["maximum"], 2);
        assert_eq!(schema["required"], serde_json::json!(["option_index"]));
        assert_eq!(schema["additionalProperties"], false);
    }

    // --- parsing ------------------------------------------------------------

    #[test]
    fn parse_valid_index() {
        assert_eq!(parse_option_index(r#"{"option_index":1}"#, 2).unwrap(), 1);
        assert_eq!(parse_option_index(r#"{"option_index":0}"#, 2).unwrap(), 0);
    }

    #[test]
    fn parse_extra_fields_are_ignored() {
        // Lenient policy: stray fields don't break parsing.
        let raw = r#"{"option_index":2,"reasoning":"because","confidence":0.9}"#;
        assert_eq!(parse_option_index(raw, 3).unwrap(), 2);
    }

    #[test]
    fn parse_rejects_out_of_range() {
        assert_eq!(
            parse_option_index(r#"{"option_index":2}"#, 2),
            Err(ParseError::OutOfRange { got: 2, count: 2 })
        );
        assert_eq!(
            parse_option_index(r#"{"option_index":255}"#, 3),
            Err(ParseError::OutOfRange { got: 255, count: 3 })
        );
    }

    #[test]
    fn parse_rejects_negative() {
        assert!(matches!(
            parse_option_index(r#"{"option_index":-1}"#, 2),
            Err(ParseError::NotAnUnsignedInteger(_))
        ));
    }

    #[test]
    fn parse_rejects_float() {
        assert!(matches!(
            parse_option_index(r#"{"option_index":1.5}"#, 3),
            Err(ParseError::NotAnUnsignedInteger(_))
        ));
        // Even an integer-valued float is rejected (wrong JSON type).
        assert!(matches!(
            parse_option_index(r#"{"option_index":1.0}"#, 3),
            Err(ParseError::NotAnUnsignedInteger(_))
        ));
    }

    #[test]
    fn parse_rejects_wrong_type() {
        assert!(matches!(
            parse_option_index(r#"{"option_index":"1"}"#, 3),
            Err(ParseError::NotAnUnsignedInteger(_))
        ));
        assert!(matches!(
            parse_option_index(r#"{"option_index":true}"#, 3),
            Err(ParseError::NotAnUnsignedInteger(_))
        ));
        assert!(matches!(
            parse_option_index(r#"{"option_index":null}"#, 3),
            Err(ParseError::NotAnUnsignedInteger(_))
        ));
    }

    #[test]
    fn parse_rejects_missing_field() {
        assert_eq!(
            parse_option_index(r#"{"other":1}"#, 3),
            Err(ParseError::MissingField)
        );
    }

    #[test]
    fn parse_rejects_non_object() {
        assert_eq!(parse_option_index("1", 3), Err(ParseError::NotAnObject));
        assert_eq!(parse_option_index("[1,2]", 3), Err(ParseError::NotAnObject));
    }

    #[test]
    fn parse_rejects_malformed_json() {
        assert!(matches!(
            parse_option_index(r#"{"option_index":}"#, 3),
            Err(ParseError::InvalidJson(_))
        ));
        assert!(matches!(
            parse_option_index("not json", 3),
            Err(ParseError::InvalidJson(_))
        ));
    }
}

//! The Cloudflare D1 REST API: request bodies, response envelopes, value mapping.
//!
//! D1 is queried through the ordinary Cloudflare API:
//! `POST /accounts/{account}/d1/database/{database}/query` with `{sql, params}`, and
//! a Cloudflare envelope back — `{success, errors, messages, result}` — carrying one
//! entry per statement.
//!
//! Reference: <https://developers.cloudflare.com/api/resources/d1/>

use std::collections::HashMap;

use ruprizzle::value::Value;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::D1Error;

/// The body of a `/query` request.
#[derive(Debug, Serialize)]
pub(crate) struct QueryRequest {
    pub sql: String,
    pub params: Vec<Json>,
}

/// The Cloudflare API envelope.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<ApiError>,
    /// `null` on failure, which is why this is an `Option` rather than a defaulted
    /// `Vec`: serde will not read `null` as an empty sequence.
    #[serde(default)]
    pub result: Option<Vec<QueryResult>>,
}

/// One entry of the Cloudflare `errors` array.
#[derive(Debug, Deserialize)]
pub(crate) struct ApiError {
    #[serde(default)]
    pub code: Option<i64>,
    #[serde(default)]
    pub message: String,
}

/// The outcome of one statement.
#[derive(Debug, Deserialize)]
pub(crate) struct QueryResult {
    #[serde(default)]
    pub results: Vec<HashMap<String, Json>>,
    #[serde(default)]
    pub meta: Meta,
}

/// The per-statement metadata D1 reports.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Meta {
    /// Rows the statement changed. Absent on reads.
    #[serde(default)]
    pub changes: u64,
}

impl Envelope {
    /// Reads the single statement result, or the error the API reported.
    ///
    /// # Errors
    ///
    /// Returns [`D1Error::Query`] when Cloudflare reported failure, and
    /// [`D1Error::Protocol`] when it reported success without a result.
    pub(crate) fn single_result(self) -> Result<QueryResult, D1Error> {
        if !self.success {
            let message = if self.errors.is_empty() {
                "the D1 API reported failure without an error message".to_owned()
            } else {
                self.errors
                    .iter()
                    .map(|e| match e.code {
                        Some(code) => format!("{} ({code})", e.message),
                        None => e.message.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            return Err(D1Error::Query(message));
        }

        self.result
            .unwrap_or_default()
            .into_iter()
            .next()
            .ok_or_else(|| {
                D1Error::Protocol("the D1 API reported success but returned no result".into())
            })
    }
}

impl QueryResult {
    /// Converts the JSON rows into the map-per-row shape [`RowBatch::Edge`] uses.
    ///
    /// [`RowBatch::Edge`]: ruprizzle::executor::RowBatch::Edge
    pub(crate) fn into_edge_rows(self) -> Vec<HashMap<String, Value>> {
        self.results
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(name, value)| (name, from_json(value)))
                    .collect()
            })
            .collect()
    }
}

/// Encodes a bound parameter.
///
/// D1's HTTP interface accepts only JSON scalars as parameters, so values with no
/// scalar form are refused rather than silently reshaped into something the database
/// would store differently from what the caller meant.
///
/// # Errors
///
/// Returns [`D1Error::Unsupported`] for byte and array parameters.
pub(crate) fn to_json(value: &Value) -> Result<Json, D1Error> {
    Ok(match value {
        Value::Null => Json::Null,
        // D1 rejects a JSON boolean parameter; SQLite stores 1 and 0, and the
        // generated SQL compares against those.
        Value::Bool(b) => Json::from(i64::from(*b)),
        Value::I32(i) => Json::from(*i),
        Value::I64(i) => Json::from(*i),
        Value::F64(f) => serde_json::Number::from_f64(*f).map_or(Json::Null, Json::Number),
        Value::Decimal(d) => Json::from(d.to_string()),
        Value::Str(s) => Json::from(s.to_string()),
        Value::Uuid(u) => Json::from(u.to_string()),
        Value::DateTime(dt) => Json::from(dt.to_rfc3339()),
        Value::Date(d) => Json::from(d.to_string()),
        Value::Time(t) => Json::from(t.to_string()),
        Value::Json(j) => Json::from(j.to_string()),
        Value::Bytes(_) => {
            return Err(D1Error::Unsupported(
                "D1's HTTP API takes no binary parameter; store the value hex- or \
                 base64-encoded in a text column"
                    .into(),
            ));
        }
        Value::Array(_) => {
            return Err(D1Error::Unsupported(
                "SQLite has no array type, so an array parameter cannot be bound".into(),
            ));
        }
    })
}

/// Decodes a value D1 returned.
///
/// D1 answers in JSON, so a column's SQL type is not recoverable from the response:
/// an integer column and a text column holding digits are told apart by JSON type
/// alone. Objects and arrays keep their JSON form rather than being flattened.
pub(crate) fn from_json(value: Json) -> Value {
    match value {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(b),
        Json::Number(n) => n
            .as_i64()
            .map(Value::I64)
            .or_else(|| n.as_f64().map(Value::F64))
            .unwrap_or_else(|| Value::Str(n.to_string().into())),
        Json::String(s) => Value::Str(s.into()),
        other @ (Json::Array(_) | Json::Object(_)) => Value::Json(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_set_becomes_one_map_per_row() {
        let body = r#"{"success":true,"errors":[],"messages":[],"result":[{
            "results":[{"id":7,"email":"a@b.c","score":1.5,"note":null}],
            "success":true,
            "meta":{"changes":0,"last_row_id":0}
        }]}"#;
        let envelope: Envelope = serde_json::from_str(body).unwrap();
        let rows = envelope.single_result().unwrap().into_edge_rows();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id"), Some(&Value::I64(7)));
        assert_eq!(rows[0].get("email"), Some(&Value::Str("a@b.c".into())));
        assert_eq!(rows[0].get("score"), Some(&Value::F64(1.5)));
        assert_eq!(rows[0].get("note"), Some(&Value::Null));
    }

    #[test]
    fn a_reported_failure_carries_cloudflares_message_and_code() {
        let body = r#"{"success":false,
            "errors":[{"code":7500,"message":"no such table: users"}],
            "messages":[],"result":null}"#;
        let envelope: Envelope = serde_json::from_str(body).unwrap();
        let err = envelope.single_result().unwrap_err();
        match err {
            D1Error::Query(message) => {
                assert!(message.contains("no such table: users"), "{message}");
                assert!(message.contains("7500"), "{message}");
            }
            other => panic!("expected a query error, got {other:?}"),
        }
    }

    #[test]
    fn success_without_a_result_is_a_protocol_error_not_an_empty_row_set() {
        let body = r#"{"success":true,"errors":[],"messages":[],"result":[]}"#;
        let envelope: Envelope = serde_json::from_str(body).unwrap();
        assert!(matches!(
            envelope.single_result().unwrap_err(),
            D1Error::Protocol(_)
        ));
    }

    #[test]
    fn the_affected_row_count_comes_from_the_metadata() {
        let body = r#"{"success":true,"errors":[],"messages":[],"result":[{
            "results":[],"success":true,"meta":{"changes":3}}]}"#;
        let envelope: Envelope = serde_json::from_str(body).unwrap();
        assert_eq!(envelope.single_result().unwrap().meta.changes, 3);
    }

    #[test]
    fn booleans_bind_as_sqlite_stores_them() {
        assert_eq!(to_json(&Value::Bool(true)).unwrap(), Json::from(1));
    }

    #[test]
    fn parameters_with_no_scalar_form_are_refused() {
        for value in [
            Value::Bytes(vec![1, 2, 3].into()),
            Value::Array(vec![Value::I64(1)]),
        ] {
            assert!(
                matches!(to_json(&value), Err(D1Error::Unsupported(_))),
                "{value:?} should be refused"
            );
        }
    }
}

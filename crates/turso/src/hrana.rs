//! The Hrana 2 wire protocol, over HTTP.
//!
//! Turso and any libSQL server expose `POST {base}/v2/pipeline`, which takes a list
//! of requests and returns one result per request. A single `execute` followed by a
//! `close` is a complete, stateless round trip, which is all this adapter needs: it
//! holds no server-side session, so it has no baton to carry and nothing to clean up
//! if a request is dropped.
//!
//! Reference: <https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_3_SPEC.md>
//! (the `/v2/pipeline` endpoint is the Hrana 2 subset of that document).

use std::collections::HashMap;

use base64::Engine as _;
use ruprizzle::value::Value;
use serde::{Deserialize, Serialize};

use crate::TursoError;

/// A statement and its positional arguments.
#[derive(Debug, Serialize)]
pub(crate) struct Stmt {
    pub sql: String,
    pub args: Vec<HranaValue>,
    /// Whether the server should send rows back. `false` for `execute_raw`, which
    /// only needs the affected-row count.
    pub want_rows: bool,
}

/// One step of a pipeline.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Request {
    Execute { stmt: Stmt },
    Close,
}

/// The `/v2/pipeline` request body.
#[derive(Debug, Serialize)]
pub(crate) struct Pipeline {
    /// Always `null`: this adapter opens no server-side stream to resume.
    pub baton: Option<String>,
    pub requests: Vec<Request>,
}

impl Pipeline {
    /// One statement, executed and then closed.
    pub(crate) fn one(stmt: Stmt) -> Self {
        Self {
            baton: None,
            requests: vec![Request::Execute { stmt }, Request::Close],
        }
    }
}

/// A SQL value as Hrana encodes it.
///
/// Integers travel as **strings** because JSON numbers cannot carry the full range
/// of `i64` without precision loss in every client that parses them as doubles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum HranaValue {
    Null,
    Integer { value: String },
    Float { value: f64 },
    Text { value: String },
    Blob { base64: String },
}

/// The `/v2/pipeline` response body.
#[derive(Debug, Deserialize)]
pub(crate) struct PipelineResponse {
    #[serde(default)]
    pub results: Vec<StepResult>,
}

/// The outcome of one pipeline step.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum StepResult {
    Ok {
        response: StepResponse,
    },
    Error {
        error: HranaError,
    },
    /// A step the server skipped because an earlier one failed.
    #[serde(other)]
    None,
}

/// The payload of a successful step.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum StepResponse {
    Execute {
        result: ExecuteResult,
    },
    /// `close`, and anything a newer server adds that we do not read.
    #[serde(other)]
    Other,
}

/// The result of one `execute`.
#[derive(Debug, Deserialize)]
pub(crate) struct ExecuteResult {
    #[serde(default)]
    pub cols: Vec<Col>,
    #[serde(default)]
    pub rows: Vec<Vec<HranaValue>>,
    #[serde(default)]
    pub affected_row_count: u64,
}

/// One column of a result set.
#[derive(Debug, Deserialize)]
pub(crate) struct Col {
    #[serde(default)]
    pub name: Option<String>,
}

/// An error reported by the server for one step.
#[derive(Debug, Deserialize)]
pub(crate) struct HranaError {
    pub message: String,
    #[serde(default)]
    pub code: Option<String>,
}

impl ExecuteResult {
    /// Names the columns, falling back to `column_N` for expressions the server
    /// declines to name.
    ///
    /// The fallback matters: `SELECT count(*)` returns an unnamed column on some
    /// libSQL builds, and a row keyed by the empty string is unreadable.
    pub(crate) fn column_names(&self) -> Vec<String> {
        self.cols
            .iter()
            .enumerate()
            .map(|(i, c)| match c.name.as_deref() {
                Some(name) if !name.is_empty() => name.to_owned(),
                _ => format!("column_{i}"),
            })
            .collect()
    }

    /// Turns the row-major result into the map-per-row shape [`RowBatch::Edge`] uses.
    ///
    /// [`RowBatch::Edge`]: ruprizzle::executor::RowBatch::Edge
    pub(crate) fn into_edge_rows(self) -> Result<Vec<HashMap<String, Value>>, TursoError> {
        let names = self.column_names();
        self.rows
            .into_iter()
            .map(|row| {
                if row.len() != names.len() {
                    return Err(TursoError::Protocol(format!(
                        "server sent a row of {} value(s) for {} column(s)",
                        row.len(),
                        names.len()
                    )));
                }
                names
                    .iter()
                    .cloned()
                    .zip(row)
                    .map(|(name, value)| Ok((name, from_hrana(value)?)))
                    .collect()
            })
            .collect()
    }
}

/// Encodes a bound parameter for the wire.
///
/// # Errors
///
/// Returns [`TursoError::Unsupported`] for values `SQLite` has no column type for.
pub(crate) fn to_hrana(value: &Value) -> Result<HranaValue, TursoError> {
    Ok(match value {
        Value::Null => HranaValue::Null,
        // SQLite has no boolean type; it stores 1 and 0, and every comparison in
        // generated SQL is written against those.
        Value::Bool(b) => HranaValue::Integer {
            value: i64::from(*b).to_string(),
        },
        Value::I32(i) => HranaValue::Integer {
            value: i.to_string(),
        },
        Value::I64(i) => HranaValue::Integer {
            value: i.to_string(),
        },
        Value::F64(f) => HranaValue::Float { value: *f },
        Value::Decimal(d) => HranaValue::Text {
            value: d.to_string(),
        },
        Value::Str(s) => HranaValue::Text {
            value: s.to_string(),
        },
        Value::Uuid(u) => HranaValue::Text {
            value: u.to_string(),
        },
        Value::DateTime(dt) => HranaValue::Text {
            value: dt.to_rfc3339(),
        },
        Value::Date(d) => HranaValue::Text {
            value: d.to_string(),
        },
        Value::Time(t) => HranaValue::Text {
            value: t.to_string(),
        },
        Value::Json(j) => HranaValue::Text {
            value: j.to_string(),
        },
        Value::Bytes(b) => HranaValue::Blob {
            base64: base64::engine::general_purpose::STANDARD.encode(b.as_ref()),
        },
        Value::Array(_) => {
            return Err(TursoError::Unsupported(
                "SQLite has no array type, so an array parameter cannot be bound".into(),
            ));
        }
    })
}

/// Decodes a value the server sent.
///
/// # Errors
///
/// Returns [`TursoError::Protocol`] when the server sends an integer that is not one
/// or a blob that is not base64 — both of which mean the response cannot be trusted,
/// and neither of which has a safe substitute value.
pub(crate) fn from_hrana(value: HranaValue) -> Result<Value, TursoError> {
    Ok(match value {
        HranaValue::Null => Value::Null,
        HranaValue::Integer { value } => Value::I64(value.parse::<i64>().map_err(|e| {
            TursoError::Protocol(format!("server sent `{value}` as an integer: {e}"))
        })?),
        HranaValue::Float { value } => Value::F64(value),
        HranaValue::Text { value } => Value::Str(value.into()),
        HranaValue::Blob { base64 } => Value::Bytes(
            base64::engine::general_purpose::STANDARD
                .decode(&base64)
                .map_err(|e| TursoError::Protocol(format!("server sent an invalid blob: {e}")))?
                .into(),
        ),
    })
}

/// Reads the single `execute` result out of a pipeline response.
///
/// # Errors
///
/// Returns [`TursoError::Query`] when the server rejected the statement, and
/// [`TursoError::Protocol`] when the response does not have the shape a pipeline of
/// one `execute` must produce.
pub(crate) fn single_execute(response: PipelineResponse) -> Result<ExecuteResult, TursoError> {
    let first = response.results.into_iter().next().ok_or_else(|| {
        TursoError::Protocol("server returned no result for the executed statement".into())
    })?;

    match first {
        StepResult::Ok {
            response: StepResponse::Execute { result },
        } => Ok(result),
        StepResult::Ok {
            response: StepResponse::Other,
        } => Err(TursoError::Protocol(
            "server answered an `execute` request with a different response type".into(),
        )),
        StepResult::Error { error } => Err(TursoError::Query {
            message: error.message,
            code: error.code,
        }),
        StepResult::None => Err(TursoError::Protocol(
            "server skipped the executed statement".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_travel_as_strings_and_come_back_whole() {
        // The reason the wire format uses strings: this value is not representable
        // as an f64, so a client that parsed JSON numbers would corrupt it.
        let big = i64::MAX;
        let encoded = to_hrana(&Value::I64(big)).unwrap();
        assert_eq!(
            encoded,
            HranaValue::Integer {
                value: big.to_string()
            }
        );
        assert_eq!(from_hrana(encoded).unwrap(), Value::I64(big));
    }

    #[test]
    fn bytes_round_trip_through_base64() {
        let bytes: &[u8] = &[0, 1, 254, 255];
        let encoded = to_hrana(&Value::Bytes(bytes.into())).unwrap();
        assert_eq!(from_hrana(encoded).unwrap(), Value::Bytes(bytes.into()));
    }

    #[test]
    fn booleans_bind_as_sqlite_stores_them() {
        assert_eq!(
            to_hrana(&Value::Bool(true)).unwrap(),
            HranaValue::Integer {
                value: "1".to_owned()
            }
        );
    }

    #[test]
    fn an_array_parameter_is_refused_rather_than_flattened() {
        let err = to_hrana(&Value::Array(vec![Value::I64(1)])).unwrap_err();
        assert!(matches!(err, TursoError::Unsupported(_)), "got {err:?}");
    }

    #[test]
    fn a_corrupt_integer_is_an_error_not_a_zero() {
        let err = from_hrana(HranaValue::Integer {
            value: "not a number".to_owned(),
        })
        .unwrap_err();
        assert!(matches!(err, TursoError::Protocol(_)), "got {err:?}");
    }

    #[test]
    fn unnamed_columns_get_positional_names() {
        let result = ExecuteResult {
            cols: vec![
                Col { name: None },
                Col {
                    name: Some(String::new()),
                },
            ],
            rows: Vec::new(),
            affected_row_count: 0,
        };
        assert_eq!(result.column_names(), vec!["column_0", "column_1"]);
    }

    #[test]
    fn a_statement_error_surfaces_as_a_query_error() {
        let body = r#"{"baton":null,"results":[
            {"type":"error","error":{"message":"no such table: users","code":"SQLITE_ERROR"}}
        ]}"#;
        let parsed: PipelineResponse = serde_json::from_str(body).unwrap();
        let err = single_execute(parsed).unwrap_err();
        match err {
            TursoError::Query { message, code } => {
                assert_eq!(message, "no such table: users");
                assert_eq!(code.as_deref(), Some("SQLITE_ERROR"));
            }
            other => panic!("expected a query error, got {other:?}"),
        }
    }

    #[test]
    fn a_result_set_becomes_one_map_per_row() {
        let body = r#"{"baton":null,"results":[
            {"type":"ok","response":{"type":"execute","result":{
                "cols":[{"name":"id"},{"name":"email"}],
                "rows":[[{"type":"integer","value":"7"},{"type":"text","value":"a@b.c"}],
                        [{"type":"integer","value":"8"},{"type":"null"}]],
                "affected_row_count":0
            }}},
            {"type":"ok","response":{"type":"close"}}
        ]}"#;
        let parsed: PipelineResponse = serde_json::from_str(body).unwrap();
        let rows = single_execute(parsed).unwrap().into_edge_rows().unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("id"), Some(&Value::I64(7)));
        assert_eq!(rows[0].get("email"), Some(&Value::Str("a@b.c".into())));
        assert_eq!(rows[1].get("email"), Some(&Value::Null));
    }
}

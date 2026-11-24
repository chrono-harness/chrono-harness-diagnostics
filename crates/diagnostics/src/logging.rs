//! Structured tracing records and their explicit sink.
#[cfg(not(verus_keep_ghost))]
use crate::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {

pub(crate) enum ReservedFailure<V, E> {
    Field(&'static str, V),
    Decode(E),
}

pub(crate) open spec fn text_or_default<V>(input: Option<Result<String, V>>, fallback: &str, text: String) -> bool {
    match input {
        None => text@ == fallback@,
        Some(Ok(value)) => text == value,
        Some(Err(_)) => false,
    }
}

/// Select the first field error and interpret error JSON only after the text fields succeed.
pub(crate) fn resolve_reserved<V, R, E, P>(
    identity: Option<Result<String, V>>, message: Option<Result<String, V>>,
    error: Option<Result<String, V>>, parse: P,
) -> (out: Result<(String, String, Option<R>), ReservedFailure<V, E>>)
    where P: FnOnce(String) -> Result<R, E>,
    requires forall|raw: String| parse.requires((raw,)),
    ensures
        match (identity, message, error) {
            (Some(Err(value)), _, _) => out == Err::<(String, String, Option<R>), _>(ReservedFailure::<V, E>::Field("event_id", value)),
            (_, Some(Err(value)), _) => out == Err::<(String, String, Option<R>), _>(ReservedFailure::<V, E>::Field("message", value)),
            (_, _, Some(Err(value))) => out == Err::<(String, String, Option<R>), _>(ReservedFailure::<V, E>::Field("chrono_error", value)),
            (id, msg, raw) => match out {
                Ok((i, m, r)) => text_or_default(id, "tracing.external", i) && text_or_default(msg, "", m)
                    && match (raw, r) {
                        (None, None) => true,
                        (Some(Ok(text)), Some(record)) => parse.ensures((text,), Ok::<R, E>(record)),
                        _ => false,
                    },
                Err(ReservedFailure::Decode(failure)) => match raw {
                    Some(Ok(text)) => parse.ensures((text,), Err::<R, E>(failure)),
                    _ => false,
                },
                _ => false,
            },
        },
{
    let id = match identity {
        None => "tracing.external".to_string(),
        Some(Ok(text)) => text,
        Some(Err(value)) => return Err(ReservedFailure::Field("event_id", value)),
    };
    let msg = match message {
        None => "".to_string(),
        Some(Ok(text)) => text,
        Some(Err(value)) => return Err(ReservedFailure::Field("message", value)),
    };
    let record = match error {
        None => None,
        Some(Err(value)) => return Err(ReservedFailure::Field("chrono_error", value)),
        Some(Ok(text)) => match parse(text) {
            Ok(parsed) => Some(parsed),
            Err(failure) => return Err(ReservedFailure::Decode(failure)),
        },
    };
    Ok((id, msg, record))
}

}

#[cfg(not(verus_keep_ghost))]
use crate::error::{Context, ErrorNode, ErrorRecord, RecordedError, io_error_node};
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Map, Value};
#[cfg(not(verus_keep_ghost))]
use std::error::Error;
#[cfg(not(verus_keep_ghost))]
use std::fmt;
#[cfg(not(verus_keep_ghost))]
use std::io::{self, Write};
#[cfg(not(verus_keep_ghost))]
use std::sync::{Arc, Mutex};
#[cfg(not(verus_keep_ghost))]
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
#[cfg(not(verus_keep_ghost))]
pub use tracing::Level;
#[cfg(not(verus_keep_ghost))]
use tracing::{
    Dispatch, Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
#[cfg(not(verus_keep_ghost))]
use tracing_subscriber::{
    Layer, Registry, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg(not(verus_keep_ghost))]
pub enum LogSchema {
    #[serde(rename = "chrono-log/v1")]
    V1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg(not(verus_keep_ghost))]
pub struct LogRecord {
    pub schema: LogSchema,
    pub timestamp: String,
    pub level: String,
    pub producer: String,
    pub event: String,
    pub message: String,
    pub context: Context,
    pub fields: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorRecord>,
}

#[derive(Debug)]
#[cfg(not(verus_keep_ghost))]
pub enum LogIoPhase {
    Write,
    Flush,
}

#[derive(Debug)]
#[cfg(not(verus_keep_ghost))]
pub enum LogFailure {
    Io {
        source: io::Error,
        phase: LogIoPhase,
        attempted_record: Option<Vec<u8>>,
    },
    Json {
        source: serde_json::Error,
        observed_fields: Map<String, Value>,
    },
    Time(time::error::Format),
    InvalidField {
        field: &'static str,
        observed: Value,
    },
    Blocked {
        source: Arc<LogFailure>,
        attempted_record: Vec<u8>,
    },
    Closed {
        attempted_record: Vec<u8>,
    },
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for LogFailure {
    fn error_node(&self) -> ErrorNode {
        let (code, source, context) = match self {
            Self::Io {
                source,
                phase,
                attempted_record,
            } => {
                let mut context = Context::new();
                context.insert(
                    "phase".into(),
                    match phase {
                        LogIoPhase::Write => "write",
                        LogIoPhase::Flush => "flush",
                    }
                    .into(),
                );
                if let Some(bytes) = attempted_record {
                    context.insert("attempted_record".into(), bytes.clone().into());
                }
                ("E_LOG_IO", Some(Box::new(io_error_node(source))), context)
            }
            Self::Json {
                source,
                observed_fields,
            } => {
                let mut context = Context::new();
                context.insert("observed_fields".into(), observed_fields.clone().into());
                ("E_LOG_JSON", Some(Box::new(source.error_node())), context)
            }
            Self::Time(source) => (
                "E_LOG_TIME",
                Some(Box::new(ErrorNode::new(
                    std::any::type_name::<time::error::Format>().into(),
                    "native".into(),
                    source.to_string(),
                    Context::new(),
                ))),
                Context::new(),
            ),
            Self::InvalidField { field, observed } => {
                let mut context = Context::new();
                context.insert("field".into(), (*field).into());
                context.insert("observed".into(), observed.clone());
                ("E_LOG_FIELD", None, context)
            }
            Self::Blocked {
                source,
                attempted_record,
            } => (
                "E_LOG_BLOCKED",
                Some(Box::new(source.error_node())),
                [("attempted_record".into(), attempted_record.clone().into())]
                    .into_iter()
                    .collect(),
            ),
            Self::Closed { attempted_record } => (
                "E_LOG_CLOSED",
                None,
                [("attempted_record".into(), attempted_record.clone().into())]
                    .into_iter()
                    .collect(),
            ),
        };
        ErrorNode::assemble(
            std::any::type_name::<Self>().into(),
            code.into(),
            self.to_string(),
            context,
            source,
            None,
            None,
            Vec::new(),
        )
    }
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for Arc<LogFailure> {
    fn error_node(&self) -> ErrorNode {
        self.as_ref().error_node()
    }
}

#[cfg(not(verus_keep_ghost))]
impl fmt::Display for LogFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { source, .. } => write!(f, "log sink failed: {source}"),
            Self::Json { source, .. } => write!(f, "log record JSON failed: {source}"),
            Self::Time(source) => write!(f, "log timestamp failed: {source}"),
            Self::InvalidField { field, .. } => write!(f, "invalid structured log field {field}"),
            Self::Blocked { .. } => {
                f.write_str("log sink is unavailable after its original failure")
            }
            Self::Closed { .. } => f.write_str("log event arrived after the sink was finalized"),
        }
    }
}

#[cfg(not(verus_keep_ghost))]
impl Error for LogFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Time(source) => Some(source),
            Self::InvalidField { .. } => None,
            Self::Blocked { source, .. } => Some(source.as_ref()),
            Self::Closed { .. } => None,
        }
    }
}

#[derive(Default)]
#[cfg(not(verus_keep_ghost))]
struct Fields(Map<String, Value>);

#[cfg(not(verus_keep_ghost))]
impl Visit for Fields {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.insert(
            field.name().into(),
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(value.to_string())),
        );
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_i128(&mut self, field: &Field, value: i128) {
        let value = serde_json::Number::from_i128(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string()));
        self.0.insert(field.name().into(), value);
    }
    fn record_u128(&mut self, field: &Field, value: u128) {
        let value = serde_json::Number::from_u128(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string()));
        self.0.insert(field.name().into(), value);
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0
            .insert(field.name().into(), format!("{value:?}").into());
    }
}

#[cfg(not(verus_keep_ghost))]
struct Sink<W> {
    writer: W,
    failures: Vec<Arc<LogFailure>>,
    stopped: Option<Arc<LogFailure>>,
    finished: bool,
}

#[cfg(not(verus_keep_ghost))]
struct LogLayer<W> {
    producer: String,
    max_level: Level,
    sink: Arc<Mutex<Sink<W>>>,
}

#[cfg(not(verus_keep_ghost))]
fn take_reserved(
    fields: &mut Map<String, Value>,
    key: &'static str,
) -> Option<Result<String, Value>> {
    fields.remove(key).map(|value| match value {
        Value::String(text) => Ok(text),
        other => Err(other),
    })
}

#[cfg(not(verus_keep_ghost))]
impl<W: Write> LogLayer<W> {
    fn publish(&self, record: Result<LogRecord, LogFailure>) {
        let mut sink = self.sink.lock().expect("diagnostic sink mutex");
        let result = record.and_then(|record| {
            let mut bytes = serde_json::to_vec(&record).map_err(|source| LogFailure::Json {
                source,
                observed_fields: record.fields,
            })?;
            bytes.push(b'\n');
            if sink.finished {
                return Err(LogFailure::Closed {
                    attempted_record: bytes,
                });
            }
            if let Some(source) = &sink.stopped {
                return Err(LogFailure::Blocked {
                    source: source.clone(),
                    attempted_record: bytes,
                });
            }
            let result = match sink.writer.write_all(&bytes) {
                Err(source) => Err((LogIoPhase::Write, source)),
                Ok(()) => sink
                    .writer
                    .flush()
                    .map_err(|source| (LogIoPhase::Flush, source)),
            };
            result.map_err(|(phase, source)| LogFailure::Io {
                source,
                phase,
                attempted_record: Some(bytes),
            })
        });
        if let Err(error) = result {
            let error = Arc::new(error);
            if matches!(error.as_ref(), LogFailure::Io { .. }) {
                sink.stopped = Some(error.clone());
            }
            sink.failures.push(error);
        }
    }
}

#[cfg(not(verus_keep_ghost))]
impl<S, W> Layer<S> for LogLayer<W>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    W: Write + Send + 'static,
{
    fn enabled(&self, metadata: &Metadata<'_>, _context: LayerContext<'_, S>) -> bool {
        metadata.is_span() || *metadata.level() <= self.max_level
    }

    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, context: LayerContext<'_, S>) {
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        if let Some(span) = context.span(id) {
            span.extensions_mut().insert(fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, context: LayerContext<'_, S>) {
        if let Some(span) = context.span(id) {
            if let Some(fields) = span.extensions_mut().get_mut::<Fields>() {
                values.record(fields);
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, context: LayerContext<'_, S>) {
        // A replaying dispatcher can submit events directly without calling enabled.
        if *event.metadata().level() > self.max_level {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let mut inherited = Context::new();
        if let Some(scope) = context.event_scope(event) {
            for span in scope.from_root() {
                if let Some(fields) = span.extensions().get::<Fields>() {
                    inherited.extend(fields.0.clone());
                }
            }
        }
        self.publish((|| {
            let (identity, message, error) = resolve_reserved(
                take_reserved(&mut fields.0, "event_id"),
                take_reserved(&mut fields.0, "message"),
                take_reserved(&mut fields.0, "chrono_error"),
                |raw| {
                    serde_json::from_str(&raw).map_err(|source| LogFailure::Json {
                        source,
                        observed_fields: Map::from_iter([("chrono_error".into(), raw.into())]),
                    })
                },
            )
            .map_err(|failure| match failure {
                ReservedFailure::Field(field, observed) => {
                    LogFailure::InvalidField { field, observed }
                }
                ReservedFailure::Decode(error) => error,
            })?;
            fields
                .0
                .insert("target".into(), event.metadata().target().into());
            Ok(LogRecord {
                schema: LogSchema::V1,
                timestamp: OffsetDateTime::now_utc()
                    .format(&Rfc3339)
                    .map_err(LogFailure::Time)?,
                level: event.metadata().level().as_str().to_ascii_lowercase(),
                producer: self.producer.clone(),
                event: identity,
                message,
                context: inherited,
                fields: fields.0,
                error,
            })
        })());
    }
}

/// Keep the logger until all instrumented work joins, then inspect finish().
/// A dispatcher clone is the explicit handoff for another thread or task.
#[cfg(not(verus_keep_ghost))]
pub struct Logger<W> {
    dispatch: Dispatch,
    // tracing-core 0.1.36's single-dispatch fast path registers new callsites
    // against the calling thread, which may have no scoped subscriber. Keep
    // two registrations of the same subscriber alive so registration consults
    // the live subscribers even on unrelated, uninstrumented threads. Neither
    // registration changes the thread's default dispatcher or duplicates events.
    _callsite_dispatch: Dispatch,
    sink: Arc<Mutex<Sink<W>>>,
}

#[cfg(not(verus_keep_ghost))]
impl<W: Write + Send + 'static> Logger<W> {
    pub fn new(producer: impl Into<String>, max_level: Level, writer: W) -> Self {
        let sink = Arc::new(Mutex::new(Sink {
            writer,
            failures: Vec::new(),
            stopped: None,
            finished: false,
        }));
        let layer = LogLayer {
            producer: producer.into(),
            max_level,
            sink: sink.clone(),
        };
        let subscriber = Arc::new(Registry::default().with(layer));
        Self {
            dispatch: Dispatch::new(subscriber.clone()),
            _callsite_dispatch: Dispatch::new(subscriber),
            sink,
        }
    }

    pub fn dispatch(&self) -> Dispatch {
        self.dispatch.clone()
    }

    pub fn run<T>(&self, work: impl FnOnce() -> T) -> T {
        tracing::dispatcher::with_default(&self.dispatch, work)
    }

    /// Retain every original failure; repeated inspection cannot erase one.
    pub fn finish(&self) -> Result<(), Vec<Arc<LogFailure>>> {
        let mut sink = self.sink.lock().expect("diagnostic sink mutex");
        if !sink.finished {
            sink.finished = true;
            if sink.stopped.is_none() {
                if let Err(source) = sink.writer.flush() {
                    let failure = Arc::new(LogFailure::Io {
                        source,
                        phase: LogIoPhase::Flush,
                        attempted_record: None,
                    });
                    sink.stopped = Some(failure.clone());
                    sink.failures.push(failure);
                }
            }
        }
        if sink.failures.is_empty() {
            Ok(())
        } else {
            Err(sink.failures.clone())
        }
    }
}

#[cfg(not(verus_keep_ghost))]
impl Logger<io::Stderr> {
    pub fn stderr(producer: impl Into<String>) -> Self {
        Self::new(producer, Level::INFO, io::stderr())
    }
}

/// Publish an already structured domain failure through the same tracing layer.
#[cfg(not(verus_keep_ghost))]
pub fn failure(event: &str, message: &str, record: &ErrorRecord) -> Result<(), serde_json::Error> {
    record_error(Level::ERROR, event, message, record)
}

/// Recovery and handling events keep their original error at the actual event level.
#[cfg(not(verus_keep_ghost))]
pub fn record_error(
    level: Level,
    event: &str,
    message: &str,
    record: &ErrorRecord,
) -> Result<(), serde_json::Error> {
    let encoded = serde_json::to_string(record)?;
    macro_rules! emit {
        ($level:expr) => {
            tracing::event!(target: "chrono", $level, event_id = event, message = message, chrono_error = encoded.as_str())
        };
    }
    match level {
        Level::ERROR => emit!(Level::ERROR),
        Level::WARN => emit!(Level::WARN),
        Level::INFO => emit!(Level::INFO),
        Level::DEBUG => emit!(Level::DEBUG),
        Level::TRACE => emit!(Level::TRACE),
    }
    Ok(())
}

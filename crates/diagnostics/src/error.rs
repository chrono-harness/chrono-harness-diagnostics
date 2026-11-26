pub mod cause {
    #[cfg(not(verus_keep_ghost))]
    use crate::verus;
    #[cfg(verus_keep_ghost)]
    use vstd::prelude::*;

    verus! {
    pub enum IoCause<A, C, J> {
        Application(A),
        Channel(C),
        Serialization(J),
        Opaque,
        None,
    }

    /// std::io supplies the presence flag and the checked downcasts of its single source.
    pub fn select_io_cause<A, C, J>(present: bool, application: Option<A>, channel: Option<C>,
        serialization: Option<J>, custom_writer: Option<bool>) -> (result: IoCause<A, C, J>)
        ensures result == match application {
            Some(x) => IoCause::Application(x),
            None => match channel {
                Some(x) => IoCause::Channel(x),
                None => match serialization {
                    Some(x) => IoCause::Serialization(x),
                    None => if present && custom_writer != Some(false) { IoCause::Opaque } else { IoCause::None },
                },
            },
        },
    {
        if let Some(x) = application { return IoCause::Application(x); }
        if let Some(x) = channel { return IoCause::Channel(x); }
        if let Some(x) = serialization { return IoCause::Serialization(x); }
        if present && custom_writer != Some(false) { IoCause::Opaque } else { IoCause::None }
    }
    }
}

#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::Value;
#[cfg(not(verus_keep_ghost))]
use std::collections::BTreeMap;
#[cfg(not(verus_keep_ghost))]
use std::error::Error as StdError;
#[cfg(not(verus_keep_ghost))]
use std::fmt;
#[cfg(not(verus_keep_ghost))]
use std::io;

#[cfg(not(verus_keep_ghost))]
pub type Context = BTreeMap<String, Value>;

#[cfg(not(verus_keep_ghost))]
pub type ErrorNode = crate::verified_error::ErrorNode<Context>;
#[cfg(not(verus_keep_ghost))]
pub type RelatedError = crate::verified_error::RelatedError<Context>;

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ErrorSchema {
    #[serde(rename = "chrono-error/v1")]
    V1,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorRecord {
    pub schema: ErrorSchema,
    pub error: ErrorNode,
}

/// An error and the record produced by its concrete owner travel together.
/// This trait deliberately does not guess a concrete type from erased Display text.
#[cfg(not(verus_keep_ghost))]
pub trait RecordedError: StdError + Send + Sync + 'static {
    fn error_node(&self) -> ErrorNode;
}

/// Adopts a native IO error without replacing its typed value with Display text.
#[cfg(not(verus_keep_ghost))]
#[derive(Debug)]
pub struct NativeIoError {
    source: io::Error,
}

#[cfg(not(verus_keep_ghost))]
impl NativeIoError {
    pub fn new(source: io::Error) -> Self {
        Self { source }
    }

    pub fn as_io_error(&self) -> &io::Error {
        &self.source
    }

    pub fn into_io_error(self) -> io::Error {
        self.source
    }
}

#[cfg(not(verus_keep_ghost))]
impl fmt::Display for NativeIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

#[cfg(not(verus_keep_ghost))]
impl StdError for NativeIoError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.source)
    }
}

/// OS errors can be reconstructed by code. A caller-created IO error carries
/// its original kind and presence of an opaque source across the writer boundary.
#[cfg(not(verus_keep_ghost))]
#[derive(Debug)]
pub(crate) struct WriterError {
    pub kind: String,
    pub message: String,
    pub custom_source: bool,
}

#[cfg(not(verus_keep_ghost))]
impl fmt::Display for WriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
#[cfg(not(verus_keep_ghost))]
impl StdError for WriterError {}

#[cfg(not(verus_keep_ghost))]
fn io_kind(error: &io::Error) -> String {
    error
        .get_ref()
        .and_then(|source| source.downcast_ref::<WriterError>())
        .map(|source| source.kind.clone())
        .unwrap_or_else(|| format!("{:?}", error.kind()))
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for tracing_tunnel::ReceiveError {
    fn error_node(&self) -> ErrorNode {
        use tracing_tunnel::ReceiveError::*;
        let mut context = Context::new();
        let code = match self {
            UnknownMetadataId(id) => {
                context.insert("metadata_id".into(), (*id).into());
                "tracing::UnknownMetadataId"
            }
            UnknownSpanId(id) => {
                context.insert("span_id".into(), (*id).into());
                "tracing::UnknownSpanId"
            }
            TooManyValues { max, actual } => {
                context.insert("max".into(), (*max).into());
                context.insert("actual".into(), (*actual).into());
                "tracing::TooManyValues"
            }
            _ => "native",
        };
        ErrorNode::new(
            std::any::type_name::<Self>().into(),
            code.into(),
            self.to_string(),
            context,
        )
    }
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for serde_json::Error {
    fn error_node(&self) -> ErrorNode {
        let context = [
            ("category".into(), format!("{:?}", self.classify()).into()),
            ("line".into(), self.line().into()),
            ("column".into(), self.column().into()),
        ]
        .into_iter()
        .collect();
        ErrorNode::new(
            std::any::type_name::<Self>().into(),
            "native".into(),
            self.to_string(),
            context,
        )
    }
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn io_error_node(error: &io::Error) -> ErrorNode {
    let mut context = Context::new();
    let kind = io_kind(error);
    context.insert("kind".into(), kind.clone().into());
    let source = error.get_ref();
    let causes = cause::select_io_cause(
        source.is_some(),
        source.and_then(|s| s.downcast_ref::<Failure>()),
        source.and_then(|s| s.downcast_ref::<tracing_tunnel::ReceiveError>()),
        source.and_then(|s| s.downcast_ref::<serde_json::Error>()),
        source
            .and_then(|s| s.downcast_ref::<WriterError>())
            .map(|w| w.custom_source),
    );
    use cause::IoCause;
    let recorded = match causes {
        IoCause::Application(source) => Some(source.error_node()),
        IoCause::Channel(source) => Some(source.error_node()),
        IoCause::Serialization(source) => Some(source.error_node()),
        IoCause::Opaque | IoCause::None => None,
    };
    if matches!(causes, IoCause::Opaque) {
        context.insert("custom_source".into(), "not_recorded".into());
    }
    ErrorNode::assemble(
        "std::io::Error".into(),
        format!("io::{kind}"),
        error.to_string(),
        context,
        recorded.map(Box::new),
        error.raw_os_error().map(i64::from),
        None,
        Vec::new(),
    )
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for NativeIoError {
    fn error_node(&self) -> ErrorNode {
        ErrorNode::assemble(
            std::any::type_name::<Self>().into(),
            "E_NATIVE_IO".into(),
            self.to_string(),
            [("kind".into(), io_kind(&self.source).into())]
                .into_iter()
                .collect(),
            Some(Box::new(io_error_node(&self.source))),
            self.source.raw_os_error().map(i64::from),
            None,
            Vec::new(),
        )
    }
}

#[cfg(not(verus_keep_ghost))]
#[derive(Debug)]
pub struct Failure {
    node: ErrorNode,
    source: Option<Box<dyn RecordedError>>,
    related: Vec<(String, Box<dyn RecordedError>)>,
}

#[cfg(not(verus_keep_ghost))]
impl Failure {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        let node = ErrorNode::new(
            std::any::type_name::<Self>().into(),
            code.into(),
            message.into(),
            Context::new(),
        );
        let (node, source, related) = crate::verified_error::initialize(node);
        Self {
            node,
            source,
            related,
        }
    }

    pub fn at(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.node.context.insert(name.into(), value.into());
        self
    }

    pub fn wrap(
        code: impl Into<String>,
        message: impl Into<String>,
        source: impl RecordedError,
    ) -> Self {
        let Self {
            node,
            source: previous,
            related,
        } = Self::new(code, message);
        let source: Box<dyn RecordedError> = Box::new(source);
        let (node, source, related) =
            crate::verified_error::set_source((node, previous, related), source);
        Self {
            node,
            source,
            related,
        }
    }

    pub fn with_related(self, role: impl Into<String>, error: impl RecordedError) -> Self {
        let Self {
            node,
            source,
            related,
        } = self;
        let role = role.into();
        let error: Box<dyn RecordedError> = Box::new(error);
        let (node, source, related) =
            crate::verified_error::append_related((node, source, related), role, error);
        Self {
            node,
            source,
            related,
        }
    }

    pub fn record(&self) -> ErrorRecord {
        ErrorRecord {
            schema: ErrorSchema::V1,
            error: self.error_node(),
        }
    }

    pub fn related(&self) -> impl Iterator<Item = (&str, &dyn RecordedError)> {
        self.related
            .iter()
            .map(|(role, error)| (role.as_str(), error.as_ref()))
    }
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for Failure {
    fn error_node(&self) -> ErrorNode {
        let node = self.node.clone();
        let source = self
            .source
            .as_ref()
            .map(|source| Box::new(source.error_node()));
        let related = self
            .related
            .iter()
            .map(|(role, error)| RelatedError::new(role.clone(), error.error_node()))
            .collect();
        node.with_causes(source, related)
    }
}

#[cfg(not(verus_keep_ghost))]
impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.node.message)
    }
}

#[cfg(not(verus_keep_ghost))]
impl StdError for Failure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &dyn StdError)
    }
}

/// A received record remains a typed cause; importing it adds no claim of local execution.
#[cfg(not(verus_keep_ghost))]
#[derive(Debug)]
pub struct ReceivedError {
    node: ErrorNode,
    source: Option<Box<ReceivedError>>,
}

#[cfg(not(verus_keep_ghost))]
impl From<ErrorRecord> for ReceivedError {
    fn from(record: ErrorRecord) -> Self {
        Self::from_node(record.error)
    }
}

#[cfg(not(verus_keep_ghost))]
impl ReceivedError {
    fn from_node(node: ErrorNode) -> Self {
        let (node, source) = node.detach_source();
        let source = source.map(|source| Box::new(Self::from_node(*source)));
        Self { node, source }
    }
}

#[cfg(not(verus_keep_ghost))]
impl fmt::Display for ReceivedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.node.message)
    }
}

#[cfg(not(verus_keep_ghost))]
impl StdError for ReceivedError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &dyn StdError)
    }
}

#[cfg(not(verus_keep_ghost))]
impl RecordedError for ReceivedError {
    fn error_node(&self) -> ErrorNode {
        let node = self.node.clone();
        let source = self
            .source
            .as_ref()
            .map(|source| Box::new(source.error_node()));
        node.with_source(source)
    }
}

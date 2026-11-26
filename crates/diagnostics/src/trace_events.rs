//! Replay an installed producer's tracing stream through the host's runtime logger.
//! The explicit writer is acknowledged by its owner so its original failure reaches Logger.
#[cfg(not(verus_keep_ghost))]
use crate::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// The JSON reply to exactly one write or flush on the producer's sink.
#[cfg_attr(not(verus_keep_ghost), derive(serde::Deserialize))]
#[cfg_attr(not(verus_keep_ghost), serde(rename_all = "snake_case", deny_unknown_fields))]
pub enum Acknowledgment {
    Ok,
    Error { native_code: Option<i32>, kind: String, custom_source: bool, message: String },
}

pub fn acknowledged<E, R, N, W>(sent: Result<(), E>, read: R, native: N, writer: W) -> (out: Result<(), E>)
where
    R: FnOnce() -> Result<Acknowledgment, E>, N: FnOnce(i32) -> E, W: FnOnce(String, bool, String) -> E,
    requires read.requires(()), forall|n:i32| native.requires((n,)),
        forall|k:String, c:bool, m:String| writer.requires((k,c,m)),
    ensures match sent {
        Err(e) => out == Err::<(), E>(e),
        Ok(()) => exists|reply: Result<Acknowledgment,E>| #[trigger] read.ensures((),reply) &&
            match reply {
                Err(e) => out == Err::<(),E>(e),
                Ok(Acknowledgment::Ok) => out == Ok::<(),E>(()),
                Ok(Acknowledgment::Error { native_code, kind, custom_source, message }) =>
                    match out {
                        Err(e) => match native_code {
                            Some(n) => native.ensures((n,), e),
                            None => writer.ensures((kind,custom_source,message), e),
                        },
                        Ok(()) => false,
                    }
            }
    },
{
    sent?;
    match read()? {
        Acknowledgment::Ok => Ok(()),
        Acknowledgment::Error { native_code, kind, custom_source, message } => Err(match native_code {
            Some(n) => native(n),
            None => writer(kind, custom_source, message),
        }),
    }
}

/// Keep a stream failure first, followed by every original finish failure in order.
pub fn ordered_errors<T>(stream: Option<T>, mut finished: Vec<T>) -> (out: Vec<T>)
    ensures out@ == match stream { None => finished@, Some(error) => seq![error] + finished@ },
{
    if let Some(error) = stream { finished.insert(0, error); }
    finished
}

/// Only a delivered terminal reply can yield a protocol status.
pub fn delivery_status<T, E>(errors: &[T], delivered: Result<(), E>) -> (out: Result<u8, E>)
    ensures out == match delivered {
        Err(error) => Err::<u8, E>(error),
        Ok(()) => Ok::<u8, E>(if errors.len() == 0 { 0u8 } else { 2u8 }),
    },
{
    match delivered {
        Err(error) => Err(error),
        Ok(()) => Ok(if errors.is_empty() { 0 } else { 2 }),
    }
}
}

#[cfg(not(verus_keep_ghost))]
use crate::{
    error::{ErrorRecord, ErrorSchema, Failure, NativeIoError, RecordedError, WriterError},
    logging::{Level, Logger},
    runtime,
};
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize, de::DeserializeOwned};
#[cfg(not(verus_keep_ghost))]
use std::{
    fs::File,
    io::{self, BufRead, BufWriter, Write},
    os::fd::AsFd,
    path::Path,
    sync::{Arc, Mutex},
};
#[cfg(not(verus_keep_ghost))]
use tracing_tunnel::{TracingEvent, TracingEventReceiver};

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Request {
    Event(TracingEvent),
    Finish,
}

#[cfg(not(verus_keep_ghost))]
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Reply<'a> {
    Ready,
    Accepted,
    Write(&'a str),
    Flush,
    Finished(&'a [ErrorRecord]),
}

#[cfg(not(verus_keep_ghost))]
struct Channel<R, W> {
    input: R,
    output: W,
}

#[cfg(not(verus_keep_ghost))]
impl<R: BufRead, W: Write> Channel<R, W> {
    fn read<T: DeserializeOwned>(&mut self) -> io::Result<T> {
        let mut line = String::new();
        if self.input.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "trace channel closed",
            ));
        }
        serde_json::from_str(&line).map_err(io::Error::other)
    }

    fn send(&mut self, reply: Reply<'_>) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&reply)?;
        bytes.push(b'\n');
        self.output.write_all(&bytes)?;
        self.output.flush()
    }

    fn finish(&mut self, errors: &[ErrorRecord]) -> io::Result<u8> {
        delivery_status(errors, self.send(Reply::Finished(errors)))
    }

    fn publish(&mut self, reply: Reply<'_>) -> io::Result<()> {
        acknowledged(
            self.send(reply),
            || self.read(),
            io::Error::from_raw_os_error,
            |kind, custom_source, message| {
                io::Error::other(WriterError {
                    kind,
                    custom_source,
                    message,
                })
            },
        )
    }
}

#[cfg(not(verus_keep_ghost))]
struct ExplicitSink<R, W>(Arc<Mutex<Channel<R, W>>>);

#[cfg(not(verus_keep_ghost))]
impl<R: BufRead, W: Write> Write for ExplicitSink<R, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let record = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        self.0.lock().unwrap().publish(Reply::Write(record))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().publish(Reply::Flush)
    }
}

#[cfg(not(verus_keep_ghost))]
pub(super) fn receiver() -> TracingEventReceiver {
    TracingEventReceiver::new(Default::default(), Default::default(), Default::default())
}

/// Use the OS writer so a lost stdout is reported even for a read-only descriptor.
#[cfg(not(verus_keep_ghost))]
pub fn serve_to_fd(
    root: Option<&Path>,
    producer: &str,
    input: impl BufRead + Send + 'static,
    output: impl AsFd,
) -> io::Result<u8> {
    let file = File::from(output.as_fd().try_clone_to_owned()?);
    serve(root, producer, input, BufWriter::new(file))
}

#[cfg(not(verus_keep_ghost))]
pub fn serve(
    root: Option<&Path>,
    producer: &str,
    input: impl BufRead + Send + 'static,
    output: impl Write + Send + 'static,
) -> io::Result<u8> {
    let channel = Arc::new(Mutex::new(Channel { input, output }));
    let sink = ExplicitSink(channel.clone());
    let chosen = match root {
        Some(root) => runtime::logger(root, producer, Level::INFO, sink),
        None => Ok(Logger::new(
            producer,
            Level::INFO,
            Box::new(sink) as Box<dyn Write + Send>,
        )),
    };
    let logger = match chosen {
        Ok(logger) => logger,
        Err(error) => return channel.lock().unwrap().finish(&[error.record()]),
    };
    channel.lock().unwrap().send(Reply::Ready)?;
    let stream = logger.run(|| -> io::Result<()> {
        let mut receiver = receiver();
        loop {
            let request = channel.lock().unwrap().read()?;
            match request {
                Request::Event(event) => {
                    receiver.try_receive(event).map_err(io::Error::other)?;
                    channel.lock().unwrap().send(Reply::Accepted)?;
                }
                Request::Finish => return Ok(()),
            }
        }
    });
    let stream_error = stream
        .err()
        .map(|e| Failure::wrap("E_LOG_PROTOCOL", e.to_string(), NativeIoError::new(e)).record());
    let finish_errors = logger
        .finish()
        .err()
        .into_iter()
        .flatten()
        .map(|error| ErrorRecord {
            schema: ErrorSchema::V1,
            error: error.error_node(),
        })
        .collect();
    let errors = ordered_errors(stream_error, finish_errors);
    channel.lock().unwrap().finish(&errors)
}

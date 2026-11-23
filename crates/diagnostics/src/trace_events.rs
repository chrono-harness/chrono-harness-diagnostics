//! Replay an installed producer's tracing stream through the host's runtime logger.
//! The explicit writer is acknowledged by its owner so its original failure reaches Logger.
use crate::{
    error::{ErrorRecord, ErrorSchema, Failure, RecordedError, WriterError},
    logging::Level,
    runtime,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{self, BufRead, Write},
    path::Path,
    sync::{Arc, Mutex},
};
use tracing_tunnel::{TracingEvent, TracingEventReceiver};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Request {
    Event(TracingEvent),
    Finish,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Reply<'a> {
    Ready,
    Accepted,
    Write(&'a str),
    Flush,
    Finished(&'a [ErrorRecord]),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Acknowledgment {
    Ok,
    Error {
        native_code: Option<i32>,
        kind: String,
        custom_source: bool,
        message: String,
    },
}

struct Channel<R, W> {
    input: R,
    output: W,
}

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
        serde_json::to_writer(&mut self.output, &reply)?;
        self.output.write_all(b"\n")?;
        self.output.flush()
    }

    fn publish(&mut self, reply: Reply<'_>) -> io::Result<()> {
        self.send(reply)?;
        match self.read()? {
            Acknowledgment::Ok => Ok(()),
            Acknowledgment::Error {
                native_code,
                kind,
                custom_source,
                message,
            } => Err(native_code
                .map(io::Error::from_raw_os_error)
                .unwrap_or_else(|| {
                    io::Error::other(WriterError {
                        kind,
                        custom_source,
                        message,
                    })
                })),
        }
    }
}

struct ExplicitSink<R, W>(Arc<Mutex<Channel<R, W>>>);

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

pub fn serve(
    root: &Path,
    producer: &str,
    input: impl BufRead + Send + 'static,
    output: impl Write + Send + 'static,
) -> io::Result<u8> {
    let channel = Arc::new(Mutex::new(Channel { input, output }));
    let sink = ExplicitSink(channel.clone());
    let logger = match runtime::logger(root, producer, Level::INFO, sink) {
        Ok(logger) => logger,
        Err(error) => {
            channel
                .lock()
                .unwrap()
                .send(Reply::Finished(&[error.record()]))?;
            return Ok(2);
        }
    };
    channel.lock().unwrap().send(Reply::Ready)?;
    let stream = logger.run(|| -> io::Result<()> {
        let mut receiver =
            TracingEventReceiver::new(Default::default(), Default::default(), Default::default());
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
    let mut errors: Vec<_> = stream
        .err()
        .map(|e| Failure::new("E_LOG_PROTOCOL", e.to_string()).record())
        .into_iter()
        .collect();
    if let Err(failures) = logger.finish() {
        errors.extend(failures.into_iter().map(|error| ErrorRecord {
            schema: ErrorSchema::V1,
            error: error.error_node(),
        }));
    }
    channel.lock().unwrap().send(Reply::Finished(&errors))?;
    Ok(if errors.is_empty() { 0 } else { 2 })
}

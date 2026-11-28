//! Explicit host adoption of runtime logging; absent adoption preserves the caller's sink.
#[cfg(not(verus_keep_ghost))]
use crate::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub fn retention_window_valid(window: Option<u64>) -> (valid: bool)
    ensures valid == (window != Some(0u64)),
{
    match window {
        None => true,
        Some(seconds) => seconds > 0,
    }
}
}
#[cfg(not(verus_keep_ghost))]
use crate::{
    error::{Failure, NativeIoError},
    logging::{self, Level, Logger},
    rolling::{ExpiredLogs, RollingFile},
    verified_runtime,
};
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

#[cfg(not(verus_keep_ghost))]
pub const CONFIG_PATH: &str = ".chrono-harness/logging.json";

/// Record an existing CLI diagnostic without inventing a typed domain cause from its text.
/// The command still publishes its original diagnostic through its existing output boundary.
#[cfg(not(verus_keep_ghost))]
pub fn diagnostic(message: &str) {
    tracing::error!(event_id = "cli.diagnostic", message = message);
}

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub level: String,
    pub directory: String,
    pub max_bytes: u64,
    pub max_files: usize,
    pub keep_seconds: Option<u64>,
    pub stderr: bool,
}

#[cfg(not(verus_keep_ghost))]
impl Policy {
    fn level(&self) -> Result<Level, Failure> {
        use verified_runtime::LogLevel;
        match verified_runtime::policy_level(&self.level) {
            Some(LogLevel::Trace) => Ok(Level::TRACE),
            Some(LogLevel::Debug) => Ok(Level::DEBUG),
            Some(LogLevel::Info) => Ok(Level::INFO),
            Some(LogLevel::Warn) => Ok(Level::WARN),
            Some(LogLevel::Error) => Ok(Level::ERROR),
            None => Err(Failure::new("E_LOG_CONFIG", "unknown runtime log level")),
        }
    }

    fn validate(&self) -> Result<(), Failure> {
        let directory = Path::new(&self.directory);
        let allowed = verified_runtime::policy_directory_allowed(
            &self.directory,
            directory.components().count(),
            directory
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
            directory.components().collect::<PathBuf>().to_str()
                == Some(self.directory.trim_end_matches('/')),
        );
        if !verified_runtime::policy_fields_allowed(
            &self.schema,
            allowed,
            crate::verified_rolling::valid_bounds(self.max_bytes, self.max_files),
            self.max_bytes,
            crate::rolling::MAX_RUNTIME_LOG_BYTES,
            self.keep_seconds,
        ) {
            return Err(Failure::new(
                "E_LOG_CONFIG",
                "invalid runtime logging schema, directory or finite bounds",
            ));
        }
        self.level()?;
        Ok(())
    }
}

#[cfg(not(verus_keep_ghost))]
pub struct RuntimeSink<W> {
    file: RollingFile,
    stderr: Option<W>,
}

#[cfg(not(verus_keep_ghost))]
impl<W: Write> Write for RuntimeSink<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let (_, _, result) = verified_runtime::write_mirrors(
            &mut self.file,
            self.stderr.as_mut(),
            bytes.len(),
            |file| {
                let result = file.write_all(bytes);
                (file, result)
            },
            |stderr| {
                verified_runtime::write_and_flush(
                    stderr,
                    |stderr| {
                        let result = stderr.write_all(bytes);
                        (stderr, result)
                    },
                    |stderr| {
                        let result = stderr.flush();
                        (stderr, result)
                    },
                )
            },
            |file, stderr| {
                io::Error::other(io_failure(file).with_related("stderr", io_failure(stderr)))
            },
        );
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        let (_, _, result) = verified_runtime::flush_mirrors(
            &mut self.file,
            self.stderr.as_mut(),
            |file| {
                let result = file.flush();
                (file, result)
            },
            |stderr| {
                let result = stderr.flush();
                (stderr, result)
            },
        );
        result
    }
}

#[cfg(not(verus_keep_ghost))]
fn io_failure(error: io::Error) -> Failure {
    Failure::wrap(
        "E_RUNTIME_LOG",
        "runtime logging IO failed",
        NativeIoError::new(error),
    )
}

#[cfg(not(verus_keep_ghost))]
fn load_policy(root: &Path) -> Result<Option<Policy>, Failure> {
    let config = root.join(CONFIG_PATH);
    let present = crate::verified_rolling::regular_member(
        fs::symlink_metadata(&config)
            .map(|metadata| metadata.is_file())
            .map_err(Some),
        |error: &Option<io::Error>| {
            error
                .as_ref()
                .is_some_and(|error| error.kind() == io::ErrorKind::NotFound)
        },
        || None,
    );
    match present {
        Ok(false) => return Ok(None),
        Ok(true) => (),
        Err(None) => {
            return Err(Failure::new(
                "E_LOG_CONFIG",
                "runtime log policy must be a regular file",
            ));
        }
        Err(Some(error)) => return Err(io_failure(error)),
    }
    let mut bytes = Vec::new();
    File::open(&config)
        .map_err(io_failure)?
        .take((verified_runtime::POLICY_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_failure)?;
    if !verified_runtime::policy_size_allowed(bytes.len()) {
        return Err(Failure::new(
            "E_LOG_CONFIG",
            "runtime log policy exceeds 16384 bytes",
        ));
    }
    let policy: Policy = serde_json::from_slice(&bytes).map_err(|source| {
        Failure::wrap(
            "E_LOG_CONFIG",
            "invalid runtime log policy",
            logging::LogFailure::Json {
                source,
                observed_fields: Default::default(),
            },
        )
    })?;
    policy.validate()?;
    Ok(Some(policy))
}

#[cfg(not(verus_keep_ghost))]
fn sink<W>(
    root: &Path,
    policy: &Policy,
    stderr: W,
    explicit: bool,
) -> Result<RuntimeSink<W>, Failure> {
    let root = fs::canonicalize(root).map_err(io_failure)?;
    let file = RollingFile::open_with_window(
        &root.join(&policy.directory),
        policy.max_bytes,
        policy.max_files,
        policy.keep_seconds,
    )
    .map_err(io_failure)?;
    Ok(RuntimeSink {
        file,
        stderr: verified_runtime::mirror_enabled(policy.stderr, explicit).then_some(stderr),
    })
}

/// Retire adopted log generations even when all log writers are idle.
#[cfg(not(verus_keep_ghost))]
pub fn maintain_at(root: &Path, observed: u64) -> Result<Option<ExpiredLogs>, Failure> {
    let policy = load_policy(root)?.map(|policy| {
        let keep_seconds = policy.keep_seconds;
        (policy, keep_seconds)
    });
    let Some((policy, keep_seconds)) = verified_runtime::maintenance_policy(policy) else {
        return Ok(None);
    };
    let root = fs::canonicalize(root).map_err(io_failure)?;
    RollingFile::expire(&root.join(&policy.directory), observed, keep_seconds)
        .map(Some)
        .map_err(io_failure)
}

#[cfg(not(verus_keep_ghost))]
pub fn maintain(root: &Path) -> Result<Option<ExpiredLogs>, Failure> {
    maintain_at(root, crate::rolling::unix_seconds().map_err(io_failure)?)
}

#[cfg(not(verus_keep_ghost))]
pub fn configured_logger<W: Write + Send + 'static>(
    root: &Path,
    producer: &str,
    stderr: W,
) -> Result<Option<Logger<RuntimeSink<W>>>, Failure> {
    verified_runtime::use_policy(
        load_policy(root),
        stderr,
        |policy, stderr| {
            Ok(Some(Logger::new(
                producer,
                policy.level()?,
                sink(root, &policy, stderr, false)?,
            )))
        },
        |_| Ok(None),
    )
}

/// Preserve a library caller's chosen sink and level when the host has not adopted file logs.
#[cfg(not(verus_keep_ghost))]
pub fn logger<W: Write + Send + 'static>(
    root: &Path,
    producer: &str,
    default_level: Level,
    stderr: W,
) -> Result<Logger<Box<dyn Write + Send>>, Failure> {
    verified_runtime::use_policy(
        load_policy(root),
        stderr,
        |policy, stderr| {
            Ok(Logger::new(
                producer,
                policy.level()?,
                Box::new(sink(root, &policy, stderr, true)?) as Box<dyn Write + Send>,
            ))
        },
        |stderr| {
            Ok(Logger::new(
                producer,
                default_level,
                Box::new(stderr) as Box<dyn Write + Send>,
            ))
        },
    )
}

#[cfg(not(verus_keep_ghost))]
fn emergency(producer: &str, failure: &Failure) {
    let logger = Logger::stderr(producer);
    let _ = logger.run(|| {
        logging::failure(
            "runtime.logging_failed",
            &failure.to_string(),
            &failure.record(),
        )
    });
    let _ = logger.finish();
    // An unavailable emergency stderr cannot be advertised as persisted evidence.
}

/// Main-boundary adapter. Explicit --host-root wins; otherwise use the actual cwd.
/// No parent-directory search, environment activation, background task or global subscriber.
#[cfg(not(verus_keep_ghost))]
pub fn cli(producer: &str, work: impl FnOnce() -> u8) -> u8 {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let root = args
        .windows(2)
        .find(|pair| pair[0] == "--host-root")
        .map(|pair| PathBuf::from(&pair[1]))
        .map(Ok)
        .unwrap_or_else(std::env::current_dir);
    let root = match root {
        Ok(root) => root,
        Err(error) => {
            emergency(producer, &io_failure(error));
            return 2;
        }
    };
    let operation = args.first().and_then(|arg| arg.to_str()).unwrap_or("");
    cli_at(&root, producer, operation, work)
}

/// Use a host and operation already selected by an entry with positional arguments.
#[cfg(not(verus_keep_ghost))]
pub fn cli_at(root: &Path, producer: &str, operation: &str, work: impl FnOnce() -> u8) -> u8 {
    cli_session(root, producer, operation, std::process::id(), || Ok(work()))
}

/// Record the lifecycle reported by an independently installed command.
#[cfg(not(verus_keep_ghost))]
pub fn cli_events(
    root: &Path,
    producer: &str,
    operation: &str,
    pid: u32,
    input: impl Read,
    ready: impl Write,
) -> u8 {
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Event {
        Diagnostic(String),
        #[serde(rename = "event")]
        Trace(tracing_tunnel::TracingEvent),
        InvalidArgument(InvalidArgument),
        ExitCode(u8),
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InvalidArgument {
        argv_index: usize,
        bytes: Vec<u8>,
    }
    cli_session(root, producer, operation, pid, || {
        let (_, result) = verified_runtime::write_and_flush(
            ready,
            |mut ready| {
                let result = ready.write_all(b"{\"ready\":true}\n");
                (ready, result)
            },
            |mut ready| {
                let result = ready.flush();
                (ready, result)
            },
        );
        result.map_err(io_failure)?;
        let mut receiver = crate::trace_events::receiver();
        for event in serde_json::Deserializer::from_reader(input).into_iter::<Event>() {
            match event
                .map_err(|error| Failure::wrap("E_LOG_PROTOCOL", error.to_string(), error))?
            {
                Event::Diagnostic(message) => diagnostic(&message),
                Event::Trace(event) => receiver
                    .try_receive(event)
                    .map_err(|error| Failure::wrap("E_LOG_PROTOCOL", error.to_string(), error))?,
                Event::InvalidArgument(argument) => {
                    let error = Failure::new("E_USAGE", "arguments must be valid UTF-8.")
                        .at("argv_index", argument.argv_index as u64)
                        .at(
                            "argument_index",
                            serde_json::json!(argument.argv_index.checked_sub(1)),
                        )
                        .at("argument_bytes", serde_json::json!(argument.bytes));
                    let publish = || {
                        logging::failure(
                            "cli.invalid_argument",
                            &error.to_string(),
                            &error.record(),
                        )
                    };
                    publish().map_err(|e| Failure::new("E_RUNTIME_LOG", e.to_string()))?;
                    let stderr = Logger::stderr(producer);
                    stderr
                        .run(publish)
                        .map_err(|e| Failure::new("E_RUNTIME_LOG", e.to_string()))?;
                    stderr.finish().map_err(|_| {
                        Failure::new("E_RUNTIME_LOG", "cannot publish invalid argument to stderr")
                    })?;
                }
                Event::ExitCode(exit_code) => return Ok(exit_code),
            }
        }
        Err(Failure::new(
            "E_LOG_PROTOCOL",
            "command ended without reporting its exit code",
        ))
    })
}

#[cfg(not(verus_keep_ghost))]
fn cli_session(
    root: &Path,
    producer: &str,
    operation: &str,
    pid: u32,
    work: impl FnOnce() -> Result<u8, Failure>,
) -> u8 {
    let failed = |error: Failure| {
        emergency(producer, &error);
        2
    };
    let logger = match configured_logger(root, producer, io::stderr()) {
        Ok(Some(logger)) => logger,
        Ok(None) => return work().unwrap_or_else(failed),
        Err(error) => return failed(error),
    };
    let exit = logger.run(|| {
        let span = tracing::info_span!("cli", pid, operation, host_root = %root.display());
        let _entered = span.enter();
        tracing::info!(event_id = "cli.started", "command started");
        let exit = work()?;
        if exit == 0 {
            tracing::info!(
                event_id = "cli.finished",
                exit_code = exit,
                "command returned"
            );
        } else {
            tracing::error!(
                event_id = "cli.finished",
                exit_code = exit,
                "command returned with failure"
            );
        }
        Ok(exit)
    });
    verified_runtime::session_exit(exit, logger.finish(), |outcome| {
        let failure = match outcome {
            verified_runtime::SessionFailure::Protocol(error) => error,
            verified_runtime::SessionFailure::Finish { operation, errors } => {
                let failure = Failure::new("E_RUNTIME_LOG", "runtime log publication failed");
                let cause = match operation {
                    Ok(code) => failure.at("operation_exit_code", u64::from(code)),
                    Err(source) => failure.with_related("protocol", source),
                };
                let mut failure = cause;
                for error in errors {
                    failure = failure.with_related("logging", error);
                }
                failure
            }
        };
        emergency(producer, &failure);
    })
}

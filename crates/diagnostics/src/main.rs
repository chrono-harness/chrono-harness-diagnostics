#[cfg(not(verus_keep_ghost))]
use chrono_diagnostics::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub(crate) fn output_exit<E>(result: Result<u8, E>) -> (code: u8)
    ensures code == match result { Ok(status) => status, Err(_) => 2u8 },
{
    match result { Ok(status) => status, Err(_) => 2 }
}
}
#[cfg(not(verus_keep_ghost))]
use chrono_diagnostics::{runtime, trace_events, verified_runtime};
#[cfg(not(verus_keep_ghost))]
use std::fs::File;
#[cfg(not(verus_keep_ghost))]
use std::io::{self, Write};
#[cfg(not(verus_keep_ghost))]
use std::os::fd::AsFd;
#[cfg(not(verus_keep_ghost))]
use std::path::Path;
#[cfg(not(verus_keep_ghost))]
use std::process::ExitCode;

#[cfg(not(verus_keep_ghost))]
fn stdout_file() -> io::Result<File> {
    // std::io::Stdout may treat EBADF as a successful write.
    let stdout = io::stdout().as_fd().try_clone_to_owned()?;
    Ok(File::from(stdout))
}

#[cfg(not(verus_keep_ghost))]
fn publish(
    status: u8,
    write: impl FnOnce(&mut io::BufWriter<File>) -> io::Result<()>,
) -> io::Result<u8> {
    let (writer, result) = verified_runtime::write_and_flush(
        io::BufWriter::new(stdout_file()?),
        |mut writer| {
            let result = write(&mut writer);
            (writer, result)
        },
        |mut writer| {
            let result = writer.flush();
            (writer, result)
        },
    );
    // A failed publication must not retry any pending writes on drop.
    let _ = writer.into_parts();
    result.map(|()| status)
}

#[cfg(not(verus_keep_ghost))]
fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match arguments.as_slice() {
        [version] if version == "--version" => publish(0, |stdout| {
            writeln!(stdout, "chrono-diagnostics {}", env!("CARGO_PKG_VERSION"))
        }),
        [command, root, producer, operation, pid] if command == "cli-events-v1" => {
            let parameters = producer.to_str().zip(operation.to_str()).zip(
                pid.to_str()
                    .and_then(|value| value.parse::<std::num::NonZeroU32>().ok()),
            );
            let Some(((producer, operation), pid)) = parameters else {
                eprintln!(
                    "cli-events-v1 requires a UTF-8 producer and operation, and a positive PID"
                );
                return ExitCode::from(2);
            };
            stdout_file().map(|stdout| {
                runtime::cli_events(
                    Path::new(root),
                    producer,
                    operation,
                    pid.get(),
                    io::stdin().lock(),
                    stdout,
                )
            })
        }
        [command, root, producer] if command == "trace-events-v1" => {
            let Some(producer) = producer.to_str() else {
                eprintln!("trace-events-v1 requires a UTF-8 producer");
                return ExitCode::from(2);
            };
            let policy = if root == "--sink-only" {
                None
            } else {
                Some(Path::new(root))
            };
            let delivered = trace_events::serve_to_fd(
                policy,
                producer,
                io::BufReader::new(io::stdin()),
                io::stdout(),
            )
            .inspect_err(|error| eprintln!("trace-events-v1: {error}"));
            return ExitCode::from(output_exit(delivered));
        }
        [command, root] if command == "expire-logs-v1" => {
            match runtime::maintain(Path::new(root)) {
                Ok(result) => {
                    let status = verified_runtime::expiry_exit(result.as_ref());
                    publish(status, |stdout| {
                        serde_json::to_writer(&mut *stdout, &result).map_err(io::Error::other)?;
                        writeln!(stdout)
                    })
                }
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(2);
                }
            }
        }
        _ => {
            eprintln!(
                "usage: chrono-diagnostics expire-logs-v1 ROOT | trace-events-v1 ROOT|--sink-only PRODUCER | cli-events-v1 ROOT PRODUCER OPERATION PID"
            );
            return ExitCode::from(2);
        }
    };
    let result = result.inspect_err(|error| {
        eprintln!("cannot publish diagnostic command output: {error}");
    });
    ExitCode::from(output_exit(result))
}

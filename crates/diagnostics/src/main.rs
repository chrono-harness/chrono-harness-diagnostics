use chrono_diagnostics::{runtime, trace_events};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let mut stdout = io::stdout().lock();
    let result = match arguments.as_slice() {
        [version] if version == "--version" => {
            writeln!(stdout, "chrono-diagnostics {}", env!("CARGO_PKG_VERSION")).map(|_| 0)
        }
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
            return ExitCode::from(runtime::cli_events(
                Path::new(root),
                producer,
                operation,
                pid.get(),
                io::stdin().lock(),
                stdout,
            ));
        }
        [command, root, producer] if command == "trace-events-v1" => {
            let Some(producer) = producer.to_str() else {
                eprintln!("trace-events-v1 requires a UTF-8 producer");
                return ExitCode::from(2);
            };
            drop(stdout);
            let policy = if root == "--sink-only" {
                None
            } else {
                Some(Path::new(root))
            };
            return match trace_events::serve(
                policy,
                producer,
                io::BufReader::new(io::stdin()),
                io::stdout(),
            ) {
                Ok(status) => ExitCode::from(status),
                Err(error) => {
                    eprintln!("trace-events-v1: {error}");
                    ExitCode::from(2)
                }
            };
        }
        [command, root] if command == "expire-logs-v1" => {
            match runtime::maintain(Path::new(root)) {
                Ok(result) => {
                    let status = u8::from(result.as_ref().is_some_and(|r| !r.errors.is_empty()));
                    serde_json::to_writer(&mut stdout, &result)
                        .map_err(io::Error::other)
                        .and_then(|()| writeln!(stdout))
                        .map(|()| status)
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
    match result.and_then(|status| stdout.flush().map(|()| status)) {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("cannot publish diagnostic command output: {error}");
            ExitCode::from(2)
        }
    }
}

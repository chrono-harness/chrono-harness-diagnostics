//! 双输出的真实调用顺序和结果组合；具体 Write 接口只转交原始结果。
#[cfg(not(verus_keep_ghost))]
use crate::runtime as owner;
#[cfg(not(verus_keep_ghost))]
use crate::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;

#[cfg(verus_keep_ghost)]
#[path = "runtime.rs"]
mod owner;

#[cfg(verus_keep_ghost)]
#[path = "logging.rs"]
pub(crate) mod logging;

#[cfg(verus_keep_ghost)]
#[path = "main.rs"]
pub mod cli;

#[cfg(verus_keep_ghost)]
#[path = "trace_events.rs"]
pub mod trace_events;

verus! {
pub const POLICY_LIMIT: usize = 16_384;

pub fn policy_size_allowed(bytes: usize) -> (allowed: bool)
    ensures allowed == (bytes <= POLICY_LIMIT),
{
    bytes <= POLICY_LIMIT
}

#[derive(PartialEq, Eq)]
pub enum LogLevel { Trace, Debug, Info, Warn, Error }

pub open spec fn has_prefix(text: &str, prefix: &str) -> bool {
    prefix.spec_bytes().len() <= text.spec_bytes().len()
        && forall|i: int| 0 <= i < prefix.spec_bytes().len()
            ==> text.spec_bytes()[i] == prefix.spec_bytes()[i]
}

fn text_starts_with(text: &str, prefix: &str) -> (same: bool)
    ensures same == has_prefix(text, prefix),
{
    let actual = text.as_bytes();
    let expected = prefix.as_bytes();
    if actual.len() < expected.len() {
        return false;
    }
    let mut index = 0usize;
    while index < expected.len()
        invariant index <= expected.len() <= actual.len(),
            forall|i: int| 0 <= i < index ==> actual[i] == expected[i],
            actual@ == text.spec_bytes(), expected@ == prefix.spec_bytes(),
        decreases expected.len() - index,
    {
        if actual[index] != expected[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn text_is(text: &str, literal: &str) -> (same: bool)
    ensures same == (text.spec_bytes() == literal.spec_bytes()),
{
    if text.as_bytes().len() != literal.as_bytes().len() || !text_starts_with(text, literal) {
        return false;
    }
    assert forall|i: int| 0 <= i < literal.spec_bytes().len() implies
        text.spec_bytes()[i] == literal.spec_bytes()[i] by {};
    assert(text.spec_bytes() =~= literal.spec_bytes());
    true
}

pub fn policy_level(text: &str) -> (level: Option<LogLevel>)
    ensures level == (
        if text.spec_bytes() == "trace".spec_bytes() { Some(LogLevel::Trace) }
        else if text.spec_bytes() == "debug".spec_bytes() { Some(LogLevel::Debug) }
        else if text.spec_bytes() == "info".spec_bytes() { Some(LogLevel::Info) }
        else if text.spec_bytes() == "warn".spec_bytes() { Some(LogLevel::Warn) }
        else if text.spec_bytes() == "error".spec_bytes() { Some(LogLevel::Error) }
        else { None }),
{
    if text_is(text, "trace") { Some(LogLevel::Trace) }
    else if text_is(text, "debug") { Some(LogLevel::Debug) }
    else if text_is(text, "info") { Some(LogLevel::Info) }
    else if text_is(text, "warn") { Some(LogLevel::Warn) }
    else if text_is(text, "error") { Some(LogLevel::Error) }
    else { None }
}

/// The path adapter observes components and their normalized spelling using std::path.
pub fn policy_directory_allowed(
    directory: &str, component_count: usize, normal_components: bool, same_spelling: bool,
) -> (allowed: bool)
    ensures allowed == (
        (directory.spec_bytes() == ".chrono-harness/logs/".spec_bytes()
            || (component_count >= 3 && has_prefix(directory, ".chrono-harness/state/")))
        && directory.spec_bytes().len() > 0
        && directory.spec_bytes().last() == 47u8
        && normal_components && same_spelling),
{
    let bytes = directory.as_bytes();
    (text_is(directory, ".chrono-harness/logs/")
        || (component_count >= 3 && text_starts_with(directory, ".chrono-harness/state/")))
        && bytes.len() != 0 && bytes[bytes.len() - 1] == 47u8
        && normal_components && same_spelling
}

pub fn policy_fields_allowed(
    schema: &str, directory_allowed: bool, rolling_bounds: bool,
    bytes: u64, max_bytes: u64, window: Option<u64>,
) -> (allowed: bool)
    ensures allowed == (
        schema.spec_bytes() == "chrono-runtime-logging/v1".spec_bytes()
        && directory_allowed
        && rolling_bounds && bytes <= max_bytes && window != Some(0u64)),
{
    text_is(schema, "chrono-runtime-logging/v1")
        && directory_allowed
        && rolling_bounds && bytes <= max_bytes
        && owner::retention_window_valid(window)
}

pub fn maintenance_policy<P>(policy: Option<(P, Option<u64>)>)
    -> (selection: Option<(P, u64)>)
    ensures match policy {
        Some((value, Some(window))) => selection == Some((value, window)),
        _ => selection.is_none(),
    },
{
    match policy {
        Some((value, Some(window))) => Some((value, window)),
        _ => None,
    }
}

pub fn mirror_enabled(policy_stderr: bool, explicit: bool) -> (mirror: bool)
    ensures mirror == (policy_stderr || explicit),
{
    policy_stderr || explicit
}

/// Absence selects the caller's default, while a rejected policy invokes neither sink.
pub fn use_policy<P, S, T, E, A, D>(
    policy: Result<Option<P>, E>, sink: S, adopt: A, default: D,
) -> (result: Result<T, E>)
    where A: FnOnce(P, S) -> Result<T, E>, D: FnOnce(S) -> Result<T, E>,
    requires match policy {
        Ok(Some(value)) => adopt.requires((value, sink)),
        Ok(None) => default.requires((sink,)),
        Err(_) => true,
    },
    ensures match policy {
        Ok(Some(value)) => adopt.ensures((value, sink), result),
        Ok(None) => default.ensures((sink,), result),
        Err(error) => result == Err::<T, E>(error),
    },
{
    match policy {
        Ok(Some(value)) => adopt(value, sink),
        Ok(None) => default(sink),
        Err(error) => Err(error),
    }
}

#[cfg_attr(not(verus_keep_ghost), derive(Default, serde::Serialize))]
pub struct ExpiredLogs {
    pub removed: usize,
    pub legacy: usize,
    pub busy: bool,
    pub errors: Vec<String>,
}

/// The platform supplies the acquired guard, a busy observation, or its original error.
pub fn with_expiry_lock<L, E, F>(acquired: Result<Option<L>, E>, sweep: F)
    -> (out: Result<ExpiredLogs, E>)
    where F: FnOnce(L) -> ExpiredLogs,
    requires match acquired { Ok(Some(guard)) => sweep.requires((guard,)), _ => true },
    ensures match acquired {
        Err(error) => out == Err::<ExpiredLogs, E>(error),
        Ok(None) => match out {
            Ok(report) => report.busy && report.removed == 0
                && report.legacy == 0 && report.errors.len() == 0,
            Err(_) => false,
        },
        Ok(Some(guard)) => match out {
            Ok(report) => sweep.ensures((guard,), report),
            Err(_) => false,
        },
    },
{
    match acquired {
        Ok(Some(guard)) => Ok(sweep(guard)),
        Ok(None) => Ok(ExpiredLogs { removed: 0, legacy: 0, busy: true, errors: Vec::new() }),
        Err(error) => Err(error),
    }
}

/// A cleanup can remove expired logs and still encounter errors in other files.
pub fn expiry_exit(report: Option<&ExpiredLogs>) -> (status: u8)
    ensures status == match report {
        None => 0u8,
        Some(logs) => if logs.errors.len() == 0 { 0u8 } else { 1u8 },
    },
{
    match report {
        Some(logs) if !logs.errors.is_empty() => 1,
        _ => 0,
    }
}

/// Logging failures keep the original operation outcome and every finish error in order.
pub enum SessionFailure<P, L> {
    Protocol(P),
    Finish { operation: Result<u8, P>, errors: Vec<L> },
}

pub(crate) fn session_exit<P, L, F>(
    operation: Result<u8, P>, finish: Result<(), Vec<L>>, emergency: F,
) -> (status: u8)
    where F: FnOnce(SessionFailure<P, L>),
    requires match finish {
        Ok(()) => match operation {
            Ok(_) => true,
            Err(e) => emergency.requires((SessionFailure::Protocol(e),)),
        },
        Err(errors) => emergency.requires((SessionFailure::Finish { operation, errors },)),
    },
    ensures match finish {
        Ok(()) => match operation {
            Ok(code) => status == code,
            Err(e) => status == 2 && emergency.ensures((SessionFailure::Protocol(e),), ()),
        },
        Err(errors) => status == 2
            && emergency.ensures((SessionFailure::Finish { operation, errors },), ()),
    },
{
    let failure = match finish {
        Ok(()) => match operation {
            Ok(code) => return code,
            Err(error) => SessionFailure::Protocol(error),
        },
        Err(errors) => SessionFailure::Finish { operation, errors },
    };
    emergency(failure);
    2
}


pub open spec fn sequence_result<S, E, W, F>(
    initial: S, write: W, flush: F, out: (S, Result<(), E>),
) -> bool
    where W: FnOnce(S) -> (S, Result<(), E>), F: FnOnce(S) -> (S, Result<(), E>),
{
    exists|middle: S, first: Result<(), E>|
        #[trigger] write.ensures((initial,), (middle, first))
        && match first {
            Err(error) => out == (middle, Err::<(), E>(error)),
            Ok(()) => flush.ensures((middle,), out),
        }
}

/// 写入失败时不刷新；成功时只把真实返回的状态交给刷新接口。
pub fn write_and_flush<S, E, W, F>(state: S, write: W, flush: F)
    -> (out: (S, Result<(), E>))
    where W: FnOnce(S) -> (S, Result<(), E>), F: FnOnce(S) -> (S, Result<(), E>),
    requires write.requires((state,)), forall|state: S| flush.requires((state,)),
    ensures sequence_result(state, write, flush, out),
{
    let ghost initial = state;
    let (state, result) = write(state);
    let ghost middle = state;
    let ghost first = result;
    let out = match result {
        Err(error) => (state, Err(error)),
        Ok(success) => {
            assert(success == ());
            flush(state)
        },
    };
    assert(write.ensures((initial,), (middle, first)));
    out
}

pub open spec fn optional_result<S, E, F>(
    initial: Option<S>, operation: F, out: (Option<S>, Result<(), E>),
) -> bool
    where F: FnOnce(S) -> (S, Result<(), E>),
{
    match initial {
        None => out == (None::<S>, Ok::<(), E>(())),
        Some(state) => exists|after: S|
            #[trigger] operation.ensures((state,), (after, out.1)) && out.0 == Some(after),
    }
}

pub fn run_optional<S, E, F>(state: Option<S>, operation: F)
    -> (out: (Option<S>, Result<(), E>))
    where F: FnOnce(S) -> (S, Result<(), E>),
    requires forall|state: S| operation.requires((state,)),
    ensures optional_result(state, operation, out),
{
    match state {
        None => (None, Ok(())),
        Some(state) => {
            let ghost before = state;
            let (state, result) = operation(state);
            let ghost after = state;
            let out = (Some(state), result);
            assert(operation.ensures((before,), (after, out.1)));
            assert(out.0 == Some(after));
            out
        },
    }
}

pub open spec fn combined_result<E, C>(
    file: Result<(), E>, mirror: Result<(), E>, length: usize, combine: C,
    out: Result<usize, E>,
) -> bool
    where C: FnOnce(E, E) -> E,
{
    match (file, mirror) {
        (Ok(()), Ok(())) => out == Ok::<usize, E>(length),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => out == Err::<usize, E>(error),
        (Err(file), Err(mirror)) => match out {
            Err(error) => combine.ensures((file, mirror), error),
            Ok(_) => false,
        },
    }
}

pub fn combine_results<E, C>(
    file: Result<(), E>, mirror: Result<(), E>, length: usize, combine: C,
) -> (out: Result<usize, E>)
    where C: FnOnce(E, E) -> E,
    requires forall|file: E, mirror: E| combine.requires((file, mirror)),
    ensures combined_result(file, mirror, length, combine, out),
{
    match (file, mirror) {
        (Ok(()), Ok(())) => Ok(length),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(file), Err(mirror)) => Err(combine(file, mirror)),
    }
}

pub open spec fn mirrored_write_result<A, B, E, W, M, C>(
    file: A, mirror: Option<B>, length: usize, write_file: W, write_mirror: M,
    combine: C, out: (A, Option<B>, Result<usize, E>),
) -> bool
    where W: FnOnce(A) -> (A, Result<(), E>),
        M: FnOnce(B) -> (B, Result<(), E>), C: FnOnce(E, E) -> E,
{
    exists|file_result: Result<(), E>, mirror_result: Result<(), E>|
        write_file.ensures((file,), (out.0, file_result))
        && optional_result(mirror, write_mirror, (out.1, mirror_result))
        && #[trigger] combined_result(file_result, mirror_result, length, combine, out.2)
}

/// 两个输出都尝试；文件写入失败仍尝试镜像。两个错误才调用组合所有者。
pub fn write_mirrors<A, B, E, W, M, C>(
    file: A, mirror: Option<B>, length: usize, write_file: W, write_mirror: M, combine: C,
) -> (out: (A, Option<B>, Result<usize, E>))
    where W: FnOnce(A) -> (A, Result<(), E>),
        M: FnOnce(B) -> (B, Result<(), E>), C: FnOnce(E, E) -> E,
    requires write_file.requires((file,)),
        forall|mirror: B| write_mirror.requires((mirror,)),
        forall|file: E, mirror: E| combine.requires((file, mirror)),
    ensures mirrored_write_result(file, mirror, length, write_file, write_mirror, combine, out),
{
    let ghost initial_file = file;
    let ghost initial_mirror = mirror;
    let (file, file_result) = write_file(file);
    let ghost observed_file = file_result;
    let (mirror, mirror_result) = run_optional(mirror, write_mirror);
    let ghost observed_mirror = mirror_result;
    let result = combine_results(file_result, mirror_result, length, combine);
    let out = (file, mirror, result);
    assert(write_file.ensures((initial_file,), (out.0, observed_file)));
    assert(optional_result(initial_mirror, write_mirror, (out.1, observed_mirror)));
    assert(combined_result(observed_file, observed_mirror, length, combine, out.2));
    out
}

pub open spec fn mirrored_flush_result<A, B, E, F, M>(
    file: A, mirror: Option<B>, flush_file: F, flush_mirror: M,
    out: (A, Option<B>, Result<(), E>),
) -> bool
    where F: FnOnce(A) -> (A, Result<(), E>), M: FnOnce(B) -> (B, Result<(), E>),
{
    exists|first: Result<(), E>|
        #[trigger] flush_file.ensures((file,), (out.0, first))
        && match first {
            Err(error) => out.1 == mirror && out.2 == Err::<(), E>(error),
            Ok(()) => optional_result(mirror, flush_mirror, (out.1, out.2)),
        }
}

/// 独立 flush 保留首错：文件失败时保留镜像状态且不调用它。
pub fn flush_mirrors<A, B, E, F, M>(
    file: A, mirror: Option<B>, flush_file: F, flush_mirror: M,
) -> (out: (A, Option<B>, Result<(), E>))
    where F: FnOnce(A) -> (A, Result<(), E>), M: FnOnce(B) -> (B, Result<(), E>),
    requires flush_file.requires((file,)),
        forall|mirror: B| flush_mirror.requires((mirror,)),
    ensures mirrored_flush_result(file, mirror, flush_file, flush_mirror, out),
{
    let ghost initial_file = file;
    let (file, first) = flush_file(file);
    let ghost observed_first = first;
    let out = match first {
        Err(error) => (file, mirror, Err(error)),
        Ok(success) => {
            assert(success == ());
            let (mirror, result) = run_optional(mirror, flush_mirror);
            (file, mirror, result)
        },
    };
    assert(flush_file.ensures((initial_file,), (out.0, observed_first)));
    out
}

}

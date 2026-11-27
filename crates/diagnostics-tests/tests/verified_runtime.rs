//! 双输出的独立结果表、完整错误载荷和实际状态变化。
#![allow(unexpected_cfgs)]
#[cfg(verus_keep_ghost)]
#[path = "../../diagnostics/src/verified_runtime.rs"]
mod policy;
#[cfg(not(verus_keep_ghost))]
use chrono_diagnostics::{verified_runtime as policy, verus};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;

verus! {
#[cfg(verus_keep_ghost)]
fn cli_exit_preserves_both_outcomes(operation: Result<u8, u64>, errors: Option<Vec<u64>>) {
    use policy::SessionFailure;
    let ghost original = errors;
    let finish = match errors { Some(list) => Err(list), None => Ok(()) };
    let status = policy::session_exit(operation, finish, |seen: SessionFailure<u64, u64>|
        requires match original {
            None => match operation {
                Err(source) => seen == SessionFailure::<u64,u64>::Protocol(source),
                Ok(_) => false,
            },
            Some(list) => match seen {
                SessionFailure::Finish { operation: result, errors: retained } =>
                    result == operation && retained@ == list@,
                _ => false,
            },
        },
    {});
    proof { match (operation, original) {
        (Ok(code), None) => assert(status == code),
        _ => assert(status == 2),
    } }
}


struct Sink {
    fail_write: bool,
    fail_flush: bool,
    code: u64,
    trace: Vec<u8>,
}

struct Fault {
    primary: u64,
    related: Option<u64>,
}

spec fn same_sink(a: Sink, b: Sink) -> bool {
    a.fail_write == b.fail_write && a.fail_flush == b.fail_flush && a.code == b.code
}

spec fn single_failure(code: u64, result: Result<(), Fault>) -> bool {
    match result {
        Err(error) => error.primary == code && error.related.is_none(),
        Ok(()) => false,
    }
}

spec fn wrote(sink: Sink, out: (Sink, Result<(), Fault>)) -> bool {
    same_sink(sink, out.0) && out.0.trace@ == sink.trace@.push(1)
        && if sink.fail_write { single_failure(sink.code, out.1) } else { out.1.is_ok() }
}

spec fn flushed(sink: Sink, out: (Sink, Result<(), Fault>)) -> bool {
    same_sink(sink, out.0) && out.0.trace@ == sink.trace@.push(2)
        && if sink.fail_flush { single_failure(sink.code, out.1) } else { out.1.is_ok() }
}

spec fn mirrored(sink: Sink, out: (Sink, Result<(), Fault>)) -> bool {
    same_sink(sink, out.0)
        && if sink.fail_write {
            out.0.trace@ == sink.trace@.push(1) && single_failure(sink.code, out.1)
        } else {
            out.0.trace@ == sink.trace@.push(1).push(2)
                && if sink.fail_flush { single_failure(sink.code, out.1) } else { out.1.is_ok() }
        }
}

fn write(mut sink: Sink) -> (out: (Sink, Result<(), Fault>))
    ensures wrote(sink, out),
{
    sink.trace.push(1);
    let result = if sink.fail_write {
        Err(Fault { primary: sink.code, related: None })
    } else { Ok(()) };
    (sink, result)
}

fn flush(mut sink: Sink) -> (out: (Sink, Result<(), Fault>))
    ensures flushed(sink, out),
{
    sink.trace.push(2);
    let result = if sink.fail_flush {
        Err(Fault { primary: sink.code, related: None })
    } else { Ok(()) };
    (sink, result)
}

fn write_mirror(sink: Sink) -> (out: (Sink, Result<(), Fault>))
    ensures mirrored(sink, out),
{
    let ghost initial = sink;
    let write = |sink: Sink| -> (out: (Sink, Result<(), Fault>))
        ensures wrote(sink, out),
    { write(sink) };
    let flush = |sink: Sink| -> (out: (Sink, Result<(), Fault>))
        ensures flushed(sink, out),
    { flush(sink) };
    let out = policy::write_and_flush(sink, write, flush);
    proof {
        let (middle, first) = choose|middle: Sink, first: Result<(), Fault>|
            #[trigger] write.ensures((initial,), (middle, first))
            && match first {
                Err(error) => out == (middle, Err::<(), Fault>(error)),
                Ok(()) => flush.ensures((middle,), out),
            };
        assert(wrote(initial, (middle, first)));
        if first.is_ok() { assert(flushed(middle, out)); }
    }
    out
}

fn expect(condition: bool)
    requires condition,
{
    if !condition { panic!(); }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn expiry_status_retains_errors_after_partial_cleanup() {
    expect(policy::expiry_exit(None) == 0);
    let mut report = policy::ExpiredLogs {
        removed: 0, legacy: 0, busy: false, errors: Vec::new(),
    };
    expect(policy::expiry_exit(Some(&report)) == 0);
    report.removed = 1;
    expect(policy::expiry_exit(Some(&report)) == 0);
    report.errors.push(String::new());
    expect(policy::expiry_exit(Some(&report)) == 1);
    report.removed = 0;
    report.errors.push(String::new());
    expect(policy::expiry_exit(Some(&report)) == 1);
}

fn fresh(fail_write: bool, fail_flush: bool, code: u64) -> (sink: Sink)
    ensures sink.fail_write == fail_write, sink.fail_flush == fail_flush,
        sink.code == code, sink.trace@ == Seq::<u8>::empty(),
{
    Sink { fail_write, fail_flush, code, trace: Vec::new() }
}

fn write_case(file_failed: bool, present: bool, mirror_write_failed: bool,
    mirror_flush_failed: bool, wanted: u8, length: usize)
    requires
        wanted <= 3,
        (wanted == 0) == (!file_failed && (!present || (!mirror_write_failed && !mirror_flush_failed))),
        (wanted == 1) == (file_failed && (!present || (!mirror_write_failed && !mirror_flush_failed))),
        (wanted == 2) == (!file_failed && present && (mirror_write_failed || mirror_flush_failed)),
        (wanted == 3) == (file_failed && present && (mirror_write_failed || mirror_flush_failed)),
{
    let file = fresh(file_failed, false, u64::MAX);
    let mirror = if present { Some(fresh(mirror_write_failed, mirror_flush_failed, 0)) } else { None };
    let ghost initial_file = file;
    let ghost initial_mirror = mirror;
    let write_file = |sink: Sink| -> (out: (Sink, Result<(), Fault>))
        ensures wrote(sink, out),
    { write(sink) };
    let write_mirror = |sink: Sink| -> (out: (Sink, Result<(), Fault>))
        ensures mirrored(sink, out),
    { write_mirror(sink) };
    let combine = |file: Fault, mirror: Fault| -> (out: Fault)
        ensures out.primary == file.primary, out.related == Some(mirror.primary),
    { Fault { primary: file.primary, related: Some(mirror.primary) } };
    let (file, mirror, result) = policy::write_mirrors(
        file, mirror, length, write_file, write_mirror, combine,
    );
    let ghost observed_result = result;
    proof {
        let (first, second) = choose|first: Result<(), Fault>, second: Result<(), Fault>|
            write_file.ensures((initial_file,), (file, first))
            && policy::optional_result(initial_mirror, write_mirror, (mirror, second))
            && #[trigger] policy::combined_result(first, second, length, combine, result);
        assert(wrote(initial_file, (file, first)));
        if let Some(initial) = initial_mirror {
            let after = choose|after: Sink|
                #[trigger] write_mirror.ensures((initial,), (after, second)) && mirror == Some(after);
            assert(mirrored(initial, (after, second)));
        }
        if let (Err(file_error), Err(mirror_error)) = (first, second) {
            match observed_result {
                Err(error) => assert(combine.ensures((file_error, mirror_error), error)),
                Ok(_) => assert(false),
            }
        }
    }
    expect(file.fail_write == file_failed);
    expect(!file.fail_flush);
    expect(file.code == u64::MAX);
    expect(file.trace.len() == 1);
    expect(file.trace[0] == 1);
    match mirror {
        None => expect(!present),
        Some(mirror) => {
            expect(present);
            expect(mirror.fail_write == mirror_write_failed);
            expect(mirror.fail_flush == mirror_flush_failed);
            expect(mirror.code == 0);
            expect(mirror.trace.len() == if mirror_write_failed { 1 } else { 2 });
            expect(mirror.trace[0] == 1);
            if !mirror_write_failed { expect(mirror.trace[1] == 2); }
        },
    }
    match result {
        Ok(actual_length) => { expect(wanted == 0); expect(actual_length == length); },
        Err(error) => {
            expect(wanted != 0);
            expect(error.primary == if wanted == 2 { 0 } else { u64::MAX });
            expect(error.related == if wanted == 3 { Some(0) } else { None });
        },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn write_outcome_table_keeps_every_attempt_error_and_success_length() {
    write_case(false, false, false, false, 0, 0);
    write_case(false, false, false, true, 0, usize::MAX);
    write_case(false, false, true, false, 0, 1);
    write_case(false, false, true, true, 0, 17);
    write_case(true, false, false, false, 1, 0);
    write_case(true, false, false, true, 1, usize::MAX);
    write_case(true, false, true, false, 1, 1);
    write_case(true, false, true, true, 1, 17);
    write_case(false, true, false, false, 0, usize::MAX);
    write_case(false, true, false, true, 2, 0);
    write_case(false, true, true, false, 2, 1);
    write_case(false, true, true, true, 2, 17);
    write_case(true, true, false, false, 1, usize::MAX);
    write_case(true, true, false, true, 3, 0);
    write_case(true, true, true, false, 3, 1);
    write_case(true, true, true, true, 3, 17);
}

fn flush_case(file_failed: bool, present: bool, mirror_failed: bool, wanted: u8)
    requires wanted <= 2,
        (wanted == 0) == (!file_failed && (!present || !mirror_failed)),
        (wanted == 1) == file_failed,
        (wanted == 2) == (!file_failed && present && mirror_failed),
{
    let file = fresh(false, file_failed, 0);
    let mirror = if present { Some(fresh(false, mirror_failed, u64::MAX)) } else { None };
    let ghost initial_file = file;
    let ghost initial_mirror = mirror;
    let flush_file = |sink: Sink| -> (out: (Sink, Result<(), Fault>))
        ensures flushed(sink, out),
    { flush(sink) };
    let flush_mirror = |sink: Sink| -> (out: (Sink, Result<(), Fault>))
        ensures flushed(sink, out),
    { flush(sink) };
    let (file, mirror, result) = policy::flush_mirrors(file, mirror, flush_file, flush_mirror);
    proof {
        let first = choose|first: Result<(), Fault>|
            #[trigger] flush_file.ensures((initial_file,), (file, first))
            && match first {
                Err(error) => mirror == initial_mirror && result == Err::<(), Fault>(error),
                Ok(()) => policy::optional_result(initial_mirror, flush_mirror, (mirror, result)),
            };
        assert(flushed(initial_file, (file, first)));
        if first.is_ok() {
            if let Some(initial) = initial_mirror {
                let after = choose|after: Sink|
                    #[trigger] flush_mirror.ensures((initial,), (after, result)) && mirror == Some(after);
                assert(flushed(initial, (after, result)));
            }
        }
    }
    expect(file.code == 0);
    expect(file.fail_flush == file_failed);
    expect(!file.fail_write);
    expect(file.trace.len() == 1);
    expect(file.trace[0] == 2);
    match mirror {
        None => expect(!present),
        Some(mirror) => {
            expect(present);
            expect(mirror.code == u64::MAX);
            expect(mirror.fail_flush == mirror_failed);
            expect(!mirror.fail_write);
            expect(mirror.trace.len() == if file_failed { 0 } else { 1 });
            if !file_failed { expect(mirror.trace[0] == 2); }
        },
    }
    match result {
        Ok(()) => expect(wanted == 0),
        Err(error) => {
            expect(wanted != 0);
            expect(error.primary == if wanted == 1 { 0 } else { u64::MAX });
            expect(error.related.is_none());
        },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn flush_outcome_table_stops_after_the_first_failure() {
    flush_case(false, false, false, 0);
    flush_case(false, false, true, 0);
    flush_case(false, true, false, 0);
    flush_case(false, true, true, 2);
    flush_case(true, false, false, 1);
    flush_case(true, false, true, 1);
    flush_case(true, true, false, 1);
    flush_case(true, true, true, 1);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn write_then_flush_appends_to_the_actual_previous_state() {
    let mut sink = fresh(false, true, 17);
    sink.trace.push(99);
    let (sink, result) = write_mirror(sink);
    expect(sink.trace.len() == 3);
    expect(sink.trace[0] == 99);
    expect(sink.trace[1] == 1);
    expect(sink.trace[2] == 2);
    match result {
        Err(error) => { expect(error.primary == 17); expect(error.related.is_none()); },
        Ok(()) => { proof { assert(false); } panic!(); },
    }
}

}

#[cfg(verus_keep_ghost)]
verus! {
use policy::logging::{resolve_reserved, ReservedFailure};

fn sink_permissions_keep_closed_priority_and_only_flush_once(cause: u64) {
    use policy::logging::{publication, begin_finish, Publication};
    let open = publication(false, None::<&u64>);
    assert(matches!(open, Publication::Write));
    match publication(false, Some(&cause)) {
        Publication::Blocked(original) => { assert(*original == cause); }
        _ => { assert(false); }
    }
    let closed_failure = publication(true, Some(&cause));
    assert(matches!(closed_failure, Publication::Closed));
    let closed = publication(true, None::<&u64>);
    assert(matches!(closed, Publication::Closed));
    let mut healthy = false;
    let flush = begin_finish(&mut healthy, false);
    assert(healthy && flush);
    let again = begin_finish(&mut healthy, false);
    assert(healthy && !again);
    let mut stopped = false;
    let skip = begin_finish(&mut stopped, true);
    assert(stopped && !skip);
}

// Independent expectations cover arbitrary payloads, including the error selected
// before a later invalid value and propagation of the decoder's own result.
fn reserved_field_expectations(id: String, msg: String, raw: String, bad: u64) {
    let decode = |s: String| -> (out: Result<String, String>)
        ensures out == Ok::<String, String>(s),
    { Ok(s) };
    let reject = |s: String| -> (out: Result<String, String>)
        ensures out == Err::<String, String>(s),
    { Err(s) };
    let absent = resolve_reserved::<u64, _, _, _>(None, None, None, decode);
    assert(match absent {
        Ok((i, m, e)) => i@ == "tracing.external"@ && m@ == ""@ && e.is_none(),
        _ => false,
    });
    let all_bad = resolve_reserved(Some(Err(bad)), Some(Err(1u64)), Some(Err(2u64)), decode);
    assert(match all_bad { Err(ReservedFailure::Field(key, v)) => key == "event_id" && v == bad, _ => false });
    let two_bad = resolve_reserved(None, Some(Err(bad)), Some(Err(2u64)), decode);
    assert(match two_bad { Err(ReservedFailure::Field(key, v)) => key == "message" && v == bad, _ => false });
    let wrong = resolve_reserved(None, None, Some(Err(bad)), decode);
    assert(match wrong { Err(ReservedFailure::Field(key, v)) => key == "chrono_error" && v == bad, _ => false });
    let ghost original_id = id;
    let ghost original_msg = msg;
    let ghost original_raw = raw;
    let accepted = resolve_reserved::<u64, _, _, _>(Some(Ok(id)), Some(Ok(msg)), Some(Ok(raw.clone())), decode);
    assert(match accepted {
        Ok((i, m, Some(record))) => i == original_id && m == original_msg && record == original_raw,
        _ => false,
    });
    let rejected = resolve_reserved::<u64, _, _, _>(None, None, Some(Ok(raw)), reject);
    assert(match rejected { Err(ReservedFailure::Decode(reason)) => reason == original_raw, _ => false });
}
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn reserved_fields_reach_the_writer_or_report_the_first_original_failure() {
    use chrono_diagnostics::error::{Failure, RecordedError};
    use chrono_diagnostics::logging::{LogFailure, LogRecord, Logger};
    use serde_json::json;
    use std::io::{Read, Seek};
    let path = std::env::temp_dir().join(format!("chrono-reserved-{}.jsonl", std::process::id()));
    let mut reader = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(path).unwrap();
    let logger = Logger::new(
        "reserved",
        tracing::Level::INFO,
        reader.try_clone().unwrap(),
    );
    let original = Failure::new("E_INPUT", "original").record();
    let encoded = serde_json::to_string(&original).unwrap();
    logger.run(|| {
        tracing::info!(custom = 7u64);
        tracing::info!(
            event_id = "explicit",
            message = "本文",
            chrono_error = encoded.as_str()
        );
        tracing::info!(event_id = 11u64, message = true, chrono_error = 12u64);
        tracing::info!(message = false, chrono_error = 13u64);
        tracing::info!(chrono_error = 14u64);
        tracing::info!(chrono_error = "{");
        tracing::info!(event_id = "", message = "");
    });
    let failures = logger.finish().unwrap_err();
    assert_eq!(failures.len(), 4);
    for (failure, (name, value)) in failures.iter().zip([
        ("event_id", json!(11)),
        ("message", json!(false)),
        ("chrono_error", json!(14)),
    ]) {
        let LogFailure::InvalidField { field, observed } = failure.as_ref() else {
            panic!("expected a field error")
        };
        assert_eq!((*field, observed), (name, &value));
        assert_eq!(failure.error_node().code, "E_LOG_FIELD");
    }
    let LogFailure::Json {
        source,
        observed_fields,
    } = failures[3].as_ref()
    else {
        panic!("expected a decoder error")
    };
    assert!(source.is_eof());
    assert_eq!(observed_fields.get("chrono_error"), Some(&json!("{")));
    assert_eq!(failures[3].error_node().code, "E_LOG_JSON");
    reader.rewind().unwrap();
    let mut bytes = String::new();
    reader.read_to_string(&mut bytes).unwrap();
    let rows: Vec<LogRecord> = bytes
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        (&*rows[0].event, &*rows[0].message),
        ("tracing.external", "")
    );
    assert_eq!(rows[0].fields["custom"], 7);
    assert_eq!(rows[0].error, None);
    assert_eq!((&*rows[1].event, &*rows[1].message), ("explicit", "本文"));
    assert_eq!(rows[1].error, Some(original));
    for name in ["event_id", "message", "chrono_error"] {
        assert!(!rows[1].fields.contains_key(name));
    }
    assert_eq!((&*rows[2].event, &*rows[2].message), ("", ""));
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn closed_sink_keeps_the_first_io_failure_and_the_late_record() {
    use chrono_diagnostics::logging::{LogFailure, LogRecord, Logger};
    use std::{io::Cursor, sync::Arc};

    let logger = Logger::new("closed", tracing::Level::INFO, Cursor::new([0u8; 0]));
    logger.run(|| tracing::info!(event_id = "first", message = "zero-capacity sink"));
    let failed = logger.finish().unwrap_err();
    assert!(matches!(&*failed[0], LogFailure::Io { .. }));
    logger.run(|| tracing::info!(event_id = "after.finish", message = "retain closed record"));
    let final_errors = logger.finish().unwrap_err();
    assert_eq!(final_errors.len(), 2);
    assert!(Arc::ptr_eq(&failed[0], &final_errors[0]));
    let LogFailure::Closed { attempted_record } = &*final_errors[1] else {
        panic!("expected a closed publication after finish");
    };
    let late: LogRecord = serde_json::from_slice(attempted_record).unwrap();
    assert_eq!(late.event, "after.finish");
}

#[cfg(not(verus_keep_ghost))]
use chrono_diagnostics::trace_events;
#[cfg(verus_keep_ghost)]
use policy::trace_events;

verus! {
fn acknowledgment_precedence(send_error: bool, read_error: bool, native_code: Option<i32>,
    kind: String, custom: bool, message: String) {
    use trace_events::{Acknowledgment, acknowledged};
    let sent = if send_error { Err(70i32) } else { Ok(()) };
    let ack = || -> (r: Result<Acknowledgment,i32>)
        ensures r == if read_error { Err::<Acknowledgment,i32>(80i32) } else {
            Ok::<Acknowledgment,i32>(Acknowledgment::Error { native_code, kind, custom_source: custom, message })
        }
    {
        if read_error { Err(80i32) } else { Ok(Acknowledgment::Error { native_code, kind, custom_source: custom, message }) }
    };
    let native = |n: i32| -> (e:i32) ensures e == n { n };
    let writer = |k:String, c:bool, m:String| -> (e:i32)
        ensures e == if c { 91i32 } else { 92i32 }
    { if c { 91i32 } else { 92i32 } };
    let observed = acknowledged(sent, ack, native, writer);
    assert(observed == Err::<(),i32>(if send_error { 70i32 } else if read_error { 80i32 }
        else if let Some(code) = native_code { code } else if custom { 91i32 } else { 92i32 }));
}

fn acknowledged_custom_fields(kind: &String, flag: bool, message: &String) {
    use trace_events::{Acknowledgment, acknowledged};
    let read = || -> (r:Result<Acknowledgment,(String,bool,String)>)
        ensures r == Ok::<Acknowledgment,(String,bool,String)>(Acknowledgment::Error {
            native_code: None, kind: *kind, custom_source: flag, message: *message })
    { Ok(Acknowledgment::Error {native_code: None, kind: kind.clone(), custom_source: flag, message: message.clone()}) };
    let native = |n:i32| -> (String,bool,String) { (String::new(),false,String::new()) };
    let custom = |k:String,c:bool,m:String| -> (e:(String,bool,String)) ensures e == (k,c,m) { (k,c,m) };
    let result = acknowledged(Ok(()), read, native, custom);
    assert(result == Err::<(),(String,bool,String)>((*kind,flag,*message)));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn terminal_status_and_error_order_follow_the_delivered_reply() {
    let mut later = Vec::new();
    later.push(7u8);
    later.push(9u8);
    let both = trace_events::ordered_errors(Some(41u8), later);
    expect(both.len() == 3 && both[0] == 41 && both[1] == 7 && both[2] == 9);
    let empty = trace_events::ordered_errors(None::<u8>, Vec::new());
    expect(empty.len() == 0);
    expect(matches!(trace_events::delivery_status(&empty, Ok::<(), u8>(())), Ok(0)));
    expect(matches!(trace_events::delivery_status(&both, Ok::<(), u8>(())), Ok(2)));
    expect(matches!(trace_events::delivery_status(&empty, Err(93u8)), Err(93)));
    expect(matches!(trace_events::delivery_status(&both, Err(94u8)), Err(94)));
}
}

#[cfg(not(verus_keep_ghost))]
use std::io::{self, Write};

#[cfg(not(verus_keep_ghost))]
#[test]
fn sending_a_failed_reply_never_consumes_an_acknowledgment() {
    use std::cell::Cell;
    let reads = Cell::new(0);
    let read = || {
        reads.set(reads.get() + 1);
        Ok(trace_events::Acknowledgment::Ok)
    };
    let native = |code| code;
    let writer = |_, _, _| 92;
    assert_eq!(
        trace_events::acknowledged(Err(32), read, native, writer),
        Err(32)
    );
    assert_eq!(reads.get(), 0);
    assert_eq!(
        trace_events::acknowledged(Ok(()), read, native, writer),
        Ok(())
    );
    assert_eq!(reads.get(), 1);
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn terminal_reply_keeps_the_rejected_event_before_the_original_flush_failure() {
    use serde_json::{Value, json};
    use std::io::{Read, Seek};
    let path = std::env::temp_dir().join(format!("chrono-trace-finish-{}", std::process::id()));
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(path).unwrap();
    let requests = [
        json!({"event":{"span_entered":{"id":91}}}),
        json!({"error":{"native_code":28,"kind":"StorageFull","custom_source":false,"message":"sink full"}}),
    ];
    let input = format!("{}\n{}\n", requests[0], requests[1]);
    let code = trace_events::serve(
        None,
        "rejected",
        io::Cursor::new(input),
        output.try_clone().unwrap(),
    )
    .unwrap();
    output.rewind().unwrap();
    let mut bytes = String::new();
    output.read_to_string(&mut bytes).unwrap();
    let replies: Vec<Value> = bytes
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(code, 2);
    assert_eq!(replies.len(), 3);
    assert_eq!(&replies[..2], &[json!("ready"), json!("flush")]);
    let errors = replies[2]["finished"].as_array().unwrap();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0]["error"]["code"], "E_LOG_PROTOCOL");
    assert_eq!(
        errors[0]["error"]["source"]["source"]["source"]["context"]["span_id"],
        91
    );
    assert_eq!(errors[1]["error"]["code"], "E_LOG_IO");
    assert_eq!(errors[1]["error"]["source"]["native_code"], 28);
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn trace_reply_to_a_read_only_descriptor_keeps_the_os_error() {
    let fd = std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let failure = chrono_diagnostics::trace_events::serve_to_fd(
        None,
        "readonly",
        io::Cursor::new(b"\"finish\"\n\"ok\"\n"),
        fd,
    )
    .unwrap_err();
    assert_eq!(failure.raw_os_error(), Some(9));
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn an_undelivered_terminal_reply_returns_the_original_write_or_flush_error() {
    struct TerminalFailure {
        acknowledged: usize,
        fail_flush: bool,
    }
    impl Write for TerminalFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.acknowledged == 2 && !self.fail_flush {
                return Err(io::Error::from_raw_os_error(32));
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.acknowledged == 2 && self.fail_flush {
                return Err(io::Error::from_raw_os_error(28));
            }
            self.acknowledged += 1;
            Ok(())
        }
    }
    for fail_flush in [false, true] {
        let failure = chrono_diagnostics::trace_events::serve(
            None,
            "terminal",
            io::Cursor::new(b"\"finish\"\n\"ok\"\n"),
            TerminalFailure {
                acknowledged: 0,
                fail_flush,
            },
        )
        .unwrap_err();
        assert_eq!(
            failure.raw_os_error(),
            Some(if fail_flush { 28 } else { 32 })
        );
    }
}

#[cfg(verus_keep_ghost)]
verus! {
fn trace_exit_keeps_the_original_status_or_reports_failure(code: u8, reason: u64) {
    let completed = policy::cli::trace_exit(Ok::<u8, u64>(code));
    let failed = policy::cli::trace_exit(Err::<u8, u64>(reason));
    assert(completed == code && failed == 2);
}
}

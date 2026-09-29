use super::*;

#[test]
fn symbols_are_nul_terminated_and_unique() {
    let syms = [SYM_DESCRIBE, SYM_INIT, SYM_CALL, SYM_SHUTDOWN, SYM_FREE];
    for s in &syms {
        assert!(s.len() > 1, "symbol must not be empty");
        assert_eq!(*s.last().unwrap(), b'\0', "symbol must be NUL terminated");
        assert!(!s[..s.len() - 1].contains(&b'\0'), "interior NUL forbidden");
    }
    let names: Vec<_> = syms.iter().map(|s| s.to_vec()).collect();
    for (i, a) in names.iter().enumerate() {
        for b in names.iter().skip(i + 1) {
            assert_ne!(a, b, "symbols must be unique");
        }
    }
}

#[test]
fn result_codes_are_distinct_and_non_positive_except_ok() {
    let codes = [
        code::OK,
        code::ERR_PANIC,
        code::ERR_UNKNOWN_TOOL,
        code::ERR_CALL_FAILED,
        code::ERR_BAD_ARGS,
        code::ERR_BAD_HANDLE,
        code::ERR_UNSUPPORTED,
    ];
    for (i, a) in codes.iter().enumerate() {
        for b in codes.iter().skip(i + 1) {
            assert_ne!(a, b);
        }
    }
    for c in codes.iter().skip(1) {
        assert!(*c < 0, "error codes must be negative");
    }
}

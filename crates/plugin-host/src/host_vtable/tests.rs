use super::*;

#[test]
fn kv_put_stores_value() {
    let key = c"probe-key";
    let val = c"probe-value";
    let rc = host_kv_put(
        key.as_ptr() as *const u8,
        key.to_bytes().len(),
        val.as_ptr() as *const u8,
        val.to_bytes().len(),
    );
    assert_eq!(rc, code::OK);
    let map = KV.lock().unwrap();
    assert_eq!(
        map.get(key.to_bytes()).map(|v| v.as_slice()),
        Some(val.to_bytes())
    );
}

#[test]
fn kv_get_is_unsupported_until_ownership_protocol_lands() {
    let mut out: *mut u8 = std::ptr::null_mut();
    let mut olen: usize = 0;
    let k = c"k";
    let rc = host_kv_get(
        k.as_ptr() as *const u8,
        k.count_bytes(),
        &mut out,
        &mut olen,
    );
    assert_eq!(rc, code::ERR_UNSUPPORTED);
}

#[test]
fn kv_put_rejects_null_pointers() {
    assert_eq!(
        host_kv_put(std::ptr::null(), 0, std::ptr::null(), 0),
        code::ERR_BAD_ARGS
    );
}

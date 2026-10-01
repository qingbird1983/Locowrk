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

/// ⑭ notify 槽：钩子在 → 投递并 OK；钩子不在 → UNSUPPORTED（如实告知，
/// 插件侧转成工具错误回给模型）。钩子是进程级静态，本用例独占操作它
/// （先装后卸），其余用例不触碰。
#[test]
fn notify_dispatches_via_hook_and_unsupported_without() {
    let title = c"t";
    let body = c"b";
    let call = |t: *const u8, b: *const u8| {
        host_notify(t, title.to_bytes().len(), b, body.to_bytes().len())
    };
    // 卸载残留（理论无，防御未来新增并行用例）
    *NOTIFY_HOOK.lock().unwrap() = None;
    assert_eq!(
        call(title.as_ptr() as *const u8, body.as_ptr() as *const u8),
        code::ERR_UNSUPPORTED
    );

    let got = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&got);
    set_notify_hook(Arc::new(move |t, b| {
        sink.lock().unwrap().push(format!("{t}/{b}"));
    }));
    assert_eq!(
        call(title.as_ptr() as *const u8, body.as_ptr() as *const u8),
        code::OK
    );
    assert_eq!(*got.lock().unwrap(), vec!["t/b".to_string()]);
    assert_eq!(
        call(std::ptr::null(), body.as_ptr() as *const u8),
        code::ERR_BAD_ARGS
    );
    // 还原：不给其他测试留下全局副作用
    *NOTIFY_HOOK.lock().unwrap() = None;
}

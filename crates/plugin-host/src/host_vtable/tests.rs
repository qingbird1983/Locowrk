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

// ---- 调用期取消查询（C3，specs/04）------------------------------------------

/// 调用期内读的是挂进去的那个标志位（0 → 翻成 1 仍读同一份）；非调用期 =
/// `ERR_UNSUPPORTED`。thread-local 按线程隔离，并行用例互不可见。
#[test]
fn call_cancelled_reads_flag_only_in_call_period() {
    // 非调用期（未预置）：插件自起后台线程的同款读数
    assert_eq!(host_call_cancelled(), code::ERR_UNSUPPORTED);

    let flag = Arc::new(AtomicBool::new(false));
    let guard = call_cancel_guard(Arc::clone(&flag));
    assert_eq!(host_call_cancelled(), 0);
    flag.store(true, Ordering::Relaxed);
    assert_eq!(host_call_cancelled(), 1);

    // 便捷封装走同一个静态表：1 → Ok(true)
    // SAFETY: HOST_VTABLE 是本 crate 的静态实例
    let vt: &HostVtable = &HOST_VTABLE;
    assert_eq!(unsafe { vt.call_cancelled() }, Ok(true));

    drop(guard);
    assert_eq!(host_call_cancelled(), code::ERR_UNSUPPORTED);
}

/// RAII：guard 丢弃（含提前返回与 panic 展开）后线程回到非调用期，
/// 不残留给下一次调用。
#[test]
fn call_cancel_guard_clears_on_drop() {
    let flag = Arc::new(AtomicBool::new(false));
    {
        let _guard = call_cancel_guard(Arc::clone(&flag));
        assert_eq!(host_call_cancelled(), 0);
    }
    assert_eq!(host_call_cancelled(), code::ERR_UNSUPPORTED);
}

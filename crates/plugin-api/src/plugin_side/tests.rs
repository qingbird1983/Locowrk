use super::*;

/// write_out 分配的缓冲按字节读回再释放，内容必须无损。
fn roundtrip(bytes: &[u8]) -> Vec<u8> {
    let mut out: *mut u8 = ptr::null_mut();
    let mut out_len: usize = 0;
    // SAFETY: out/out_len 是本测试栈上的合法槽位
    assert_eq!(
        unsafe { write_out(bytes, &mut out, &mut out_len) },
        code::OK
    );
    if bytes.is_empty() {
        assert!(out.is_null());
        assert_eq!(out_len, 0);
        return Vec::new();
    }
    // SAFETY: write_out 成功后 (out, out_len) 指向可读缓冲
    let copy = unsafe { std::slice::from_raw_parts(out, out_len) }.to_vec();
    // SAFETY: (out, out_len) 与 write_out 的分配配对
    unsafe { free_allocated(out, out_len) };
    copy
}

#[test]
fn write_out_roundtrips_bytes() {
    assert_eq!(roundtrip(b"hello ffi"), b"hello ffi");
    assert_eq!(roundtrip(&[0u8, 1, 254, 255]), [0u8, 1, 254, 255]);
}

#[test]
fn empty_payload_yields_null_buffer() {
    assert!(roundtrip(b"").is_empty());
}

#[test]
fn release_of_null_or_empty_buffer_is_noop() {
    // SAFETY: 空指针/零长度按契约必须无操作
    unsafe {
        free_allocated(ptr::null_mut(), 0);
        free_allocated(ptr::null_mut(), 16);
    }
}

// 注：无法在单测里 dlopen 自身验证导出符号；declare_plugin! 的端到端行为
// 由 plugins-src/tool-echo（编译期）与 plugin-host 集成测试覆盖。

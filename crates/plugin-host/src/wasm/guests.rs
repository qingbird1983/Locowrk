//! 测试用 WAT 客户机（cfg(test)）：不用 wasm32 工具链，`wat` 即时编译。
//!
//! - [`MAIN_GUEST`]：五件套工具（echo / fail / trap / loop / oob），
//!   覆盖正常回包、错误信封、trap、死循环（epoch 预算）、谎报输出
//! - [`NO_NUL_GUEST`]：describe 返回无 NUL 的整页数据（扫描上限兜底）
//! - [`MINIMAL_GUEST`]：缺 alloc 等导出（ABI 清单报错）
//! - [`WASI_GUEST`]：声明 wasi 导入（宿主 Linker 未提供 → 实例化失败）
//! - [`BIGMEM_GUEST`]：声明 2.5 GiB 内存（超 limiter → 实例化失败）
//! - [`CANCEL_GUEST`]：导入 `locowrk.call_cancelled`（C3，specs/04）——
//!   `poll_cancel` 轮询到 1 才收束、`init_probe` 报 init 期（非调用期）读数
//!
//! 内存布局（MAIN_GUEST 页 0/1 静态区）：1024 manifest · 2048 "ping"
//! 日志载荷 · 2064/2080 错误信封 · 4096 echo 前缀 · 4104 echo scratch；
//! heap 自 131072 起（页 2），alloc 8 字节对齐向上分配，永不复用。

/// 工具名按首字节路由：'e'/'f'/'t'/'l'/'o'，其余 unknown。
pub(crate) const MAIN_GUEST: &str = r#"
(module
  (import "locowrk" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 4)
  (global $heap (mut i32) (i32.const 131072))
  (data (i32.const 1024) "{\"id\":\"wasm-echo\",\"version\":\"0.1.0\",\"abi_version\":1,\"kind\":\"tool\",\"tools\":[{\"name\":\"echo\",\"description\":\"echo back args\",\"schema\":{\"type\":\"object\"},\"side_effect\":\"Pure\"},{\"name\":\"fail\",\"description\":\"always fails\"},{\"name\":\"trap\",\"description\":\"traps\"},{\"name\":\"loop\",\"description\":\"spins\"},{\"name\":\"oob\",\"description\":\"lies about output\"}]}")
  (data (i32.const 2048) "ping")
  (data (i32.const 2064) "{\"error\":\"boom\"}")
  (data (i32.const 2080) "{\"error\":\"unknown tool\"}")
  (data (i32.const 4096) "{\"echo\":")

  (func $alloc (export "alloc") (param $len i32) (result i32)
    (local $ptr i32)
    (local.set $len (i32.and (i32.add (local.get $len) (i32.const 7)) (i32.const -8)))
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $ptr))

  (func $free (export "free") (param i32 i32))

  (func $describe (export "describe") (result i32) (i32.const 1024))

  (func $init (export "init") (result i32)
    (call $log (i32.const 3) (i32.const 2048) (i32.const 4))
    (i32.const 42))

  (func $shutdown (export "shutdown") (param i32))

  (func $copy (param $src i32) (param $dst i32) (param $len i32)
    (local $i i32)
    (block $done (loop $l
      (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
      (i32.store8 (i32.add (local.get $dst) (local.get $i))
                  (i32.load8_u (i32.add (local.get $src) (local.get $i))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $l))))

  (func $call (export "call") (param $h i32) (param $tp i32) (param $tl i32)
                              (param $ap i32) (param $al i32)
                              (param $opt i32) (param $olt i32) (result i32)
    (local $c i32)
    (local.set $c (i32.load8_u (local.get $tp)))
    (if (i32.eq (local.get $c) (i32.const 101))
      (then
        (call $copy (local.get $ap) (i32.const 4104) (local.get $al))
        (i32.store8 (i32.add (i32.const 4104) (local.get $al)) (i32.const 125))
        (i32.store (local.get $opt) (i32.const 4096))
        (i32.store (local.get $olt) (i32.add (local.get $al) (i32.const 9)))
        (return (i32.const 0))))
    (if (i32.eq (local.get $c) (i32.const 102))
      (then
        (i32.store (local.get $opt) (i32.const 2064))
        (i32.store (local.get $olt) (i32.const 16))
        (return (i32.const -3))))
    (if (i32.eq (local.get $c) (i32.const 116))
      (then (unreachable)))
    (if (i32.eq (local.get $c) (i32.const 108))
      (then (loop $spin (br $spin))))
    (if (i32.eq (local.get $c) (i32.const 111))
      (then
        (i32.store (local.get $opt) (i32.const 4096))
        (i32.store (local.get $olt) (i32.const 1073741824))
        (return (i32.const 0))))
    (i32.store (local.get $opt) (i32.const 2080))
    (i32.store (local.get $olt) (i32.const 24))
    (i32.const -2))
)
"#;

/// describe 先把整页填满 'a'（无 NUL），再返回数据指针。
pub(crate) const NO_NUL_GUEST: &str = r#"
(module
  (memory (export "memory") 1)
  (func $describe (export "describe") (result i32)
    (local $i i32)
    (block $done (loop $l
      (br_if $done (i32.ge_u (local.get $i) (i32.const 65536)))
      (i32.store8 (local.get $i) (i32.const 97))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $l)))
    (i32.const 1024))
  (func $alloc (export "alloc") (param i32) (result i32) (i32.const 0))
  (func $free (export "free") (param i32 i32))
  (func $init (export "init") (result i32) (i32.const 0))
  (func $call (export "call") (param i32 i32 i32 i32 i32 i32 i32) (result i32) (i32.const 0))
  (func $shutdown (export "shutdown") (param i32))
)
"#;

/// 只有 memory + describe：第一个缺的就是 `alloc`。
pub(crate) const MINIMAL_GUEST: &str = r#"
(module
  (memory (export "memory") 1)
  (data (i32.const 1024) "{\"id\":\"wasm-min\",\"version\":\"0.1.0\",\"abi_version\":1,\"kind\":\"tool\",\"tools\":[]}\00")
  (func $describe (export "describe") (result i32) (i32.const 1024))
)
"#;

/// 未被宿主 Linker 提供的 wasi 导入：实例化必须失败（能力面枚举报错）。
pub(crate) const WASI_GUEST: &str = r#"
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
)
"#;

/// 40000 页 = 2.5 GiB：超 Store limiter 内存上限。
pub(crate) const BIGMEM_GUEST: &str = r#"
(module
  (memory (export "memory") 40000)
)
"#;

/// C3 取消查询客户机（specs/04）：`locowrk.call_cancelled` 导入的两个消费
/// 形态。`poll_cancel`（工具名首字节 'p'）自旋轮询到读到 1 才收束——自愿
/// 早停的原型；读到负码 = 调用期身份缺失，错误信封（-3）如实报出。
/// `init_probe`（'i'）把 init 期间（loader 握手、adapter 预置段之外 = 非调用
/// 期）读到的码报出来——三选一静态字符串，wat 免 itoa。
///
/// 内存布局：1024 manifest · 2048 `{"cancelled":true}` · 2176 错误信封 ·
/// 3072/3096/3120 init_code 三选一；heap 自 32768 起，alloc 8 字节对齐。
pub(crate) const CANCEL_GUEST: &str = r#"
(module
  (import "locowrk" "call_cancelled" (func $cancelled (result i32)))
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 32768))
  (global $init_code (mut i32) (i32.const 0))
  (data (i32.const 1024) "{\"id\":\"wasm-cancel\",\"version\":\"0.1.0\",\"abi_version\":1,\"kind\":\"tool\",\"tools\":[{\"name\":\"poll_cancel\",\"description\":\"polls call_cancelled until it reads 1\"},{\"name\":\"init_probe\",\"description\":\"reports the code seen at init\"}]}")
  (data (i32.const 2048) "{\"cancelled\":true}")
  (data (i32.const 2176) "{\"error\":\"call_cancelled unavailable\"}")
  (data (i32.const 3072) "{\"init_code\":-6}")
  (data (i32.const 3096) "{\"init_code\":0}")
  (data (i32.const 3120) "{\"init_code\":1}")

  (func $alloc (export "alloc") (param $len i32) (result i32)
    (local $ptr i32)
    (local.set $len (i32.and (i32.add (local.get $len) (i32.const 7)) (i32.const -8)))
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $ptr))

  (func $free (export "free") (param i32 i32))

  (func $describe (export "describe") (result i32) (i32.const 1024))

  (func $init (export "init") (result i32)
    (global.set $init_code (call $cancelled))
    (i32.const 42))

  (func $shutdown (export "shutdown") (param i32))

  (func $call (export "call") (param $h i32) (param $tp i32) (param $tl i32)
                              (param $ap i32) (param $al i32)
                              (param $opt i32) (param $olt i32) (result i32)
    (local $c i32)
    (local $r i32)
    (local.set $c (i32.load8_u (local.get $tp)))
    (if (i32.eq (local.get $c) (i32.const 112))
      (then
        (loop $poll
          (local.set $r (call $cancelled))
          (if (i32.eq (local.get $r) (i32.const 1))
            (then
              (i32.store (local.get $opt) (i32.const 2048))
              (i32.store (local.get $olt) (i32.const 18))
              (return (i32.const 0))))
          (if (i32.lt_s (local.get $r) (i32.const 0))
            (then
              (i32.store (local.get $opt) (i32.const 2176))
              (i32.store (local.get $olt) (i32.const 38))
              (return (i32.const -3))))
          (br $poll))))
    (if (i32.eq (local.get $c) (i32.const 105))
      (then
        (if (i32.eq (global.get $init_code) (i32.const -6))
          (then
            (i32.store (local.get $opt) (i32.const 3072))
            (i32.store (local.get $olt) (i32.const 16))
            (return (i32.const 0))))
        (if (i32.eq (global.get $init_code) (i32.const 0))
          (then
            (i32.store (local.get $opt) (i32.const 3096))
            (i32.store (local.get $olt) (i32.const 15))
            (return (i32.const 0))))
        (i32.store (local.get $opt) (i32.const 3120))
        (i32.store (local.get $olt) (i32.const 15))
        (return (i32.const 0))))
    (i32.const -2))
)
"#;

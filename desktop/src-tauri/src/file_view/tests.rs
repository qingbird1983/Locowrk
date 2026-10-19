//! `file_view` 的三条边界（specs/08 AC-7）：截断 / 二进制 / 读不到。

use super::read_text_file;
use crate::testutil::{unique_dir, work_dir};

#[test]
fn truncates_over_cap_and_reports_real_size() {
    let dir = work_dir("file-view-truncate");
    let f = dir.join("big.txt");
    std::fs::write(&f, "0123456789abcdef").unwrap();
    let path = f.to_str().unwrap();
    let dto = read_text_file(path, Some(8)).unwrap();
    assert_eq!(dto.text, "01234567", "超上限只回前缀");
    assert_eq!(dto.bytes, 16, "bytes 是文件真实大小，不是读回来的长度");
    assert!(dto.truncated);
    assert!(!dto.binary);
    assert_eq!(dto.path, path, "路径原样回显");
}

#[test]
fn flags_binary_and_withholds_text() {
    let dir = work_dir("file-view-binary");
    let f = dir.join("blob.bin");
    std::fs::write(&f, [0x50u8, 0x4b, 0x00, 0x01]).unwrap();
    let dto = read_text_file(f.to_str().unwrap(), None).unwrap();
    assert!(dto.binary, "前 8 KiB 里有 NUL 就判二进制");
    assert_eq!(dto.text, "");
    assert_eq!(dto.bytes, 4);
    assert!(!dto.truncated);
}

#[test]
fn missing_file_errors_with_plain_words() {
    let missing = unique_dir("file-view-missing").join("nope.txt");
    let err = read_text_file(missing.to_str().unwrap(), None).unwrap_err();
    assert!(err.starts_with("读不到这个文件"), "{err}");
}

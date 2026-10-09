//! 资产存储（批 D，多模态设计 v2.0 §6）：内容寻址 `<数据根>/assets/`，
//! 消息只存 `AssetRef{id, mime, name}` 引用。`id` = 内容 sha256（天然去
//! 重，同图同字节 = wire 编码前缀稳定，M1 前缀缓存纪律同源）；读取带
//! data URL LRU（wire 编码与缩略图渲染两处同吃，重复读盘防闪烁）。

pub mod gc;
pub mod validate;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;
use session_store::AssetRef;

/// data URL LRU 容量（条）。单条上限 maxSizeMb（缺省 5MB）×base64 膨胀
/// ≈6.7MB，16 条封顶 ≈107MB 最坏情形；缩略图渲染多为小图远低于此。
const LRU_CAP: usize = 16;

/// 资产存储（组装点一份，AppState 持有）。
pub struct AssetStore {
    root: PathBuf,
    lru: Mutex<Lru>,
}

struct Lru {
    map: HashMap<String, std::sync::Arc<String>>,
    order: VecDeque<String>,
}

impl Lru {
    fn get(&mut self, k: &str) -> Option<std::sync::Arc<String>> {
        let hit = self.map.get(k).cloned()?;
        // 命中即重排到队尾（Move-to-back，O(n) 但 n≤16 无所谓）
        if let Some(pos) = self.order.iter().position(|x| x == k) {
            self.order.remove(pos);
        }
        self.order.push_back(k.to_string());
        Some(hit)
    }

    fn put(&mut self, k: String, v: std::sync::Arc<String>) {
        if self.map.contains_key(&k) {
            return;
        }
        if self.order.len() >= LRU_CAP {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        self.order.push_back(k.clone());
        self.map.insert(k, v);
    }
}

impl AssetStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lru: Mutex::new(Lru {
                map: HashMap::new(),
                order: VecDeque::new(),
            }),
        }
    }

    /// 存储根（GC 引用面共用）。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 上传即落盘：校验 → sha256 → 写 `<root>/{images,files}/<sha>.<ext>`。
    /// 同内容重复上传 = 幂等（写已存在的同字节文件无害）。
    pub fn put(
        &self,
        bytes: &[u8],
        mime: &str,
        name: &str,
        limits: &validate::Limits,
    ) -> Result<AssetRef, String> {
        let kind = validate::validate(bytes, mime, limits)?;
        let id = sha256_hex(bytes);
        let dir = self.root.join(kind.dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("资产目录创建失败: {e}"))?;
        let path = dir.join(format!("{id}.{}", kind.ext));
        if !path.exists() {
            std::fs::write(&path, bytes).map_err(|e| format!("资产写入失败: {e}"))?;
        }
        Ok(AssetRef {
            id,
            mime: kind.mime.to_string(),
            name: name.to_string(),
        })
    }

    /// 资产文件路径（id + mime 定位；reveal_in_explorer 顶位用）。
    pub fn path_of(&self, id: &str, mime: &str) -> Result<PathBuf, String> {
        let (dir, ext) = kind_of(mime).ok_or_else(|| format!("未知资产类型: {mime}"))?;
        let path = self.root.join(dir).join(format!("{id}.{ext}"));
        Ok(path)
    }

    /// wire 编码用 data URL（LRU 缓存；读失败如实报错——引用悬空说明
    /// 资产被外力清除，发送方应当看到错误而不是静默丢图）。
    pub fn data_url(&self, id: &str, mime: &str) -> Result<String, String> {
        {
            let mut lru = self.lru.lock().expect("asset lru poisoned");
            if let Some(hit) = lru.get(id) {
                return Ok(hit.to_string());
            }
        }
        let path = self.path_of(id, mime)?;
        let bytes =
            std::fs::read(&path).map_err(|e| format!("资产读取失败（{}）: {e}", path.display()))?;
        let url = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        let mut lru = self.lru.lock().expect("asset lru poisoned");
        lru.put(id.to_string(), std::sync::Arc::new(url.clone()));
        Ok(url)
    }
}

/// mime → (子目录, 扩展名)。与 validate 的嗅探白名单一一对应。
pub fn kind_of(mime: &str) -> Option<(&'static str, &'static str)> {
    match mime {
        "image/png" => Some(("images", "png")),
        "image/jpeg" => Some(("images", "jpg")),
        "image/webp" => Some(("images", "webp")),
        "image/gif" => Some(("images", "gif")),
        "application/pdf" => Some(("files", "pdf")),
        _ => None,
    }
}

/// sha256 hex（内容寻址 id）。
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests;

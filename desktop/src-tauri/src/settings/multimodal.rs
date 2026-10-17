//! 多模态设置区（批 D，多模态设计 v2.0 §十一）：上传校验上限 + 孤儿资产
//! GC。字段级缺省零迁移（旧 settings.json 无此区 = 全缺省）；存储位置不
//! 开放配置（固定数据根，走查隔离依赖 LOCOWRK_DATA_DIR）。

use serde::{Deserialize, Serialize};

/// 图片格式白名单缺省（多模态稿 §6.1；转码不做，非白名单拒收如实提示）。
pub const DEFAULT_IMAGE_FORMATS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
/// 文件格式白名单缺省（第一版只收 PDF）。
pub const DEFAULT_FILE_FORMATS: [&str; 1] = ["pdf"];

/// `multimodal` 区。上限值**远严于**官方限制（DeepSeek 单图 32MiB/单边
/// 8192px），留兼容层与 IPC 载荷余量（多模态稿 §7.3 记档）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MultimodalSettings {
    /// 图片单边上限（px）。0 = 不限。
    pub max_dimension: u32,
    /// 上传体积上限（MB）。0 = 不限。
    pub max_size_mb: u64,
    pub image_formats: Vec<String>,
    pub file_formats: Vec<String>,
    /// 孤儿资产自动清理（启动 + 每 24h 后台；手动命令不受此开关限）。
    pub auto_gc: bool,
    /// 孤儿宽限期（天）。0 = 立即过期（走查用）。
    pub gc_threshold_days: u32,
}

impl Default for MultimodalSettings {
    fn default() -> Self {
        Self {
            max_dimension: 2048,
            max_size_mb: 5,
            image_formats: DEFAULT_IMAGE_FORMATS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            file_formats: DEFAULT_FILE_FORMATS.iter().map(|s| s.to_string()).collect(),
            auto_gc: true,
            gc_threshold_days: 7,
        }
    }
}

impl MultimodalSettings {
    /// 保存校验：拒绝不污染内存态（settings update 同款约定）。0 = 不限
    /// 是合法档；上限值拉到「远严于官方」的区间之外没有意义但也无害——
    /// 只挡负数/格式错误，不替用户做可选性判断（约束最小化）。
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.max_dimension > 100_000 {
            return Err("multimodal.maxDimension 出界（0–100000）".into());
        }
        if self.max_size_mb > 512 {
            return Err("multimodal.maxSizeMb 出界（0–512）".into());
        }
        if self.gc_threshold_days > 365 {
            return Err("multimodal.gcThresholdDays 出界（0–365）".into());
        }
        for f in self.image_formats.iter().chain(self.file_formats.iter()) {
            if f.is_empty() || !f.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err("multimodal 白名单格式必须是非空字母数字扩展名".into());
            }
        }
        Ok(())
    }

    /// 校验 + 应用（台账 T15 从 `settings/update.rs` **机械搬移**：两条变换 =
    /// `next.multimodal.` → `self.`、缩进减一层，其余逐字节不动；签名吃消费型值，
    /// 理由与等价性证据同 `plugins.rs`）。落值之后仍由本区 `validate()` 裁决
    /// （"校验体在本区、编排层不复制规则"这条原样保留）。
    pub(crate) fn apply_update(&mut self, u: MultimodalUpdate) -> Result<(), String> {
        // 多模态区（批 D）：数值界 + 白名单格式校验在
        // MultimodalSettings::validate；失败不污染内存态
        if let Some(v) = u.max_dimension {
            self.max_dimension = v;
        }
        if let Some(v) = u.max_size_mb {
            self.max_size_mb = v;
        }
        if let Some(v) = u.image_formats {
            self.image_formats = v.iter().map(|s| s.trim().to_lowercase()).collect();
        }
        if let Some(v) = u.file_formats {
            self.file_formats = v.iter().map(|s| s.trim().to_lowercase()).collect();
        }
        if let Some(v) = u.auto_gc {
            self.auto_gc = v;
        }
        if let Some(v) = u.gc_threshold_days {
            self.gc_threshold_days = v;
        }
        self.validate()?;
        Ok(())
    }
}

/// IPC 投影（无秘钥，全量展示）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MultimodalView {
    pub max_dimension: u32,
    pub max_size_mb: u64,
    pub image_formats: Vec<String>,
    pub file_formats: Vec<String>,
    pub auto_gc: bool,
    pub gc_threshold_days: u32,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MultimodalUpdate {
    pub max_dimension: Option<u32>,
    pub max_size_mb: Option<u64>,
    pub image_formats: Option<Vec<String>>,
    pub file_formats: Option<Vec<String>>,
    pub auto_gc: Option<bool>,
    pub gc_threshold_days: Option<u32>,
}

impl crate::settings::SettingsStore {
    /// 后端专用：多模态区全量（asset_put 校验限 + GC 参数）。
    pub fn multimodal(&self) -> MultimodalSettings {
        self.inner
            .lock()
            .expect("settings poisoned")
            .multimodal
            .clone()
    }
}

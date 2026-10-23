//! IPC 与持久化契约。凭据不属于任何可序列化配置，规则按请求保存快照。
//! 作者：ahui。

use serde::{Deserialize, Serialize};

/// 默认题目仅保留五行正文；前端恢复默认使用同文案，已保存的自定义规则不受影响。
pub const DEFAULT_PROMPT: &str = "请快速按顺序回答：\n1. 仅依据你的记忆，你所知道的最新一位日本首相是谁？\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。";
pub const RULE_VERSION: &str = "literal-contains-v1";

/// 保存的模型配置。has_key 只表示凭据存在，不向前端返回密钥。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
}

/// 题目最多 20000 Unicode 字符，关键词最多 500 字符；不修剪有效空白。
#[derive(Clone, Serialize, Deserialize)]
pub struct TestRule {
    pub prompt: String,
    pub keyword: String,
}

impl Default for TestRule {
    fn default() -> Self {
        Self {
            prompt: DEFAULT_PROMPT.into(),
            keyword: "高市早苗".into(),
        }
    }
}

impl TestRule {
    /// 空白关键词会令所有字符串意外通过，因此保存和执行前都拒绝。
    pub fn validate(&self) -> Result<(), String> {
        if self.prompt.trim().is_empty() || self.prompt.chars().count() > 20_000 {
            return Err("题目必须为 1—20000 个字符，不能只有空白。".into());
        }
        if self.keyword.trim().is_empty() || self.keyword.chars().count() > 500 {
            return Err("判定字符串必须为 1—500 个字符，不能只有空白。".into());
        }
        Ok(())
    }
}

/// 桌面偏好。超时为秒；透明度只用于前端面板背景。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Preferences {
    pub always_on_top: bool,
    pub snap_to_edge: bool,
    pub opacity: u8,
    pub timeout_seconds: u64,
    pub shortcut_enabled: bool,
    /// 默认启用、每三分钟一次；没有选定模型时不会发送请求。
    pub auto_test_enabled: bool,
    /// 分钟数，范围 1—1440；手动测试或保存设置后重新计时。
    pub auto_test_interval_minutes: u64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            always_on_top: true,
            snap_to_edge: true,
            opacity: 97,
            timeout_seconds: 60,
            shortcut_enabled: true,
            auto_test_enabled: true,
            auto_test_interval_minutes: 3,
        }
    }
}

/// 数据库配置结构；首版只有本地用户，无后台账户与遥测。
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub profiles: Vec<ModelProfile>,
    pub selected_profile_id: Option<String>,
    pub rule: TestRule,
    pub preferences: Preferences,
    /// 物理像素坐标，恢复时按当前显示器工作区重新夹紧；旧配置默认为未定位。
    #[serde(default)]
    pub widget_position: Option<(i32, i32)>,
}

/// 历史结果。错误和取消不产生通过/未通过判定，answer 可保留截断正文。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRecord {
    pub id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub model: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub status: String,
    pub answer: String,
    pub error: Option<String>,
    pub prompt: String,
    pub keyword: String,
    pub rule_version: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRun {
    pub id: String,
    pub profile_name: String,
    pub model: String,
    pub started_at: String,
}

/// 所有窗口读取相同快照；running 用于跨窗口阻止重复请求。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub config: AppConfig,
    pub history: Vec<TestRecord>,
    pub running: Option<ActiveRun>,
    /// 自动任务距单调时钟期限的毫秒数；关闭或无有效配置时为 None，不持久化。
    pub auto_test_remaining_ms: Option<u64>,
}

/// 密钥输入只短暂存在于保存流程，不写入数据库或调试格式。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub remove_key: bool,
}

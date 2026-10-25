//! IPC 与持久化契约。凭据不属于任何可序列化配置，规则按请求保存快照。
//! 作者：ahui。

use serde::{Deserialize, Serialize};

/// 默认题目仅保留五行正文；前端恢复默认使用同文案，已保存的自定义规则不受影响。
pub const DEFAULT_PROMPT: &str = "请快速按顺序回答：\n1. 仅依据你的记忆，你所知道的最新一位日本首相是谁？\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。";
pub const RULE_VERSION: &str = "literal-contains-v1";
/// 多答案仍逐项按原文连续匹配；任一命中即可，不能把答案拼接后交给模型。
pub const ANY_RULE_VERSION: &str = "literal-contains-any-v1";
pub const CANDY_RULE_ID: &str = "builtin-candy-test";
/// 用户提供的自带题目原文；只作初始化，不覆盖已保存的修改或删除。
pub const CANDY_PROMPT: &str = "请完成回答下面的问题，答案要求只能由数字和“颗”组成，例如10颗、20颗、30颗；数字与“颗”之间不得有空格：\n\n在一个黑色的袋子里放有三种口味的糖果，每种糖果有两种不同的形状（圆形和五角星形，不同的形状靠手感可以分辨）。现已知不同口味的糖和不同形状的数量统计如下表。参赛者需要在活动前决定摸出的糖果数目，那么，最少取出多少个糖果才能保证手中同时拥有不同形状的苹果味和桃子味的糖？（同时手中有圆形苹果味匹配五角星桃子味糖果，或者有圆形桃子味匹配五角星苹果味糖果都满足要求）\n苹果味 桃子味 西瓜味\n圆形 7 9 8\n五角星形 7 6 4";

/// 新配置默认包含糖果测试，稳定 ID 便于一次性迁移与模型绑定。
pub fn builtin_rule_presets() -> Vec<RulePreset> {
    vec![RulePreset {
        id: CANDY_RULE_ID.into(),
        name: "糖果测试".into(),
        prompt: CANDY_PROMPT.into(),
        keyword: "21颗".into(),
        alternative_keywords: Vec::new(),
    }]
}

/// 默认规则名称只用于展示；旧配置缺字段时命名为日本首相测试，不参与条件摘要。
pub fn default_rule_name() -> String {
    "日本首相测试".into()
}

/// 保存的模型配置。has_key 只表示凭据存在，不向前端返回密钥。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
    /// 本机凭据发生实际变化时生成的新版本标识；不是密钥或其哈希。
    /// 只参与结果条件失效，旧配置为空，备份不能携带此本机状态。
    #[serde(default)]
    pub credential_revision: Option<String>,
    /// 请求协议；旧配置缺省为 Chat Completions，避免升级后改变已有服务行为。
    #[serde(default = "default_interface_mode")]
    pub interface_mode: String,
    /// 旧配置缺字段时关闭；新建表单默认开启。错误最多尝试另一协议一次，共享总超时，不改首选协议。
    #[serde(default)]
    pub protocol_fallback_enabled: bool,
    /// 推理强度；`null` 表示不发送该参数，旧配置升级为默认 low。
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: Option<String>,
    /// 模型级超时（秒）；为空时继承全局偏好。
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// 可选规则预设绑定；空值沿用全局规则。
    #[serde(default)]
    pub rule_preset_id: Option<String>,
    /// 本机模型列表缓存句柄；不参与请求条件，不导出，缺失时仍可手动输入。
    #[serde(default)]
    pub catalog_token: Option<String>,
}

/// 旧配置缺少协议字段时沿用首版请求格式，不能因升级隐式切换到 Responses。
pub fn default_interface_mode() -> String {
    "chat_completions".into()
}

/// 首次创建和旧配置迁移使用 low；显式保存的 None 仍表示不发送强度参数。
pub fn default_reasoning_effort() -> Option<String> {
    Some("low".into())
}

/// 允许的推理档位来自当前 UI 契约；`None` 表示服务商不接受推理参数。
pub fn validate_reasoning_effort(value: Option<&str>) -> Result<(), String> {
    match value {
        None | Some("") | Some("low") | Some("medium") | Some("high") | Some("xhigh") => Ok(()),
        Some(_) => Err("思考强度只能选择 low、medium、high、xhigh 或不指定。".into()),
    }
}

/// 一个题目对应 1—20 个答案，每项最多 500 Unicode 字符；保留有效空白和换行。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRule {
    pub prompt: String,
    pub keyword: String,
    /// 首个答案继续使用 keyword，旧多行字符串不拆分；缺字段等同单答案。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternative_keywords: Vec<String>,
}

/// 可复用的题目与连续字符串规则；历史仍保存实际使用的文本快照。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePreset {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub keyword: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternative_keywords: Vec<String>,
}

impl Default for TestRule {
    fn default() -> Self {
        Self {
            prompt: DEFAULT_PROMPT.into(),
            keyword: "高市早苗".into(),
            alternative_keywords: Vec::new(),
        }
    }
}

impl TestRule {
    /// 空答案会令所有回答通过；保存/导入/执行前均校验，精确重复也拒绝。
    pub fn validate(&self) -> Result<(), String> {
        if self.prompt.trim().is_empty() || self.prompt.chars().count() > 20_000 {
            return Err("题目必须为 1—20000 个字符，不能只有空白。".into());
        }
        if self.alternative_keywords.len() > 19 {
            return Err("每个题目最多设置 20 个判定答案。".into());
        }
        let mut seen = std::collections::HashSet::new();
        for answer in std::iter::once(&self.keyword).chain(&self.alternative_keywords) {
            if answer.trim().is_empty() || answer.chars().count() > 500 {
                return Err("每个判定答案必须为 1—500 个字符，不能只有空白。".into());
            }
            if !seen.insert(answer) {
                return Err("判定答案不能完全重复。".into());
            }
        }
        Ok(())
    }

    /// 仅完整成功的模型回答参与判定；防御性排除空答案，区分大小写，不作 Markdown/语义转换。
    pub fn matches(&self, answer: &str) -> bool {
        std::iter::once(&self.keyword)
            .chain(&self.alternative_keywords)
            .any(|keyword| !keyword.trim().is_empty() && answer.contains(keyword))
    }

    /// 单答案保留首版规则版本，历史可以准确说明当次判定语义。
    pub fn version(&self) -> &'static str {
        if self.alternative_keywords.is_empty() {
            RULE_VERSION
        } else {
            ANY_RULE_VERSION
        }
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
    /// 分钟数，范围 1—1440；成功开始手动测试、开启自动测试或修改间隔时重新计时。
    pub auto_test_interval_minutes: u64,
    /// 历史最多保留条数；None 仅使用 5000 条硬上限，旧配置默认为 500 条。
    pub history_max_records: Option<u64>,
    /// 历史最多保留天数；None 表示不按时间清理。
    pub history_retention_days: Option<u64>,
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
            history_max_records: Some(500),
            history_retention_days: None,
        }
    }
}

/// 数据库配置结构；首版只有本地用户，无后台账户与遥测。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub profiles: Vec<ModelProfile>,
    pub selected_profile_id: Option<String>,
    pub rule: TestRule,
    /// 可改名但不可删除的默认规则，身份仍由空绑定表示，不随名称变化。
    #[serde(default = "default_rule_name")]
    pub default_rule_name: String,
    /// 仅控制自带预设的一次性补入；删除/编辑糖果测试后不能在重启时重新生成或覆盖。
    #[serde(default)]
    pub builtin_rules_version: u8,
    #[serde(default)]
    pub rule_presets: Vec<RulePreset>,
    pub preferences: Preferences,
    /// 物理像素坐标，恢复时按当前显示器工作区重新夹紧；旧配置默认为未定位。
    #[serde(default)]
    pub widget_position: Option<(i32, i32)>,
    /// 邮件配置不包含授权码；凭据由操作系统保管，备份导入不继承密码状态。
    #[serde(default)]
    pub email_notifications: crate::notifications::EmailNotificationSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            selected_profile_id: None,
            rule: TestRule::default(),
            default_rule_name: default_rule_name(),
            rule_presets: builtin_rule_presets(),
            builtin_rules_version: 1,
            preferences: Preferences::default(),
            widget_position: None,
            email_notifications: crate::notifications::EmailNotificationSettings::default(),
        }
    }
}

impl AppConfig {
    /// 首次升级补入自带预设，保留同 ID 的用户内容；标记保存后，删除不会在重启时被撤销。
    /// 此方法只改变内存，调用方继续负责校验和事务持久化。
    pub fn upgrade_builtin_rules(&mut self) {
        if self.builtin_rules_version == 0 {
            if !self
                .rule_presets
                .iter()
                .any(|item| item.id == CANDY_RULE_ID)
            {
                self.rule_presets.extend(builtin_rule_presets());
            }
            if self.default_rule_name == "默认规则" {
                self.default_rule_name = default_rule_name();
            }
            self.builtin_rules_version = 1;
        }
    }
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
    /// 当次请求实际使用的全部替代答案和名称；旧历史保留缺失，不反推当前名称。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternative_keywords: Vec<String>,
    #[serde(default)]
    pub rule_name: Option<String>,
    pub rule_version: String,
    /// 记录写入时的请求来源，旧历史缺失时前端显示“未记录”。
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub interface_mode: Option<String>,
    /// 当次请求是否允许回退及真实尝试顺序；旧历史缺字段保留“未记录”。
    #[serde(default)]
    pub protocol_fallback_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempted_modes: Vec<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub effective_timeout_seconds: Option<u64>,
    /// 对实际请求条件的 SHA-256 摘要；旧历史缺失时不能证明与当前条件相同。
    #[serde(default)]
    pub condition_fingerprint: Option<String>,
    /// 诊断只保存客户端分类与经过约束的服务商编号，不保存鉴权信息或原始错误正文。
    #[serde(default)]
    pub error_kind: Option<String>,
    #[serde(default)]
    pub http_status: Option<u16>,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRun {
    pub id: String,
    pub profile_name: String,
    pub model: String,
    pub started_at: String,
}

/// 所有窗口共享配置与运行状态；历史按窗口所需范围读取，running 用于阻止重复请求。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub config: AppConfig,
    /// 设置页显式读取全部保留历史，悬浮窗只读取当前模型最近一条；均为原文快照。
    pub history: Vec<TestRecord>,
    pub running: Option<ActiveRun>,
    /// 自动任务距单调时钟期限的毫秒数；关闭或无有效配置时为 None，不持久化。
    pub auto_test_remaining_ms: Option<u64>,
    /// 后端按规范化端点及有效规则生成，前端不能凭展示名称猜测旧结果是否仍有效。
    pub condition_fingerprints: std::collections::HashMap<String, String>,
    /// 最近邮件发送故障独立展示，不改写测试结果，也不持久化 SMTP 服务端原文。
    pub email_notification_error: Option<String>,
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
    #[serde(default = "default_interface_mode")]
    pub interface_mode: String,
    /// 兼容旧调用方缺省关闭；当前新建表单显式传 true，编辑已有配置传其保存值，可能产生第二次计费。
    #[serde(default)]
    pub protocol_fallback_enabled: bool,
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub rule_preset_id: Option<String>,
    #[serde(default)]
    pub catalog_token: Option<String>,
}

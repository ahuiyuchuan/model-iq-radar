//! IPC 与持久化契约。凭据不属于任何可序列化配置，规则按请求保存快照。
//! 作者：ahui。

use serde::{Deserialize, Serialize};

/// IPC 的 JavaScript number 必须精确往返；这是存储表示边界，不是业务超时上限。
const MAX_SAFE_TIMEOUT_SECONDS: u64 = 9_007_199_254_740_991;

/// 验证秒数为可精确保存的正整数；规则、全局偏好和请求入口共用，不再限制为 10—300 秒。
/// 超过前端安全整数范围时明确拒绝，避免 JSON 往返悄悄修改用户填写的秒数。
pub fn validate_timeout_seconds(seconds: u64) -> Result<(), String> {
    if seconds == 0 {
        return Err("超时必须填写正整数秒。".into());
    }
    if seconds > MAX_SAFE_TIMEOUT_SECONDS {
        return Err("超时数值超过可精确保存的整数范围，请减小秒数。".into());
    }
    Ok(())
}

/// 按用户原文保留题目与要求之间的空行；只迁移完全一致的旧内置正文，不覆盖自定义题目。
pub const DEFAULT_PROMPT: &str = "仅依据你的记忆，你所知道的最新一位日本首相是谁？\n\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。";
const LEGACY_DEFAULT_PROMPT: &str = "请快速按顺序回答：\n1. 仅依据你的记忆，你所知道的最新一位日本首相是谁？\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。";
pub const RULE_VERSION: &str = "literal-contains-v1";
/// 多答案仍逐项按原文连续匹配；任一命中即可，不能把答案拼接后交给模型。
pub const ANY_RULE_VERSION: &str = "literal-contains-any-v1";
pub const CANDY_RULE_ID: &str = "builtin-candy-test";
pub const PELICAN_RULE_ID: &str = "builtin-pelican-test";
/// 自带规则补入版本独立于 SQLite schema；v2 增加鹈鹕题目并精确迁移旧默认正文。
pub const BUILTIN_RULES_VERSION: u8 = 2;
/// “禁止联网/读文件”是发给被测模型的题目要求，不是客户端或开发工具的操作指令。
pub const PELICAN_PROMPT: &str = "创建一个HTML，内容是SVG绘制一个黑天鹅骑自行车的2D动画，你不需要任何测试。\n\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。";
/// 用户提供的自带题目原文；只作初始化，不覆盖已保存的修改或删除。
pub const CANDY_PROMPT: &str = "请完成回答下面的问题，答案要求只能由数字和“颗”组成，例如10颗、20颗、30颗；数字与“颗”之间不得有空格：\n\n在一个黑色的袋子里放有三种口味的糖果，每种糖果有两种不同的形状（圆形和五角星形，不同的形状靠手感可以分辨）。现已知不同口味的糖和不同形状的数量统计如下表。参赛者需要在活动前决定摸出的糖果数目，那么，最少取出多少个糖果才能保证手中同时拥有不同形状的苹果味和桃子味的糖？（同时手中有圆形苹果味匹配五角星桃子味糖果，或者有圆形桃子味匹配五角星苹果味糖果都满足要求）\n苹果味 桃子味 西瓜味\n圆形 7 9 8\n五角星形 7 6 4";

/// 新配置包含糖果和鹈鹕测试；HTML 识别先于关键词判定，备用关键词不评价动画质量。
pub fn builtin_rule_presets() -> Vec<RulePreset> {
    vec![
        RulePreset {
            id: CANDY_RULE_ID.into(),
            name: "糖果测试".into(),
            prompt: CANDY_PROMPT.into(),
            keyword: "21颗".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        },
        RulePreset {
            id: PELICAN_RULE_ID.into(),
            name: "鹈鹕测试".into(),
            prompt: PELICAN_PROMPT.into(),
            // 沿用现有非空答案契约；仅非 HTML 回答走普通文本匹配，HTML 始终中性展示、不发信。
            keyword: "<svg".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        },
    ]
}

/// 两个固定 ID 不占用 100 个自定义预设配额；启动、保存及备份必须使用同一规则。
pub fn is_builtin_rule_id(id: &str) -> bool {
    matches!(id, CANDY_RULE_ID | PELICAN_RULE_ID)
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
    /// 仅兼容读取 v1.0.2 及以前的接口超时；v1.0.3 起不生效、不再序列化。
    /// 同题可绑定多个接口且旧值可能冲突，因此统一采用规则/全局值，不做隐式合并。
    #[serde(default, skip_serializing)]
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
    /// 本题请求总超时（正整数秒）；None 继承全局值，旧数据缺字段时同样继承。
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
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
    /// 该题所有绑定模型共用的总超时；None 继承悬浮设置，不读取旧接口覆盖值。
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for TestRule {
    fn default() -> Self {
        Self {
            prompt: DEFAULT_PROMPT.into(),
            keyword: "高市早苗".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        }
    }
}

impl TestRule {
    /// 空答案会令所有回答通过；保存/导入/执行前均校验，精确重复也拒绝。
    pub fn validate(&self) -> Result<(), String> {
        if let Some(seconds) = self.timeout_seconds {
            validate_timeout_seconds(seconds)?;
        }
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
/// 已移除全局显示/隐藏快捷键；旧 shortcutEnabled 由 serde 忽略，保存和备份不再写出。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Preferences {
    pub always_on_top: bool,
    pub snap_to_edge: bool,
    pub opacity: u8,
    /// 全局总超时，正整数秒，默认 300；仅在规则未覆盖时使用，校验 IPC 精度边界。
    pub timeout_seconds: u64,
    /// 默认启用、每三分钟一次；没有选定模型时不会发送请求。
    pub auto_test_enabled: bool,
    /// 分钟数，范围 1—1440；手动/自动测试结算后等待完整间隔，开启或修改间隔也重新计时。
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
            timeout_seconds: 300,
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
    /// 控制自带预设的一次性补入；已完成对应版本后，删除或编辑不在重启时被撤销。
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
            builtin_rules_version: BUILTIN_RULES_VERSION,
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
                self.rule_presets.extend(
                    builtin_rule_presets()
                        .into_iter()
                        .filter(|item| item.id == CANDY_RULE_ID),
                );
            }
            if self.default_rule_name == "默认规则" {
                self.default_rule_name = default_rule_name();
            }
            self.builtin_rules_version = 1;
        }
        if self.builtin_rules_version < BUILTIN_RULES_VERSION {
            if !self
                .rule_presets
                .iter()
                .any(|item| item.id == PELICAN_RULE_ID)
            {
                self.rule_presets.extend(
                    builtin_rule_presets()
                        .into_iter()
                        .filter(|item| item.id == PELICAN_RULE_ID),
                );
            }
            if self.rule.prompt == LEGACY_DEFAULT_PROMPT {
                self.rule.prompt = DEFAULT_PROMPT.into();
            }
            self.builtin_rules_version = BUILTIN_RULES_VERSION;
        }
    }
}

/// 两个管理入口共用同一清空语义；只接受五类显式选择，空选择由命令拒绝。
/// 不包含“关于”、外部备份文件或用户导出的 EXE，避免清理范围越界。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataResetSelection {
    /// 接口分组、模型目录缓存与对应系统 API Key。
    pub api: bool,
    /// 默认题答、全部预设及模型的规则绑定。
    pub rules: bool,
    /// 桌面偏好、保存的位置及本程序的系统自启项。
    pub widget: bool,
    /// 测试正文、最近结果索引及通知恢复判断依据。
    pub history: bool,
    /// 邮箱设置、授权码及通知恢复判断依据。
    pub mail: bool,
}

impl DataResetSelection {
    /// 重建所选配置分类，其他字段原样保留；数据库/凭据及运行时副作用由命令协调提交。
    pub fn reset_config(&self, current: &AppConfig) -> Result<AppConfig, String> {
        if !(self.api || self.rules || self.widget || self.history || self.mail) {
            return Err("请至少选择一个需要清空的分类。".into());
        }
        let mut config = current.clone();
        if self.api {
            config.profiles.clear();
            config.selected_profile_id = None;
        }
        if self.rules {
            config.rule = TestRule::default();
            config.default_rule_name = default_rule_name();
            config.rule_presets = builtin_rule_presets();
            config.builtin_rules_version = BUILTIN_RULES_VERSION;
            // 清空规则会重建内置预设；初始题目选糖果，接口凭据/模型和旧历史不随此操作改变。
            for profile in &mut config.profiles {
                profile.rule_preset_id = Some(CANDY_RULE_ID.into());
            }
        }
        if self.widget {
            config.preferences = Preferences::default();
            config.widget_position = None;
        }
        if self.mail {
            config.email_notifications = crate::notifications::EmailNotificationSettings::default();
        }
        Ok(config)
    }

    /// 明确清空分类时从未知通知状态重新开始；普通历史页“清空历史”仍保留恢复依据。
    pub fn resets_notification_state(&self) -> bool {
        self.api || self.history || self.mail
    }
}

/// 历史结果。HTML 成功结果使用中性 `html` 状态，不产生通过/未通过判定；错误和取消
/// 仍保留原状态，answer 可保留截断正文用于排查。
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
    /// 新记录的自动识别结果；旧历史缺失时前端按正文临时识别，但不改写旧结论。
    #[serde(default)]
    pub answer_type: Option<String>,
    /// 识别规则版本，便于未来调整启发式时保留历史语义。
    #[serde(default)]
    pub answer_parsing_version: Option<String>,
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
    /// 默认只读取当前模型最近一条原文；显式完整读取仅供导出与旧调用方使用。
    pub history: Vec<TestRecord>,
    /// 实际保留集合的轻量版本；列表仅在集合变化时重读摘要，不随每次运行状态广播读取全文。
    pub history_revision: String,
    pub running: Option<ActiveRun>,
    /// 自动任务距单调时钟期限的毫秒数；测试中、关闭或无有效配置时为 None，不持久化。
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
    pub rule_preset_id: Option<String>,
    #[serde(default)]
    pub catalog_token: Option<String>,
}

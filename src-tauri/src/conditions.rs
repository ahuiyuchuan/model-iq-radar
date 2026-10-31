//! 请求条件与配置验证的共同入口。名字和外观不参与条件摘要，实际请求参数参与。
//! 摘要不能代替鉴权，也不包含密钥；旧历史没有摘要时必须提示重测。作者：ahui。

use crate::{api, model::*};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// 解析当前配置实际使用的规则；悬空绑定是配置错误，不能静默改用默认题目。
pub fn effective_rule(config: &AppConfig, profile: &ModelProfile) -> Result<TestRule, String> {
    rule_for_binding(config, profile.rule_preset_id.as_deref())
}

/// 正式请求和草稿连接测试共用已保存规则解析；None 沿用默认，悬空 ID 必须明确报错。
pub fn rule_for_binding(config: &AppConfig, preset_id: Option<&str>) -> Result<TestRule, String> {
    let rule = match preset_id {
        Some(id) => {
            let preset = config
                .rule_presets
                .iter()
                .find(|preset| preset.id == id)
                .ok_or("模型绑定的规则预设不存在，请重新选择规则后保存。")?;
            TestRule {
                prompt: preset.prompt.clone(),
                keyword: preset.keyword.clone(),
                alternative_keywords: preset.alternative_keywords.clone(),
                timeout_seconds: preset.timeout_seconds,
            }
        }
        None => config.rule.clone(),
    };
    rule.validate()?;
    Ok(rule)
}

/// 标准化实际协议端点，既用于条件比较，也用于阻止把已有密钥隐式发送到新地址。
pub fn normalized_endpoint(profile: &ModelProfile) -> Result<String, String> {
    match profile.interface_mode.as_str() {
        "chat_completions" => api::endpoint(&profile.base_url).map(|url| url.to_string()),
        "responses" => api::responses_endpoint(&profile.base_url).map(|url| url.to_string()),
        _ => Err("接口模式必须是 Chat Completions 或 Responses。".into()),
    }
}

/// 固定字段顺序生成跨进程稳定的条件键。保留题目/关键词有效空白，且不包含展示名称。
pub fn condition_fingerprint(config: &AppConfig, profile: &ModelProfile) -> Result<String, String> {
    #[derive(Serialize)]
    struct Conditions<'a> {
        version: u8,
        endpoint: String,
        model: &'a str,
        credential_revision: Option<&'a str>,
        prompt: String,
        keyword: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        alternative_keywords: Vec<String>,
        interface_mode: &'a str,
        #[serde(skip_serializing_if = "fallback_disabled")]
        protocol_fallback_enabled: bool,
        reasoning_effort: Option<&'a str>,
        effective_timeout_seconds: u64,
    }
    let rule = effective_rule(config, profile)?;
    let timeout = rule
        .timeout_seconds
        .unwrap_or(config.preferences.timeout_seconds);
    let conditions = Conditions {
        // 改用版本 2：同一网关地址更换凭据也可能切换账户或上游，必须重新测试。
        // 保存随机版本而非密钥摘要，避免把可用于离线猜测凭据的信息写入历史。
        // 关闭回退时单答案保持版本 2、多答案保持版本 3；显式启用回退使用版本 4。
        // 升级不能使未启用新策略的结果无故失效，开关变化则必须提示重测。
        version: if profile.protocol_fallback_enabled {
            4
        } else if rule.alternative_keywords.is_empty() {
            2
        } else {
            3
        },
        endpoint: normalized_endpoint(profile)?,
        model: &profile.model,
        credential_revision: profile.credential_revision.as_deref(),
        prompt: rule.prompt,
        keyword: rule.keyword,
        alternative_keywords: rule.alternative_keywords,
        interface_mode: &profile.interface_mode,
        protocol_fallback_enabled: profile.protocol_fallback_enabled,
        reasoning_effort: profile
            .reasoning_effort
            .as_deref()
            .filter(|value| !value.is_empty()),
        effective_timeout_seconds: timeout,
    };
    let bytes = serde_json::to_vec(&conditions).map_err(|_| "请求条件摘要生成失败。")?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// 关闭回退时不新增序列化字段，保证已有 v2/v3 条件摘要不因升级无故失效。
fn fallback_disabled(value: &bool) -> bool {
    !value
}

/// 持久化 ID 只允许安全短标识；新 ID 由 UUID 生成，导入不能注入无限长对象键。
pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("配置与规则 ID 必须为 1—80 位字母、数字、横线或下划线。".into());
    }
    Ok(())
}

/// 所有保存/导入入口复用的偏好边界；None 表示不使用相应用户限制，硬上限始终存在。
pub fn validate_preferences(preferences: &Preferences) -> Result<(), String> {
    validate_timeout_seconds(preferences.timeout_seconds)?;
    if !(96..=192).contains(&preferences.skin.size)
        || !crate::pet_window::valid_skin_id(&preferences.skin.id)
    {
        return Err("皮肤标识无效或角色尺寸不在 96—192 像素范围。".into());
    }
    // 允许皮肤契约内的全部动作作为空闲外观；仅保存名称，不执行该动作对应的业务事件。
    if !matches!(
        preferences.skin.idle_action.as_str(),
        "idle"
            | "petting"
            | "dragging"
            | "working"
            | "success"
            | "seated"
            | "wave"
            | "stretch"
            | "landing"
            | "bow"
            | "failed"
            | "error"
    ) {
        return Err("默认待机动作无效，请选择皮肤支持的动作。".into());
    }

    if !(80..=100).contains(&preferences.opacity)
        || !(1..=1440).contains(&preferences.auto_test_interval_minutes)
        || preferences
            .history_max_records
            .is_some_and(|count| !(1..=5000).contains(&count))
        || preferences
            .history_retention_days
            .is_some_and(|days| !(1..=3650).contains(&days))
    {
        return Err("偏好设置超出允许范围：透明度 80—100、自动间隔 1—1440 分钟、历史 1—5000 条或 1—3650 天。".into());
    }
    Ok(())
}

/// 备份/导入验证完整对象和新版本数量上限，禁止生成不可重新导入的备份或部分落盘。
pub fn validate_config(config: &AppConfig) -> Result<(), String> {
    if config.profiles.len() > 100 {
        return Err("备份及导入最多包含 100 个模型配置；已有超额旧配置可继续使用，请减少数量后再备份或导入。".into());
    }
    validate_stored_config(config)
}

/// 校验本机已保存配置的可执行性；首版未限制模型数量，不能用新增上限阻止旧库启动
/// 或编辑。新建入口另行限制数量；字段、引用和协议仍完整验证，不放宽损坏数据边界。
pub(crate) fn validate_stored_config(config: &AppConfig) -> Result<(), String> {
    validate_preferences(&config.preferences)?;
    config.rule.validate()?;
    if config.default_rule_name.trim().is_empty() || config.default_rule_name.chars().count() > 80 {
        return Err("默认规则名称必须为 1—80 个字符。".into());
    }
    if config
        .rule_presets
        .iter()
        .filter(|item| !is_builtin_rule_id(&item.id))
        .count()
        > 100
    {
        return Err("最多保存 100 个自定义规则预设。".into());
    }
    let mut preset_ids = HashSet::new();
    for preset in &config.rule_presets {
        validate_id(&preset.id)?;
        if !preset_ids.insert(&preset.id) {
            return Err("规则预设 ID 重复。".into());
        }
        if preset.name.trim().is_empty() || preset.name.chars().count() > 80 {
            return Err("规则预设名称必须为 1—80 个字符。".into());
        }
        TestRule {
            prompt: preset.prompt.clone(),
            keyword: preset.keyword.clone(),
            alternative_keywords: preset.alternative_keywords.clone(),
            timeout_seconds: preset.timeout_seconds,
        }
        .validate()?;
    }
    let mut profile_ids = HashSet::new();
    for profile in &config.profiles {
        validate_id(&profile.id)?;
        if let Some(revision) = &profile.credential_revision {
            validate_id(revision)?;
        }
        if !profile_ids.insert(&profile.id) {
            return Err("模型配置 ID 重复。".into());
        }
        if profile.name.trim().is_empty()
            || profile.name.chars().count() > 80
            || profile.model.trim().is_empty()
            || profile.model.chars().count() > 200
            || profile.base_url.len() > 2000
        {
            return Err("配置名称、模型名称或接口地址不符合长度要求。".into());
        }
        normalized_endpoint(profile)?;
        validate_reasoning_effort(profile.reasoning_effort.as_deref())?;
        effective_rule(config, profile)?;
    }
    if config
        .selected_profile_id
        .as_ref()
        .is_some_and(|id| !profile_ids.contains(id))
    {
        return Err("选中的模型配置不存在。".into());
    }
    Ok(())
}

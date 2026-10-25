//! 无凭据的配置备份与合并计划。预览/准备阶段不写 SQLite、不读取系统凭据。
//! 调用方必须先应用可回滚的运行时偏好，再将完整计划在一个事务中提交。作者：ahui。

use crate::{
    conditions::{normalized_endpoint, validate_config},
    model::AppConfig,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// 备份限额与配置数量限制共同生效，避免导入超大文件或递归对象拖慢桌面进程。
pub const BACKUP_MAX_BYTES: usize = 4 * 1024 * 1024;

/// 版本 2 支持默认规则改名与多答案；读取兼容版本 1，旧程序会拒绝新版本而非静默丢答案。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDocument {
    pub version: u32,
    pub exported_at: String,
    pub config: AppConfig,
    #[serde(default)]
    pub note: String,
}

/// 导入前的只读摘要；重复数按稳定 ID 计算，预览不授权覆盖或历史清理。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub profiles_count: usize,
    pub presets_count: usize,
    pub duplicate_profiles: usize,
    pub duplicate_presets: usize,
    pub profiles_missing_keys: usize,
    pub warnings: Vec<String>,
}

/// 原子导入完成后的数量统计；缺凭据数量基于合并后的最终配置。
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added_profiles: usize,
    pub updated_profiles: usize,
    pub skipped_profiles: usize,
    pub added_presets: usize,
    pub updated_presets: usize,
    pub skipped_presets: usize,
    pub profiles_missing_keys: usize,
    /// 事务已提交，但原生窗口恢复失败；只提示，不表示导入失败或需要重新提交。
    pub warning: Option<String>,
}

/// 完整、已验证的提交计划；保留字段公开以供 IPC 集成事务与运行时回滚。
pub struct PreparedImport {
    pub config: AppConfig,
    pub report: ImportReport,
}

/// 导出只克隆可序列化配置，密钥不在类型中；存在标记归零，坐标留空便于跨电脑恢复。
pub fn export_backup(config: &AppConfig) -> Result<BackupDocument, String> {
    // 本机可保留首版超过 100 个的旧配置，但备份必须符合导入的相同数量/字段边界。
    validate_config(config)?;
    let mut config = config.clone();
    for profile in &mut config.profiles {
        profile.has_key = false;
        profile.credential_revision = None;
        profile.catalog_token = None;
    }
    config.email_notifications.has_password = false;
    config.widget_position = None;
    let document = BackupDocument {
        version: 2,
        exported_at: Utc::now().to_rfc3339(),
        config,
        note: "不包含 API Key 或邮箱授权码；导入后须补填凭据，邮件通知默认关闭。重复项按稳定 ID 判断。".into(),
    };
    // 大量合法长规则可能超过单个备份的资源上限。下载前明确拒绝，不能生成
    // 应用自身无法导入的文件；按与前端下载一致的两空格缩进核验，避免边界处
    // 紧凑 JSON 合法而下载文件因格式化膨胀超过前端 4 MiB 文件限制。
    if serde_json::to_vec_pretty(&document)
        .map_err(|_| "配置备份序列化失败。")?
        .len()
        > BACKUP_MAX_BYTES
    {
        return Err("当前配置备份超过 4 MiB，未导出；请减少规则数量或题目长度后重试。".into());
    }
    Ok(document)
}

/// 即使额外字段会被 serde 忽略，也拒绝看起来包含凭据的文件，避免误把不安全文件当备份。
fn reject_secrets(value: &Value, depth: usize) -> Result<(), String> {
    if depth > 16 {
        return Err("备份嵌套过深。".into());
    }
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let normalized = key.replace(['_', '-'], "").to_ascii_lowercase();
                if matches!(
                    normalized.as_str(),
                    "apikey"
                        | "password"
                        | "smtppassword"
                        | "authorization"
                        | "authorizationcode"
                        | "accesstoken"
                        | "refreshtoken"
                ) {
                    return Err(
                        "备份包含不允许导入的凭据字段，请移除 API Key 和邮箱授权码后重试。".into(),
                    );
                }
                reject_secrets(value, depth + 1)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                reject_secrets(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn read_backup(backup: &Value) -> Result<AppConfig, String> {
    if serde_json::to_vec(backup)
        .map_err(|_| "备份序列化失败。")?
        .len()
        > BACKUP_MAX_BYTES
    {
        return Err("配置备份不能超过 4 MiB。".into());
    }
    reject_secrets(backup, 0)?;
    let document: BackupDocument =
        serde_json::from_value(backup.clone()).map_err(|_| "配置备份格式无效。")?;
    if !matches!(document.version, 1 | 2) {
        return Err("不支持此备份版本，请使用匹配的应用版本。".into());
    }
    let mut config = document.config;
    config.upgrade_builtin_rules();
    validate_config(&config)?;
    crate::notifications::validate_settings(&config.email_notifications)?;
    // hasKey/hasPassword 仅为本机状态，不信任外部 JSON 的宣称；邮件必须重新确认凭据。
    for profile in &mut config.profiles {
        profile.has_key = false;
        profile.credential_revision = None;
        profile.catalog_token = None;
    }
    config.email_notifications.enabled = false;
    config.email_notifications.has_password = false;
    Ok(config)
}

/// 只读摘要先展示数量、重复与覆盖边界；实际提交再次解析和验证，预览不能绕过校验。
pub fn preview_import(current: &AppConfig, backup: &Value) -> Result<ImportPreview, String> {
    let imported = read_backup(backup)?;
    let duplicate_profiles = imported
        .profiles
        .iter()
        .filter(|profile| {
            current
                .profiles
                .iter()
                .any(|existing| existing.id == profile.id)
        })
        .count();
    let duplicate_presets = imported
        .rule_presets
        .iter()
        .filter(|preset| {
            current
                .rule_presets
                .iter()
                .any(|existing| existing.id == preset.id)
        })
        .count();
    Ok(ImportPreview {
        profiles_count: imported.profiles.len(),
        presets_count: imported.rule_presets.len(),
        duplicate_profiles,
        duplicate_presets,
        profiles_missing_keys: imported.profiles.len(),
        warnings: vec![
            "全局测试规则、偏好与邮件配置将采用备份值；邮件通知导入后关闭。".into(),
            "重复项按 ID 判断：跳过保留现有内容，替换更新内容，复制创建新 ID 并重建规则绑定。"
                .into(),
            "新建/复制的模型没有 API Key；替换有密钥的配置不能改变实际请求地址，请改用复制。"
                .into(),
        ],
    })
}

/// 生成原子提交计划。稳定 ID 负责重复判断，copy 分配全新 ID 并同步映射预设引用。
/// replace 只在实际请求端点不变时保留现有凭据标记，防止导入地址接收旧 API Key。
pub fn prepare_import(
    current: &AppConfig,
    backup: &Value,
    duplicate_strategy: &str,
) -> Result<PreparedImport, String> {
    if !matches!(duplicate_strategy, "skip" | "replace" | "copy") {
        return Err("重复配置处理方式只能是 skip、replace 或 copy。".into());
    }
    let imported = read_backup(backup)?;
    let mut config = current.clone();
    let mut report = ImportReport::default();
    let mut preset_map = HashMap::new();
    for mut preset in imported.rule_presets {
        let old_id = preset.id.clone();
        let existing = config
            .rule_presets
            .iter()
            .position(|item| item.id == preset.id);
        if duplicate_strategy == "copy" {
            preset.id = Uuid::new_v4().to_string();
        }
        preset_map.insert(old_id, preset.id.clone());
        match (existing, duplicate_strategy) {
            (Some(_), "skip") => report.skipped_presets += 1,
            (Some(index), "replace") => {
                config.rule_presets[index] = preset;
                report.updated_presets += 1;
            }
            _ => {
                config.rule_presets.push(preset);
                report.added_presets += 1;
            }
        }
    }
    let mut profile_map = HashMap::new();
    for mut profile in imported.profiles {
        let old_id = profile.id.clone();
        let existing = config
            .profiles
            .iter()
            .position(|item| item.id == profile.id);
        if duplicate_strategy == "copy" {
            profile.id = Uuid::new_v4().to_string();
        }
        profile_map.insert(old_id, profile.id.clone());
        if let Some(id) = &profile.rule_preset_id {
            profile.rule_preset_id =
                Some(preset_map.get(id).ok_or("导入的规则绑定无效。")?.clone());
        }
        match (existing, duplicate_strategy) {
            (Some(_), "skip") => report.skipped_profiles += 1,
            (Some(index), "replace") => {
                let previous = &config.profiles[index];
                if previous.has_key
                    && normalized_endpoint(previous)? != normalized_endpoint(&profile)?
                {
                    return Err("替换操作改变了已有密钥配置的请求地址。请使用复制模式，并为新配置补填密钥。".into());
                }
                profile.has_key = previous.has_key;
                // replace 沿用的是本机同目标的凭据，不能采用备份中的版本宣称。
                profile.credential_revision = previous.credential_revision.clone();
                // 缓存是本机非权威建议；仅保留同一实际地址的原句柄，不导入外部句柄。
                profile.catalog_token =
                    if normalized_endpoint(previous)? == normalized_endpoint(&profile)? {
                        previous.catalog_token.clone()
                    } else {
                        None
                    };
                config.profiles[index] = profile;
                report.updated_profiles += 1;
            }
            _ => {
                profile.has_key = false;
                config.profiles.push(profile);
                report.added_profiles += 1;
            }
        }
    }
    config.rule = imported.rule;
    config.default_rule_name = imported.default_rule_name;
    config.builtin_rules_version = imported.builtin_rules_version;
    config.preferences = imported.preferences;
    config.email_notifications = imported.email_notifications;
    if let Some(selected) = imported.selected_profile_id {
        config.selected_profile_id = profile_map.get(&selected).cloned();
    }
    // 当前显示器坐标不随备份覆盖；完整结果条件由后端重新生成，旧历史不重新判定。
    validate_config(&config)?;
    report.profiles_missing_keys = config
        .profiles
        .iter()
        .filter(|profile| !profile.has_key)
        .count();
    Ok(PreparedImport { config, report })
}

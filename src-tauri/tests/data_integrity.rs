//! 条件失效与安全迁移回归。只构造本地配置，不连接服务商或操作系统凭据。作者：ahui。

use model_probe_lib::{
    backup, conditions,
    model::{AppConfig, ModelProfile, RulePreset},
    storage::Store,
};
use serde_json::json;

/// 外观不参与模型条件；旧配置、v3 备份和分类重置共同覆盖新增嵌套字段。
#[test]
fn skin_defaults_backup_reset_and_fingerprint_remain_compatible() {
    use model_probe_lib::model::{DataResetSelection, Preferences, SkinPreferences};
    let mut config = config();
    let before = conditions::condition_fingerprint(&config, &config.profiles[0]).unwrap();
    let mut old = serde_json::to_value(&config.preferences).unwrap();
    old.as_object_mut().unwrap().remove("skin");
    let migrated: Preferences = serde_json::from_value(old).unwrap();
    assert_eq!(migrated.skin.id, "classic");
    config.preferences.skin = SkinPreferences {
        id: "test-pet".into(),
        size: 192,
        animation: false,
        bubbles: false,
        autonomous: false,
        idle_action: "error".into(),
    };
    assert_eq!(
        conditions::condition_fingerprint(&config, &config.profiles[0]).unwrap(),
        before
    );
    let backup = backup::export_backup(&config).unwrap();
    let roundtrip: backup::BackupDocument =
        serde_json::from_value(serde_json::to_value(backup).unwrap()).unwrap();
    assert_eq!(roundtrip.config.preferences.skin.id, "test-pet");
    assert!(!roundtrip.config.preferences.skin.animation);
    assert_eq!(roundtrip.config.preferences.skin.idle_action, "error");
    assert!(conditions::validate_preferences(&roundtrip.config.preferences).is_ok());
    // 真正关闭并重新打开隔离数据库，验证重启恢复；不读取正式配置或系统凭据。
    let path = std::env::temp_dir().join(format!("model-probe-idle-{}.db", uuid::Uuid::new_v4()));
    {
        let mut store = Store::open(&path).unwrap();
        store.save_config(config.clone()).unwrap();
    }
    assert_eq!(
        Store::open(&path)
            .unwrap()
            .config
            .preferences
            .skin
            .idle_action,
        "error"
    );
    std::fs::remove_file(path).unwrap();
    let reset = DataResetSelection {
        api: false,
        rules: false,
        widget: true,
        history: false,
        mail: false,
    }
    .reset_config(&config)
    .unwrap();
    assert_eq!(reset.preferences.skin.id, "classic");
    assert_eq!(reset.preferences.skin.idle_action, "idle");
    assert_eq!(reset.profiles[0].id, config.profiles[0].id);
    config.preferences.skin.size = 95;
    assert!(conditions::validate_preferences(&config.preferences).is_err());
    config.preferences.skin.size = 144;
    for action in [
        "idle", "petting", "dragging", "working", "success", "seated", "wave", "stretch",
        "landing", "bow", "failed", "error",
    ] {
        config.preferences.skin.idle_action = action.into();
        assert!(conditions::validate_preferences(&config.preferences).is_ok());
    }
    config.preferences.skin.idle_action = "unknown".into();
    assert!(conditions::validate_preferences(&config.preferences).is_err());
}

/// 新开关默认关闭，关闭时旧摘要不变；开启改变条件，备份和配置往返保留显式策略。
#[test]
fn protocol_fallback_defaults_off_invalidates_conditions_and_roundtrips_backup() {
    let mut config = config();
    let original = conditions::condition_fingerprint(&config, &config.profiles[0]).unwrap();
    let mut old = serde_json::to_value(&config.profiles[0]).unwrap();
    old.as_object_mut()
        .unwrap()
        .remove("protocolFallbackEnabled");
    let old_profile: ModelProfile = serde_json::from_value(old).unwrap();
    assert!(!old_profile.protocol_fallback_enabled);
    assert_eq!(
        conditions::condition_fingerprint(&config, &old_profile).unwrap(),
        original
    );
    config.profiles[0].protocol_fallback_enabled = true;
    assert_ne!(
        conditions::condition_fingerprint(&config, &config.profiles[0]).unwrap(),
        original
    );
    let document = backup::export_backup(&config).unwrap();
    let roundtrip: backup::BackupDocument =
        serde_json::from_slice(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(roundtrip.config.profiles[0].protocol_fallback_enabled);
    assert!(!roundtrip.config.profiles[0].has_key);
}

/// 兼容旧备份中的快捷键字段；忽略旧开关但保留其他偏好，再导出不携带已删除功能。
#[test]
fn legacy_shortcut_backup_is_ignored_without_changing_other_preferences() {
    for version in [1, 2, 3] {
        for enabled in [false, true] {
            let mut current = config();
            current.preferences.opacity = 88;
            let mut document =
                serde_json::to_value(backup::export_backup(&current).unwrap()).unwrap();
            document["version"] = json!(version);
            document["config"]["preferences"]["shortcutEnabled"] = json!(enabled);
            let imported =
                backup::prepare_import(&AppConfig::default(), &document, "replace").unwrap();
            assert_eq!(imported.config.preferences.opacity, 88);
            let exported =
                serde_json::to_value(backup::export_backup(&imported.config).unwrap()).unwrap();
            assert!(exported["config"]["preferences"]
                .get("shortcutEnabled")
                .is_none());
            assert_eq!(exported["version"], 3);
        }
    }
}

/// 多答案使用原文 OR，不发送或拼接判定答案；旧多行 keyword 无损保留。
#[test]
fn multiple_answers_preserve_literal_semantics_and_reject_invalid_items() {
    use model_probe_lib::model::{TestRule, ANY_RULE_VERSION, RULE_VERSION};
    let mut rule: TestRule =
        serde_json::from_value(json!({"prompt":"题", "keyword":" A\nB "})).unwrap();
    assert_eq!(rule.version(), RULE_VERSION);
    assert!(!rule.matches("A\nB"));
    assert!(rule.matches("前 A\nB 后"));
    rule.alternative_keywords = vec!["21颗".into()];
    assert!(rule.validate().is_ok());
    assert!(rule.matches("21颗"));
    assert!(!rule.matches("21 颗"));
    assert_eq!(rule.version(), ANY_RULE_VERSION);
    rule.alternative_keywords.push("\n".into());
    assert!(rule.validate().is_err());
    rule.alternative_keywords = vec![rule.keyword.clone()];
    assert!(rule.validate().is_err());
    rule.alternative_keywords = (0..20).map(|index| index.to_string()).collect();
    assert!(rule.validate().is_err());
}

#[test]
fn multi_answer_fingerprint_backup_and_default_name_roundtrip() {
    let mut current = config();
    let old_key = conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap();
    current.rule_presets[0].alternative_keywords = vec!["21颗".into()];
    let key = conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap();
    assert_ne!(old_key, key);
    current.default_rule_name = "默认改名".into();
    current.rule_presets[0].name = "预设改名".into();
    assert_eq!(
        key,
        conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap()
    );
    let document = serde_json::to_value(backup::export_backup(&current).unwrap()).unwrap();
    assert_eq!(document["version"], 3);
    let imported = backup::prepare_import(&AppConfig::default(), &document, "replace").unwrap();
    assert_eq!(imported.config.default_rule_name, "默认改名");
    assert_eq!(
        imported
            .config
            .rule_presets
            .iter()
            .find(|item| item.id == "rule-one")
            .unwrap()
            .alternative_keywords,
        ["21颗"]
    );
    let mut old_document = document;
    old_document["version"] = json!(1);
    old_document["config"]
        .as_object_mut()
        .unwrap()
        .remove("defaultRuleName");
    old_document["config"]
        .as_object_mut()
        .unwrap()
        .remove("builtinRulesVersion");
    old_document["config"]["rulePresets"][0]
        .as_object_mut()
        .unwrap()
        .remove("alternativeKeywords");
    let imported = backup::prepare_import(&AppConfig::default(), &old_document, "replace").unwrap();
    assert_eq!(imported.config.default_rule_name, "日本首相测试");
    assert_eq!(
        imported
            .config
            .rule_presets
            .iter()
            .filter(|item| item.id == model_probe_lib::model::CANDY_RULE_ID)
            .count(),
        1
    );
}

#[test]
fn builtin_rules_upgrade_once_without_overwriting_user_changes() {
    use model_probe_lib::model::{CANDY_PROMPT, CANDY_RULE_ID};
    let mut current = AppConfig::default();
    assert_eq!(current.default_rule_name, "日本首相测试");
    let candy = &mut current.rule_presets[0];
    assert_eq!(candy.name, "糖果测试");
    assert_eq!(candy.keyword, "21颗");
    assert_eq!(candy.prompt, CANDY_PROMPT);
    candy.name = "用户改名".into();
    current.builtin_rules_version = 0;
    current.upgrade_builtin_rules();
    assert_eq!(current.rule_presets[0].name, "用户改名");
    assert_eq!(
        current
            .rule_presets
            .iter()
            .filter(|item| item.id == CANDY_RULE_ID)
            .count(),
        1
    );
    current.rule_presets.clear();
    current.upgrade_builtin_rules();
    assert!(current.rule_presets.is_empty());
}

/// 一次性规则迁移必须在启动事务内落盘；之后用户删除/修改和重复启动都不能重新补入。
#[test]
fn builtin_rule_migration_persists_once_and_rolls_back_failed_write() {
    use model_probe_lib::model::{BUILTIN_RULES_VERSION, DEFAULT_PROMPT, PELICAN_RULE_ID};
    let path =
        std::env::temp_dir().join(format!("model-probe-builtin-{}.db", uuid::Uuid::new_v4()));
    let mut legacy = AppConfig {
        builtin_rules_version: 1,
        ..AppConfig::default()
    };
    legacy
        .rule_presets
        .retain(|rule| rule.id != PELICAN_RULE_ID);
    legacy.rule.prompt = "仅依据你的记忆，你所知道的最新一位日本首相是谁？".into();
    // 自定义题目并不等于旧内置全文，升级不能替用户重写。
    let custom_prompt = legacy.rule.prompt.clone();
    {
        let mut store = Store::open(&path).unwrap();
        store.save_config(legacy.clone()).unwrap();
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_builtin_migration BEFORE UPDATE ON app_config BEGIN SELECT RAISE(FAIL, 'fixture'); END;").unwrap();
    assert!(Store::open(&path)
        .err()
        .unwrap()
        .contains("内置规则迁移保存失败"));
    let payload: String = connection
        .query_row("SELECT payload FROM app_config WHERE id=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let unchanged: AppConfig = serde_json::from_str(&payload).unwrap();
    assert_eq!(unchanged.builtin_rules_version, 1);
    assert!(!unchanged
        .rule_presets
        .iter()
        .any(|rule| rule.id == PELICAN_RULE_ID));
    connection
        .execute_batch("DROP TRIGGER reject_builtin_migration;")
        .unwrap();
    {
        let store = Store::open(&path).unwrap();
        assert_eq!(store.config.builtin_rules_version, BUILTIN_RULES_VERSION);
        assert_eq!(store.config.rule.prompt, custom_prompt);
        assert_ne!(store.config.rule.prompt, DEFAULT_PROMPT);
        assert!(store
            .config
            .rule_presets
            .iter()
            .any(|rule| rule.id == PELICAN_RULE_ID));
    }
    {
        let mut store = Store::open(&path).unwrap();
        let mut deleted = store.config.clone();
        deleted
            .rule_presets
            .retain(|rule| rule.id != PELICAN_RULE_ID);
        store.save_config(deleted).unwrap();
    }
    assert!(!Store::open(&path)
        .unwrap()
        .config
        .rule_presets
        .iter()
        .any(|rule| rule.id == PELICAN_RULE_ID));
    drop(connection);
    std::fs::remove_file(path).unwrap();
}

/// 使用隔离 SQLite 文件验证重启缓存、端点隔离与更新替换，绝不访问真实凭据或服务商。
#[test]
fn model_catalog_persists_isolates_endpoints_and_updates_atomically() {
    let path =
        std::env::temp_dir().join(format!("model-probe-catalog-{}.db", uuid::Uuid::new_v4()));
    {
        let mut store = Store::open(&path).unwrap();
        let models: Vec<_> =
            serde_json::from_value(json!([{"id":"first","ownedBy":null}])).unwrap();
        store
            .save_model_catalog("fixture-catalog", "https://example.com/v1/models", &models)
            .unwrap();
    }
    {
        let mut store = Store::open(&path).unwrap();
        assert_eq!(
            store
                .model_catalog("fixture-catalog", "https://example.com/v1/models")
                .unwrap()[0]
                .id,
            "first"
        );
        assert!(store
            .model_catalog("fixture-catalog", "https://other.example/v1/models")
            .unwrap()
            .is_empty());
        let models: Vec<_> =
            serde_json::from_value(json!([{"id":"second","ownedBy":"provider"}])).unwrap();
        store
            .save_model_catalog("fixture-catalog", "https://example.com/v1/models", &models)
            .unwrap();
        assert_eq!(
            store
                .model_catalog("fixture-catalog", "https://example.com/v1/models")
                .unwrap()[0]
                .id,
            "second"
        );
        let too_big = vec![models[0].clone(); 5001];
        assert!(store
            .save_model_catalog("fixture-catalog", "https://example.com/v1/models", &too_big)
            .is_err());
        assert_eq!(
            store
                .model_catalog("fixture-catalog", "https://example.com/v1/models")
                .unwrap()[0]
                .id,
            "second"
        );
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        3
    );
    drop(connection);
    std::fs::remove_file(&path).unwrap();
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

fn config() -> AppConfig {
    AppConfig {
        profiles: vec![ModelProfile {
            id: "profile-one".into(),
            name: "分组一".into(),
            base_url: "https://example.com/v1".into(),
            model: "gpt-6".into(),
            has_key: true,
            credential_revision: Some("local-credential-revision".into()),
            interface_mode: "chat_completions".into(),
            protocol_fallback_enabled: false,
            reasoning_effort: Some("low".into()),
            timeout_seconds: None,
            rule_preset_id: Some("rule-one".into()),
            catalog_token: None,
        }],
        selected_profile_id: Some("profile-one".into()),
        rule_presets: vec![RulePreset {
            id: "rule-one".into(),
            name: "预设".into(),
            prompt: " 问题\n".into(),
            keyword: " 高市早苗 ".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        }],
        ..AppConfig::default()
    }
}

#[test]
fn condition_key_tracks_actual_parameters_and_preserves_whitespace() {
    let original = config();
    let key = conditions::condition_fingerprint(&original, &original.profiles[0]).unwrap();
    assert!(key.starts_with("sha256:"));
    assert!(!key.contains("问题"));
    let mut renamed = original.clone();
    renamed.profiles[0].name = "改名".into();
    renamed.rule_presets[0].name = "预设改名".into();
    renamed.preferences.opacity = 90;
    renamed.profiles[0].base_url = "https://example.com/v1/chat/completions".into();
    assert_eq!(
        key,
        conditions::condition_fingerprint(&renamed, &renamed.profiles[0]).unwrap()
    );
    for change in 0..8 {
        let mut changed = original.clone();
        match change {
            0 => changed.profiles[0].base_url = "https://other.example/v1".into(),
            1 => changed.profiles[0].model = "gpt-6-new".into(),
            2 => changed.rule_presets[0].prompt = "问题\n".into(),
            3 => changed.rule_presets[0].keyword = "高市早苗".into(),
            4 => changed.profiles[0].interface_mode = "responses".into(),
            5 => changed.profiles[0].reasoning_effort = None,
            6 => changed.preferences.timeout_seconds = 120,
            _ => changed.profiles[0].credential_revision = Some("replaced-credential".into()),
        }
        assert_ne!(
            key,
            conditions::condition_fingerprint(&changed, &changed.profiles[0]).unwrap()
        );
    }
    let mut explicit = original.clone();
    explicit.profiles[0].timeout_seconds = Some(explicit.preferences.timeout_seconds);
    assert_eq!(
        key,
        conditions::condition_fingerprint(&explicit, &explicit.profiles[0]).unwrap()
    );
}

#[test]
fn dangling_rules_duplicate_ids_and_invalid_protocols_are_rejected() {
    let mut invalid = config();
    invalid.rule_presets.clear();
    assert!(conditions::effective_rule(&invalid, &invalid.profiles[0]).is_err());
    assert!(conditions::validate_config(&invalid).is_err());
    invalid = config();
    invalid.profiles.push(invalid.profiles[0].clone());
    assert!(conditions::validate_config(&invalid).is_err());
    invalid = config();
    invalid.profiles[0].interface_mode = "unknown".into();
    assert!(conditions::validate_config(&invalid).is_err());
}

/// 可解析但不可执行的本地配置必须在启动前阻止调度器；拒绝不能修复、清理或覆盖用户数据库。
#[test]
fn startup_rejects_invalid_persisted_config_without_rewriting_it() {
    let path = std::env::temp_dir().join(format!(
        "model-probe-invalid-config-{}.db",
        uuid::Uuid::new_v4()
    ));
    {
        let mut store = Store::open(&path).unwrap();
        let mut invalid = config();
        invalid.profiles[0].interface_mode = "unsupported-protocol".into();
        // save_config is deliberately a low-level persistence primitive; startup must defend
        // against this and external tools that leave JSON structurally valid but unusable.
        store.save_config(invalid).unwrap();
    }

    let error = match Store::open(&path) {
        Ok(_) => panic!("invalid persisted configuration must prevent startup"),
        Err(error) => error,
    };
    assert!(error.contains("本地配置无效"));
    let connection = rusqlite::Connection::open(&path).unwrap();
    let payload: String = connection
        .query_row("SELECT payload FROM app_config WHERE id=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let saved: AppConfig = serde_json::from_str(&payload).unwrap();
    assert_eq!(saved.profiles[0].interface_mode, "unsupported-protocol");
    drop(connection);
    std::fs::remove_file(&path).unwrap();
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

/// v1.0.0 没有配置数量上限；新增限制不能阻止合法旧库启动，也不能改写原配置。
#[test]
fn startup_preserves_legacy_profiles_above_the_new_creation_limit() {
    let path = std::env::temp_dir().join(format!(
        "model-probe-legacy-capacity-{}.db",
        uuid::Uuid::new_v4()
    ));
    let mut legacy = serde_json::to_value(AppConfig::default()).unwrap();
    legacy["profiles"] = json!((0..101)
        .map(|index| json!({
            "id": format!("legacy-{index}"), "name": format!("旧配置 {index}"),
            "baseUrl": "https://example.com/v1", "model": "fixture", "hasKey": false
        }))
        .collect::<Vec<_>>());
    legacy["selectedProfileId"] = json!("legacy-100");
    let payload = serde_json::to_string(&legacy).unwrap();
    {
        // 仅创建首版两张表，确保测试同时经过真实的旧 schema 初始化路径。
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE app_config (id INTEGER PRIMARY KEY, payload TEXT NOT NULL);
            CREATE TABLE test_record (sequence INTEGER PRIMARY KEY AUTOINCREMENT, payload TEXT NOT NULL);
            PRAGMA user_version=1;").unwrap();
        connection
            .execute("INSERT INTO app_config VALUES(1,?1)", [&payload])
            .unwrap();
    }
    {
        let store = Store::open(&path).unwrap();
        assert_eq!(store.config.profiles.len(), 101);
        assert_eq!(
            store.config.selected_profile_id.as_deref(),
            Some("legacy-100")
        );
        assert_eq!(
            store.config.profiles[100].interface_mode,
            "chat_completions"
        );
        assert_eq!(
            store.config.profiles[100].reasoning_effort.as_deref(),
            Some("low")
        );
        // 备份仍遵守可重新导入的数量边界，不能生成不可用文件。
        assert!(backup::export_backup(&store.config).is_err());
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    let saved: String = connection
        .query_row("SELECT payload FROM app_config WHERE id=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(saved, payload);
    drop(connection);
    std::fs::remove_file(&path).unwrap();
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

#[test]
fn backup_copy_remaps_ids_and_never_imports_credential_claims() {
    let original = config();
    let backup = serde_json::to_value(backup::export_backup(&original).unwrap()).unwrap();
    assert_eq!(backup["config"]["profiles"][0]["hasKey"], false);
    assert!(backup["config"]["profiles"][0]["credentialRevision"].is_null());
    assert_eq!(backup["config"]["emailNotifications"]["hasPassword"], false);
    assert!(backup.to_string().find("apiKey").is_none());
    let preview = backup::preview_import(&original, &backup).unwrap();
    assert_eq!(preview.duplicate_profiles, 1);
    let copied = backup::prepare_import(&original, &backup, "copy").unwrap();
    assert_eq!(copied.config.profiles.len(), 2);
    assert_eq!(copied.config.rule_presets.len(), 2);
    assert_ne!(copied.config.profiles[1].id, original.profiles[0].id);
    assert_eq!(
        copied.config.profiles[1].rule_preset_id.as_deref(),
        Some(copied.config.rule_presets[1].id.as_str())
    );
    assert!(!copied.config.profiles[1].has_key);
    assert!(copied.config.profiles[1].credential_revision.is_none());
    assert!(copied.config.profiles[0].has_key);
    assert_eq!(
        copied.config.selected_profile_id.as_deref(),
        Some(copied.config.profiles[1].id.as_str())
    );
    assert_eq!(original.profiles.len(), 1);
}

#[test]
fn backup_replace_guards_credentials_and_skip_keeps_existing_objects() {
    let original = config();
    let mut backup = serde_json::to_value(backup::export_backup(&original).unwrap()).unwrap();
    backup["config"]["profiles"][0]["name"] = json!("导入改名");
    let replaced = backup::prepare_import(&original, &backup, "replace").unwrap();
    assert!(replaced.config.profiles[0].has_key);
    assert_eq!(
        replaced.config.profiles[0].credential_revision,
        original.profiles[0].credential_revision
    );
    assert_eq!(replaced.config.profiles[0].name, "导入改名");
    let skipped = backup::prepare_import(&original, &backup, "skip").unwrap();
    assert_eq!(skipped.report.skipped_profiles, 1);
    assert_eq!(skipped.config.profiles[0].name, "分组一");
    backup["config"]["profiles"][0]["baseUrl"] = json!("https://attacker.example/v1");
    assert!(backup::prepare_import(&original, &backup, "replace").is_err());
    assert!(backup::prepare_import(&original, &backup, "copy").is_ok());
    backup["config"]["profiles"][0]["apiKey"] = json!("never-import-this");
    assert!(backup::preview_import(&original, &backup).is_err());
}

#[test]
fn backup_rejects_invalid_whole_document_before_planning() {
    let original = config();
    let mut backup = serde_json::to_value(backup::export_backup(&original).unwrap()).unwrap();
    backup["config"]["preferences"]["timeoutSeconds"] = json!(0);
    assert!(backup::prepare_import(&original, &backup, "copy").is_err());
    backup["version"] = json!(999);
    assert!(backup::preview_import(&original, &backup).is_err());
    assert_eq!(original.profiles.len(), 1);
}

/// 备份的本机凭据版本是不可信声明，不能让新配置继承旧结果或假装拥有凭据。
#[test]
fn imported_credential_revision_is_discarded_and_old_profiles_remain_compatible() {
    let original = config();
    let mut document = serde_json::to_value(backup::export_backup(&original).unwrap()).unwrap();
    document["config"]["profiles"][0]["credentialRevision"] = json!("untrusted-claim");
    let imported = backup::prepare_import(&AppConfig::default(), &document, "skip").unwrap();
    assert!(imported.config.profiles[0].credential_revision.is_none());
    assert!(!imported.config.profiles[0].has_key);
    let mut legacy = serde_json::to_value(&original.profiles[0]).unwrap();
    legacy.as_object_mut().unwrap().remove("credentialRevision");
    let profile: ModelProfile = serde_json::from_value(legacy).unwrap();
    assert!(profile.credential_revision.is_none());
}

/// 合法的长规则组合也可能超过单文件上限；必须在生成下载前明确报错。
#[test]
fn export_rejects_oversized_document_instead_of_creating_unimportable_backup() {
    let mut large = AppConfig::default();
    for index in 0..100 {
        large.rule_presets.push(RulePreset {
            id: format!("rule-{index}"),
            name: format!("预设 {index}"),
            prompt: "题".repeat(20_000),
            keyword: "答案".into(),
            alternative_keywords: Vec::new(),
            timeout_seconds: None,
        });
    }
    conditions::validate_config(&large).unwrap();
    let error = match backup::export_backup(&large) {
        Ok(_) => panic!("oversized backup must fail before download"),
        Err(error) => error,
    };
    assert!(error.contains("4 MiB"));
    assert_eq!(large.rule_presets.len(), 102);
}

/// 同题共享覆盖值；旧接口值彻底忽略，等效超时相同不使历史无故失效。
#[test]
fn rule_timeout_inherits_global_ignores_legacy_profiles_and_roundtrips() {
    let mut current = config();
    current.preferences.timeout_seconds = 60;
    let original = conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap();
    current.profiles[0].timeout_seconds = Some(10);
    assert_eq!(
        original,
        conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap()
    );
    current.rule_presets[0].timeout_seconds = Some(60);
    assert_eq!(
        original,
        conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap()
    );
    current.rule_presets[0].timeout_seconds = Some(120);
    assert_eq!(
        conditions::effective_rule(&current, &current.profiles[0])
            .unwrap()
            .timeout_seconds,
        Some(120)
    );
    let overridden = conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap();
    assert_ne!(original, overridden);
    current.preferences.timeout_seconds = 300;
    assert_eq!(
        overridden,
        conditions::condition_fingerprint(&current, &current.profiles[0]).unwrap()
    );
    current.rule.timeout_seconds = Some(90);
    let document = serde_json::to_value(backup::export_backup(&current).unwrap()).unwrap();
    assert_eq!(document["version"], 3);
    assert!(document["config"]["profiles"][0]
        .get("timeoutSeconds")
        .is_none());
    let imported = backup::prepare_import(&AppConfig::default(), &document, "replace").unwrap();
    assert_eq!(imported.config.rule.timeout_seconds, Some(90));
    assert_eq!(
        conditions::effective_rule(&imported.config, &imported.config.profiles[0])
            .unwrap()
            .timeout_seconds,
        Some(120)
    );
    // v1/v2 备份缺新字段：保留显式全局值，接口旧覆盖不迁移、不再导出。
    for version in [1, 2] {
        let mut legacy = document.clone();
        legacy["version"] = json!(version);
        legacy["config"]["preferences"]["timeoutSeconds"] = json!(60);
        legacy["config"]["rule"]
            .as_object_mut()
            .unwrap()
            .remove("timeoutSeconds");
        legacy["config"]["rulePresets"][0]
            .as_object_mut()
            .unwrap()
            .remove("timeoutSeconds");
        legacy["config"]["profiles"][0]["timeoutSeconds"] = json!(20);
        let migrated = backup::prepare_import(&AppConfig::default(), &legacy, "replace").unwrap();
        assert_eq!(migrated.config.preferences.timeout_seconds, 60);
        assert_eq!(
            conditions::effective_rule(&migrated.config, &migrated.config.profiles[0])
                .unwrap()
                .timeout_seconds,
            None
        );
        assert!(
            serde_json::to_value(backup::export_backup(&migrated.config).unwrap()).unwrap()
                ["config"]["profiles"][0]
                .get("timeoutSeconds")
                .is_none()
        );
    }
    // 只拒绝非正数和无法通过 IPC 精确往返的整数，不再采用 10—300 的业务范围。
    for value in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(9_007_199_254_740_992_u64),
        json!(u64::MAX),
    ] {
        let mut invalid = document.clone();
        invalid["config"]["rulePresets"][0]["timeoutSeconds"] = value.clone();
        assert!(backup::prepare_import(&AppConfig::default(), &invalid, "replace").is_err());
        invalid = document.clone();
        invalid["config"]["preferences"]["timeoutSeconds"] = value;
        assert!(backup::prepare_import(&AppConfig::default(), &invalid, "replace").is_err());
    }
    for value in [1, 9, 300, 301, 600, 3600, 9_007_199_254_740_991] {
        current.rule.timeout_seconds = Some(value);
        current.rule_presets[0].timeout_seconds = Some(value);
        current.preferences.timeout_seconds = value;
        assert!(current.rule.validate().is_ok());
        let exported = serde_json::to_value(backup::export_backup(&current).unwrap()).unwrap();
        let imported = backup::prepare_import(&AppConfig::default(), &exported, "replace").unwrap();
        assert_eq!(imported.config.rule.timeout_seconds, Some(value));
        // 导入保留目标库的自带预设顺序，须按绑定核对目标题目，不能假设它位于首项。
        assert_eq!(
            conditions::effective_rule(&imported.config, &imported.config.profiles[0])
                .unwrap()
                .timeout_seconds,
            Some(value)
        );
        assert_eq!(imported.config.preferences.timeout_seconds, value);
    }
}

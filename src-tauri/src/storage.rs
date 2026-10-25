//! 单连接 SQLite 存储。配置更新与历史裁剪使用事务；调用者持有进程内互斥锁。
//! 数据库不存密钥，损坏时报告错误，绝不静默重建覆盖原数据。作者：ahui。

use crate::{
    conditions::{validate_preferences, validate_stored_config},
    model::{AppConfig, Preferences, TestRecord},
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

/// 限制保留的历史 JSON 正文总字节数；SQLite 已分配页面/WAL 不等于逻辑正文大小。
/// 128 MiB 与 5000 条硬上限同时生效，避免长回答使常驻窗口占用失控。
pub const HISTORY_STORAGE_LIMIT_BYTES: u64 = 128 * 1024 * 1024;
pub const HISTORY_HARD_MAX_RECORDS: u64 = 5000;

/// 按真实数据库计算的保留影响，单位为条数和 UTF-8 JSON 正文字节；不等于文件大小。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCleanupPreview {
    pub removed_count: u64,
    pub remaining_count: u64,
    pub removed_bytes: u64,
    pub remaining_bytes: u64,
    pub storage_limit_bytes: u64,
}

struct CleanupPlan {
    sequences: Vec<i64>,
    preview: HistoryCleanupPreview,
}

/// 单连接存储及提交后内存快照；调用方必须用同一应用互斥锁串行访问。
/// 只有 SQLite 写入成功后才更新 config，失败不允许内存提前显示新设置。
pub struct Store {
    connection: Connection,
    pub config: AppConfig,
}

/// 以序号保留最新记录，时间按 UTC 解析，不能比较包含不同时区偏移的日期字符串。
/// 逐条解析后仅保留元数据，避免为清理同时复制全部长回答；损坏记录必须报错并回滚。
fn cleanup_plan(connection: &Connection, preferences: &Preferences) -> Result<CleanupPlan, String> {
    cleanup_plan_at(
        connection,
        preferences,
        Utc::now(),
        HISTORY_STORAGE_LIMIT_BYTES,
    )
}

/// 注入时刻与字节预算供确定性回归验证；生产入口始终使用当前 UTC 与固定硬上限。
fn cleanup_plan_at(
    connection: &Connection,
    preferences: &Preferences,
    now: DateTime<Utc>,
    storage_limit: u64,
) -> Result<CleanupPlan, String> {
    validate_preferences(preferences)?;
    let max_records = preferences
        .history_max_records
        .unwrap_or(HISTORY_HARD_MAX_RECORDS);
    let cutoff = preferences
        .history_retention_days
        .map(|days| now - Duration::days(days as i64));
    let mut statement = connection
        .prepare("SELECT sequence,payload FROM test_record ORDER BY sequence DESC")
        .map_err(|_| "无法查询历史清理范围。")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| "无法查询历史清理范围。")?;
    let mut plan = CleanupPlan {
        sequences: Vec::new(),
        preview: HistoryCleanupPreview {
            removed_count: 0,
            remaining_count: 0,
            removed_bytes: 0,
            remaining_bytes: 0,
            storage_limit_bytes: storage_limit,
        },
    };
    let mut size_exhausted = false;
    for row in rows {
        let (sequence, payload) = row.map_err(|_| "历史记录读取失败，未执行清理。")?;
        let record: TestRecord = serde_json::from_str(&payload)
            .map_err(|_| "历史记录格式无效；请先备份数据库，未执行清理。")?;
        let expired = match cutoff {
            Some(cutoff) => {
                DateTime::parse_from_rfc3339(&record.started_at)
                    .map_err(|_| "历史记录时间无效；请先备份数据库，未执行清理。")?
                    .with_timezone(&Utc)
                    < cutoff
            }
            None => false,
        };
        let bytes = payload.len() as u64;
        if !expired && plan.preview.remaining_bytes.saturating_add(bytes) > storage_limit {
            size_exhausted = true;
        }
        if expired || plan.preview.remaining_count >= max_records || size_exhausted {
            plan.sequences.push(sequence);
            plan.preview.removed_count += 1;
            plan.preview.removed_bytes += bytes;
        } else {
            plan.preview.remaining_count += 1;
            plan.preview.remaining_bytes += bytes;
        }
    }
    Ok(plan)
}

fn apply_cleanup(connection: &Connection, plan: &CleanupPlan) -> Result<(), String> {
    let mut statement = connection
        .prepare("DELETE FROM test_record WHERE sequence=?1")
        .map_err(|_| "无法准备历史清理。")?;
    for sequence in &plan.sequences {
        statement
            .execute([sequence])
            .map_err(|_| "历史记录清理失败，操作已回滚。")?;
    }
    Ok(())
}

/// 从已保留的 JSON 历史重建每个模型的最近序号。历史表有意不拆列，避免迁移时
/// 复制不可信正文；这个派生索引只存 profile ID 和 SQLite 序号，绝不存回答内容。
fn rebuild_latest_record_index(connection: &Connection) -> Result<(), String> {
    connection
        .execute("DELETE FROM latest_test_record", [])
        .map_err(|_| "无法重建最近测试索引。")?;
    let mut statement = connection
        .prepare("SELECT sequence,payload FROM test_record ORDER BY sequence DESC")
        .map_err(|_| "无法重建最近测试索引。")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| "无法重建最近测试索引。")?;
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let (sequence, payload) = row.map_err(|_| "历史记录读取失败，未重建最近测试索引。")?;
        let record: TestRecord = serde_json::from_str(&payload)
            .map_err(|_| "历史记录格式无效；请先备份数据库，未重建最近测试索引。")?;
        if seen.insert(record.profile_id.clone()) {
            upsert_latest_record_index(connection, &record.profile_id, sequence)?;
        }
    }
    Ok(())
}

/// 写入新记录且没有任何裁剪时，仅更新该模型的一项索引，避免全表重复解析。
fn upsert_latest_record_index(
    connection: &Connection,
    profile_id: &str,
    sequence: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO latest_test_record(profile_id,sequence) VALUES(?1,?2)
             ON CONFLICT(profile_id) DO UPDATE SET sequence=excluded.sequence",
            params![profile_id, sequence],
        )
        .map_err(|_| "无法更新最近测试索引。")?;
    Ok(())
}

/// 只有减少用户保留范围才要求额外确认；已接受的过期/硬上限清理属于既有策略。
pub fn retention_tightened(previous: &Preferences, next: &Preferences) -> bool {
    next.history_max_records.unwrap_or(HISTORY_HARD_MAX_RECORDS)
        < previous
            .history_max_records
            .unwrap_or(HISTORY_HARD_MAX_RECORDS)
        || match (previous.history_retention_days, next.history_retention_days) {
            (None, Some(_)) => true,
            (Some(previous), Some(next)) => next < previous,
            _ => false,
        }
}

impl Store {
    /// 读取首版配置并升级至 schema v2；旧程序拒绝新版本，防止多答案被忽略。
    /// 启动时执行已经保存的保留策略，因此长期未测试也不会把过期历史继续展示出来。
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection =
            Connection::open(path).map_err(|_| "无法打开本地数据库。请检查目录权限。")?;
        connection
            .busy_timeout(std::time::Duration::from_secs(3))
            .map_err(|_| "无法设置数据库等待超时。")?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| "无法读取数据库版本。")?;
        if version > 2 {
            return Err("数据库来自更新版本，请使用对应版本应用。".into());
        }
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS app_config (id INTEGER PRIMARY KEY CHECK(id = 1), payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS test_record (sequence INTEGER PRIMARY KEY AUTOINCREMENT, payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS latest_test_record (profile_id TEXT PRIMARY KEY, sequence INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS notification_state (id INTEGER PRIMARY KEY CHECK(id = 1), payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS model_catalog (token TEXT PRIMARY KEY, endpoint TEXT NOT NULL, payload TEXT NOT NULL, updated_at TEXT NOT NULL);"
        ).map_err(|_| "初始化数据库失败，已有数据未被删除。")?;
        let mut config: AppConfig =
            match connection.query_row("SELECT payload FROM app_config WHERE id=1", [], |row| {
                row.get::<_, String>(0)
            }) {
                Ok(payload) => serde_json::from_str(&payload)
                    .map_err(|_| "本地配置格式无效，请先备份数据库。")?,
                Err(rusqlite::Error::QueryReturnedNoRows) => AppConfig::default(),
                Err(_) => return Err("读取本地配置失败。".into()),
            };
        // 配置文件可能被旧版、手工工具或意外中断写入为可解析但不可执行的 JSON。
        // 必须在启动调度器前验证完整请求条件：若只检查保留偏好，选中的失效模型会在
        // 自动期限到达后每秒重新尝试本地校验，既制造无意义工作，也不能给用户明确修复路径。
        // 校验失败不写回配置正文、不清理历史；前面仅初始化兼容附加表和 WAL，
        // 保留原始业务数据供用户备份或使用兼容版本修复。
        // 在内存补入一次性预设，下一次显式保存时持久化标记；不改写原题答或原数据库 payload。
        // 旧库已有同 ID 时视为已补入，不能覆盖其自定义内容；容量为 100 自定义 + 1 自带规则。
        config.upgrade_builtin_rules();
        validate_stored_config(&config)
            .map_err(|message| format!("本地配置无效，请先备份数据库后修复：{message}"))?;
        let transaction = connection
            .transaction()
            .map_err(|_| "无法开始历史检查事务。")?;
        let plan = cleanup_plan(&transaction, &config.preferences)?;
        apply_cleanup(&transaction, &plan)?;
        // 旧 schema 的 history 只有 JSON 正文，没有可索引的 profile_id 列。启动时已经
        // 逐条校验正文并执行保留策略，因此在同一事务中重建一个本地派生索引，不改变
        // 历史内容；本轮格式升级单独记录为版本 2。悬浮窗只需读取当前模型的一条记录。
        rebuild_latest_record_index(&transaction)?;
        // 多答案改变判定语义，旧程序必须拒绝新数据库，不能忽略额外答案后静默降级判定。
        // 仅完整验证/历史整理成功后随事务升级；回退旧程序须恢复升级前数据库备份。
        transaction
            .pragma_update(None, "user_version", 2)
            .map_err(|_| "数据库版本更新失败。")?;
        transaction.commit().map_err(|_| "历史整理提交失败。")?;
        Ok(Self { connection, config })
    }

    /// 单行 UPSERT 成功后才替换内存配置；普通模型/外观保存不触发额外历史裁剪。
    pub fn save_config(&mut self, config: AppConfig) -> Result<(), String> {
        let payload = serde_json::to_string(&config).map_err(|_| "配置序列化失败。")?;
        self.connection.execute("INSERT INTO app_config(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![payload]).map_err(|_| "配置保存失败，请检查磁盘空间和权限。")?;
        self.config = config;
        Ok(())
    }

    /// 成功列表才原子更新，失败事务保留旧缓存；最多 100 份、总正文 16 MiB。
    /// 随机 token 不含 API Key 或其哈希，缓存只作选择建议，不改变配置或测试结果。
    pub fn save_model_catalog(
        &mut self,
        token: &str,
        endpoint: &str,
        models: &[crate::discovery::FetchedModel],
    ) -> Result<(), String> {
        crate::conditions::validate_id(token)?;
        let payload = serde_json::to_string(models).map_err(|_| "模型列表缓存序列化失败。")?;
        if models.len() > 5000 || payload.len() > 1024 * 1024 {
            return Err("模型列表缓存超过上限。".into());
        }
        let tx = self
            .connection
            .transaction()
            .map_err(|_| "无法开始模型列表缓存事务。")?;
        tx.execute("INSERT INTO model_catalog(token,endpoint,payload,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(token) DO UPDATE SET endpoint=excluded.endpoint,payload=excluded.payload,updated_at=excluded.updated_at", params![token, endpoint, payload, Utc::now().to_rfc3339()]).map_err(|_| "模型列表缓存保存失败，原列表保留。")?;
        // 只保留最新完整列表，不按单个模型进行 N+1 写入；引用失效时界面仍可手填/重新拉取。
        tx.execute("DELETE FROM model_catalog WHERE token IN (SELECT token FROM (SELECT token, ROW_NUMBER() OVER (ORDER BY updated_at DESC,token) AS n, SUM(length(CAST(payload AS BLOB))) OVER (ORDER BY updated_at DESC,token) AS bytes FROM model_catalog) WHERE n>100 OR bytes>16777216)", []).map_err(|_| "模型列表缓存清理失败。")?;
        tx.commit()
            .map_err(|_| "模型列表缓存提交失败。".to_string())
    }

    /// 只读取同一规范化端点的列表，不访问网络或系统凭据；过期/缺失句柄返回空列表。
    pub fn model_catalog(
        &self,
        token: &str,
        endpoint: &str,
    ) -> Result<Vec<crate::discovery::FetchedModel>, String> {
        crate::conditions::validate_id(token)?;
        let payload: Option<String> = self
            .connection
            .query_row(
                "SELECT payload FROM model_catalog WHERE token=?1 AND endpoint=?2",
                params![token, endpoint],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| "模型列表缓存读取失败。")?;
        payload
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|_| "模型列表缓存格式无效，请重新拉取。".into())
            })
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    /// 只读预览当前真实记录的清理影响；确认后仍在同一事务中重新计算，避免预览陈旧。
    pub fn preview_history_retention(
        &self,
        preferences: &Preferences,
    ) -> Result<HistoryCleanupPreview, String> {
        cleanup_plan(&self.connection, preferences).map(|plan| plan.preview)
    }

    /// 配置迁移/偏好更新和真实删除同事务提交。收紧策略且有数据受影响时必须确认，
    /// 任何序列化、损坏记录或 SQLite 错误都保留旧配置与历史，不产生部分导入。
    pub fn save_config_with_retention(
        &mut self,
        config: AppConfig,
        confirm_cleanup: bool,
    ) -> Result<HistoryCleanupPreview, String> {
        let payload = serde_json::to_string(&config).map_err(|_| "配置序列化失败。")?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| "无法开始配置保存事务。")?;
        let plan = cleanup_plan(&transaction, &config.preferences)?;
        if retention_tightened(&self.config.preferences, &config.preferences)
            && plan.preview.removed_count > 0
            && !confirm_cleanup
        {
            return Err(format!(
                "新的保留范围将清理 {} 条历史记录，请预览并确认后保存。",
                plan.preview.removed_count
            ));
        }
        transaction.execute("INSERT INTO app_config(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![payload])
            .map_err(|_| "配置保存失败，操作已回滚。")?;
        apply_cleanup(&transaction, &plan)?;
        if !plan.sequences.is_empty() {
            rebuild_latest_record_index(&transaction)?;
        }
        transaction
            .commit()
            .map_err(|_| "配置与历史清理提交失败。")?;
        self.config = config;
        Ok(plan.preview)
    }

    /// 返回实际保留的全部记录，按自增序号倒序；不能用 LIMIT 隐藏尚未删除的数据。
    pub fn history(&self) -> Result<Vec<TestRecord>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM test_record ORDER BY sequence DESC")
            .map_err(|_| "无法读取测试历史。")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| "无法查询测试历史。")?;
        rows.map(|row| {
            let payload = row.map_err(|_| "历史记录读取失败。")?;
            serde_json::from_str(&payload).map_err(|_| "历史记录格式无效；请先备份数据库。".into())
        })
        .collect()
    }

    /// 返回指定模型最新的一条实际保留记录。索引由启动、写入和清理维护，避免悬浮窗
    /// 每次状态刷新传输所有历史原文；找不到记录与选中模型尚未测试等价。
    pub fn latest_record_for_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<TestRecord>, String> {
        let payload = match self.connection.query_row(
            "SELECT record.payload FROM latest_test_record AS latest
             JOIN test_record AS record ON record.sequence = latest.sequence
             WHERE latest.profile_id = ?1",
            [profile_id],
            |row| row.get::<_, String>(0),
        ) {
            Ok(payload) => payload,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(_) => return Err("无法读取当前模型的最近测试记录。".into()),
        };
        serde_json::from_str(&payload)
            .map(Some)
            .map_err(|_| "最近测试记录格式无效；请先备份数据库。".into())
    }

    /// 插入与 count/day/128MiB 清理同事务，按完整原文保存，不重新判定旧回答。
    pub fn save_record(&mut self, record: &TestRecord) -> Result<(), String> {
        let payload = serde_json::to_string(record).map_err(|_| "结果序列化失败。")?;
        if payload.len() as u64 > HISTORY_STORAGE_LIMIT_BYTES {
            return Err("单条历史超过 128 MiB，未写入数据库。".into());
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| "无法开始结果保存事务。")?;
        transaction
            .execute(
                "INSERT INTO test_record(payload) VALUES(?1)",
                params![payload],
            )
            .map_err(|_| "测试已结束，但历史保存失败。")?;
        let sequence = transaction.last_insert_rowid();
        let plan = cleanup_plan(&transaction, &self.config.preferences)?;
        apply_cleanup(&transaction, &plan)?;
        if plan.sequences.is_empty() {
            upsert_latest_record_index(&transaction, &record.profile_id, sequence)?;
        } else {
            // 条数、日期或容量清理可能删除某个模型的当前最新记录，统一重建避免悬空索引。
            rebuild_latest_record_index(&transaction)?;
        }
        transaction
            .commit()
            .map_err(|_| "历史记录提交失败。".into())
    }

    /// 仅响应用户明确确认的清空命令，不删除配置和凭据。
    pub fn clear_history(&mut self) -> Result<(), String> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| "无法开始清空测试历史。")?;
        transaction
            .execute("DELETE FROM test_record", [])
            .map_err(|_| "清空测试历史失败。")?;
        transaction
            .execute("DELETE FROM latest_test_record", [])
            .map_err(|_| "无法清理最近测试索引。")?;
        transaction.commit().map_err(|_| "清空测试历史提交失败。")?;
        Ok(())
    }

    /// 邮件去重状态独立于历史保留范围；清理/导出回答不会丢失此前降智或故障状态。
    /// 只保存按条件哈希分组的有限状态，不保存题目、回答、邮箱授权码或 API Key。
    pub fn read_notification_state(&self) -> Result<Option<String>, String> {
        match self.connection.query_row(
            "SELECT payload FROM notification_state WHERE id=1",
            [],
            |row| row.get::<_, String>(0),
        ) {
            Ok(payload) => Ok(Some(payload)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(_) => Err("读取邮件通知状态失败。".into()),
        }
    }

    /// 调用方在尝试发信前保存状态，不因发送失败积压补发；数据库失败必须明确报告。
    pub fn save_notification_state(&self, payload: &str) -> Result<(), String> {
        if payload.len() > 2 * 1024 * 1024
            || serde_json::from_str::<serde_json::Value>(payload).is_err()
        {
            return Err("邮件通知状态格式无效或超过大小限制。".into());
        }
        self.connection.execute("INSERT INTO notification_state(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", [payload])
            .map_err(|_| "保存邮件通知状态失败。")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(id: &str, time: &str) -> TestRecord {
        serde_json::from_value(json!({
            "id":id,"profileId":"profile","profileName":"测试","model":"fixture",
            "startedAt":time,"durationMs":10,"status":"passed","answer":" 原文\n",
            "error":null,"prompt":"问题","keyword":"原文","ruleVersion":"literal-contains-v1"
        }))
        .unwrap()
    }

    /// 历史保存请求快照；之后改名或改答案不影响旧记录，旧字段缺失也不能猜测补填。
    #[test]
    fn history_preserves_multiple_answers_and_original_rule_name() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        let mut snapshot = record("multi", "2026-10-25T12:00:00Z");
        snapshot.rule_name = Some("糖果测试".into());
        snapshot.keyword = "21颗".into();
        snapshot.alternative_keywords = vec![" 二十一颗\n".into()];
        snapshot.rule_version = crate::model::ANY_RULE_VERSION.into();
        store.save_record(&snapshot).unwrap();
        let mut config = store.config.clone();
        config.default_rule_name = "改名".into();
        config.rule.keyword = "其他答案".into();
        store.save_config(config).unwrap();
        let saved = &store.history().unwrap()[0];
        assert_eq!(saved.rule_name.as_deref(), Some("糖果测试"));
        assert_eq!(saved.keyword, "21颗");
        assert_eq!(saved.alternative_keywords, [" 二十一颗\n"]);
        assert_eq!(
            store
                .latest_record_for_profile("profile")
                .unwrap()
                .unwrap()
                .rule_version,
            crate::model::ANY_RULE_VERSION
        );
        let old = record("old", "2026-10-25T12:00:00Z");
        assert!(old.rule_name.is_none());
        assert!(old.alternative_keywords.is_empty());
    }

    /// 缓存淘汰与更新同事务：容量受限，清理失败不能替换旧列表或留下半次更新。
    #[test]
    fn catalog_limits_and_cleanup_failure_are_atomic() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        let models: Vec<crate::discovery::FetchedModel> =
            serde_json::from_value(json!([{"id":"old","ownedBy":null}])).unwrap();
        for index in 0..101 {
            store
                .save_model_catalog(
                    &format!("catalog-{index}"),
                    "https://example.com/models",
                    &models,
                )
                .unwrap();
        }
        let count: u64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM model_catalog", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 100);
        assert!(store
            .model_catalog("catalog-0", "https://example.com/models")
            .unwrap()
            .is_empty());
        store.connection.execute_batch("CREATE TRIGGER fail_catalog_cleanup BEFORE DELETE ON model_catalog BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        assert!(store
            .save_model_catalog("extra-catalog", "https://example.com/models", &models)
            .is_err());
        assert!(store
            .model_catalog("extra-catalog", "https://example.com/models")
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .model_catalog("catalog-100", "https://example.com/models")
                .unwrap()[0]
                .id,
            "old"
        );
        store
            .connection
            .execute_batch("DROP TRIGGER fail_catalog_cleanup; DELETE FROM model_catalog;")
            .unwrap();
        let large: Vec<crate::discovery::FetchedModel> = serde_json::from_value(json!((0..4000)
            .map(|index| json!({"id":format!("{index}-{}", "m".repeat(190)),"ownedBy":null}))
            .collect::<Vec<_>>()))
        .unwrap();
        for index in 0..20 {
            store
                .save_model_catalog(
                    &format!("large-{index}"),
                    "https://example.com/models",
                    &large,
                )
                .unwrap();
        }
        let (count, bytes): (u64, u64) = store
            .connection
            .query_row(
                "SELECT COUNT(*), SUM(length(CAST(payload AS BLOB))) FROM model_catalog",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(count < 20);
        assert!(bytes <= 16 * 1024 * 1024);
        assert_eq!(
            store
                .model_catalog("large-19", "https://example.com/models")
                .unwrap()
                .len(),
            4000
        );
    }

    #[test]
    fn notification_state_survives_history_clear_and_rejects_invalid_payload() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        assert!(store.read_notification_state().unwrap().is_none());
        let state = r#"{"states":{"profile:sha256:fixture":{"degraded":true}}}"#;
        store.save_notification_state(state).unwrap();
        store.clear_history().unwrap();
        assert_eq!(
            store.read_notification_state().unwrap().as_deref(),
            Some(state)
        );
        assert!(store.save_notification_state("invalid-json").is_err());
        assert_eq!(
            store.read_notification_state().unwrap().as_deref(),
            Some(state)
        );
    }

    #[test]
    fn retention_preview_is_read_only_and_confirmed_save_deletes_real_rows() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        for id in ["one", "two", "three"] {
            store
                .save_record(&record(id, "2026-10-25T12:00:00Z"))
                .unwrap();
        }
        let mut config = store.config.clone();
        config.preferences.history_max_records = Some(1);
        let preview = store
            .preview_history_retention(&config.preferences)
            .unwrap();
        assert_eq!(preview.removed_count, 2);
        assert_eq!(store.history().unwrap().len(), 3);
        assert!(store
            .save_config_with_retention(config.clone(), false)
            .is_err());
        assert_eq!(store.config.preferences.history_max_records, Some(500));
        assert_eq!(store.history().unwrap().len(), 3);
        store.save_config_with_retention(config, true).unwrap();
        assert_eq!(store.history().unwrap()[0].id, "three");
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM test_record", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn retention_compares_utc_instants_instead_of_offset_strings() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        store
            .save_record(&record("recent", "2026-10-24T01:00:00-12:00"))
            .unwrap();
        store
            .save_record(&record("old", "2026-10-25T01:00:00+14:00"))
            .unwrap();
        let preferences = Preferences {
            history_retention_days: Some(1),
            ..Preferences::default()
        };
        let now = DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let plan = cleanup_plan_at(
            &store.connection,
            &preferences,
            now,
            HISTORY_STORAGE_LIMIT_BYTES,
        )
        .unwrap();
        assert_eq!(plan.preview.removed_count, 1);
        apply_cleanup(&store.connection, &plan).unwrap();
        assert_eq!(store.history().unwrap()[0].id, "recent");
    }

    #[test]
    fn storage_budget_keeps_latest_complete_raw_answer() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        store
            .save_record(&record("old", "2026-10-25T12:00:00Z"))
            .unwrap();
        store
            .save_record(&record("new", "2026-10-25T12:00:00Z"))
            .unwrap();
        let newest_size: u64 = store.connection.query_row("SELECT length(CAST(payload AS BLOB)) FROM test_record ORDER BY sequence DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        let plan = cleanup_plan_at(
            &store.connection,
            &Preferences::default(),
            Utc::now(),
            newest_size,
        )
        .unwrap();
        assert_eq!(plan.preview.remaining_count, 1);
        assert_eq!(plan.preview.removed_count, 1);
        apply_cleanup(&store.connection, &plan).unwrap();
        let records = store.history().unwrap();
        assert_eq!(records[0].id, "new");
        assert_eq!(records[0].answer, " 原文\n");
    }

    #[test]
    fn latest_record_index_tracks_writes_cleanup_and_clear_history() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        let mut first = record("first", "2026-10-25T12:00:00Z");
        first.profile_id = "one".into();
        let mut second = record("second", "2026-10-25T12:01:00Z");
        second.profile_id = "one".into();
        let mut other = record("other", "2026-10-25T12:02:00Z");
        other.profile_id = "two".into();
        store.save_record(&first).unwrap();
        store.save_record(&second).unwrap();
        store.save_record(&other).unwrap();
        assert_eq!(
            store.latest_record_for_profile("one").unwrap().unwrap().id,
            "second"
        );
        assert_eq!(
            store.latest_record_for_profile("two").unwrap().unwrap().id,
            "other"
        );

        let mut config = store.config.clone();
        config.preferences.history_max_records = Some(1);
        store.save_config_with_retention(config, true).unwrap();
        assert!(store.latest_record_for_profile("one").unwrap().is_none());
        assert_eq!(
            store.latest_record_for_profile("two").unwrap().unwrap().id,
            "other"
        );
        store.clear_history().unwrap();
        assert!(store.latest_record_for_profile("two").unwrap().is_none());
    }

    /// 旧版只写 test_record，不维护附加索引；再次启动必须从原始历史恢复最近结果。
    #[test]
    fn latest_record_index_rebuilds_after_an_older_version_writes_history() {
        let path =
            std::env::temp_dir().join(format!("model-probe-index-{}.db", uuid::Uuid::new_v4()));
        {
            let mut store = Store::open(&path).unwrap();
            store
                .save_record(&record("old", "2026-10-25T12:00:00Z"))
                .unwrap();
        }
        {
            let connection = Connection::open(&path).unwrap();
            let payload = serde_json::to_string(&record("new", "2026-10-25T12:01:00Z")).unwrap();
            connection
                .execute("INSERT INTO test_record(payload) VALUES(?1)", [payload])
                .unwrap();
        }
        {
            let store = Store::open(&path).unwrap();
            assert_eq!(
                store
                    .latest_record_for_profile("profile")
                    .unwrap()
                    .unwrap()
                    .id,
                "new"
            );
            assert_eq!(store.history().unwrap().len(), 2);
        }
        std::fs::remove_file(&path).unwrap();
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    /// 索引写入/删除失败不能留下已清理历史或半条新结果，窗口仍须读到原结果。
    #[test]
    fn latest_record_index_failure_rolls_back_record_cleanup_and_clear() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        let mut config = store.config.clone();
        config.preferences.history_max_records = Some(1);
        store.save_config(config).unwrap();
        store
            .save_record(&record("old", "2026-10-25T12:00:00Z"))
            .unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER prevent_index_insert BEFORE INSERT ON latest_test_record
            BEGIN SELECT RAISE(ABORT, 'fixture'); END;",
            )
            .unwrap();
        assert!(store
            .save_record(&record("new", "2026-10-25T12:01:00Z"))
            .is_err());
        assert_eq!(store.history().unwrap().len(), 1);
        assert_eq!(
            store
                .latest_record_for_profile("profile")
                .unwrap()
                .unwrap()
                .id,
            "old"
        );

        store
            .connection
            .execute_batch(
                "CREATE TRIGGER prevent_index_delete BEFORE DELETE ON latest_test_record
            BEGIN SELECT RAISE(ABORT, 'fixture'); END;",
            )
            .unwrap();
        assert!(store.clear_history().is_err());
        assert_eq!(store.history().unwrap()[0].id, "old");
        assert_eq!(
            store
                .latest_record_for_profile("profile")
                .unwrap()
                .unwrap()
                .id,
            "old"
        );
    }

    #[test]
    fn cleanup_failure_rolls_back_config_and_history_and_corruption_is_not_swallowed() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        store
            .save_record(&record("one", "2026-10-25T12:00:00Z"))
            .unwrap();
        store
            .save_record(&record("two", "2026-10-25T12:00:00Z"))
            .unwrap();
        store.save_config(store.config.clone()).unwrap();
        store.connection.execute_batch("CREATE TRIGGER prevent_cleanup BEFORE DELETE ON test_record BEGIN SELECT RAISE(ABORT, 'fixture'); END;").unwrap();
        let mut config = store.config.clone();
        config.preferences.history_max_records = Some(1);
        assert!(store.save_config_with_retention(config, true).is_err());
        assert_eq!(store.history().unwrap().len(), 2);
        let payload: String = store
            .connection
            .query_row("SELECT payload FROM app_config WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<AppConfig>(&payload)
                .unwrap()
                .preferences
                .history_max_records,
            Some(500)
        );
        store
            .connection
            .execute("INSERT INTO test_record(payload) VALUES('broken-json')", [])
            .unwrap();
        assert!(store
            .preview_history_retention(&Preferences::default())
            .is_err());
        assert!(store
            .save_record(&record("three", "2026-10-25T12:00:00Z"))
            .is_err());
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM test_record", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
}

//! 单连接 SQLite 存储。调用者持有进程内互斥锁；配置更新与历史裁剪使用事务。
//! 数据库不存密钥，损坏时报告错误，绝不静默重建覆盖原数据。作者：ahui。

use crate::model::{AppConfig, TestRecord};
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Store {
    connection: Connection,
    pub config: AppConfig,
}

impl Store {
    /// 创建首版 schema；未知未来版本拒绝写入，保护降级运行时的兼容性。
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection =
            Connection::open(path).map_err(|_| "无法打开本地数据库。请检查目录权限。")?;
        connection
            .busy_timeout(std::time::Duration::from_secs(3))
            .map_err(|_| "无法设置数据库等待超时。")?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| "无法读取数据库版本。")?;
        if version > 1 {
            return Err("数据库来自更新版本，请使用对应版本应用。".into());
        }
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS app_config (id INTEGER PRIMARY KEY CHECK(id = 1), payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS test_record (sequence INTEGER PRIMARY KEY AUTOINCREMENT, payload TEXT NOT NULL);
             PRAGMA user_version=1;"
        ).map_err(|_| "初始化数据库失败，已有数据未被删除。")?;
        let config =
            match connection.query_row("SELECT payload FROM app_config WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            }) {
                Ok(payload) => serde_json::from_str(&payload)
                    .map_err(|_| "本地配置格式无效，请先备份数据库。")?,
                Err(rusqlite::Error::QueryReturnedNoRows) => AppConfig::default(),
                Err(_) => return Err("读取本地配置失败。".into()),
            };
        Ok(Self { connection, config })
    }

    /// 单行 UPSERT 成功后才替换内存配置，失败时界面可保留草稿重试。
    pub fn save_config(&mut self, config: AppConfig) -> Result<(), String> {
        let payload = serde_json::to_string(&config).map_err(|_| "配置序列化失败。")?;
        self.connection.execute("INSERT INTO app_config(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![payload]).map_err(|_| "配置保存失败，请检查磁盘空间和权限。")?;
        self.config = config;
        Ok(())
    }

    /// 按自增序号倒序展示，不依赖系统时间有序；最多保留最近 500 条。
    pub fn history(&self) -> Result<Vec<TestRecord>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT payload FROM test_record ORDER BY sequence DESC LIMIT 500")
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

    /// 插入结果和清理最旧记录处于同一事务，异常时全部回滚。
    pub fn save_record(&mut self, record: &TestRecord) -> Result<(), String> {
        let payload = serde_json::to_string(record).map_err(|_| "结果序列化失败。")?;
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
        transaction.execute("DELETE FROM test_record WHERE sequence NOT IN (SELECT sequence FROM test_record ORDER BY sequence DESC LIMIT 500)", []).map_err(|_| "历史记录整理失败。")?;
        transaction
            .commit()
            .map_err(|_| "历史记录提交失败。".into())
    }

    /// 仅响应用户明确确认的清空命令，不删除配置和凭据。
    pub fn clear_history(&self) -> Result<(), String> {
        self.connection
            .execute("DELETE FROM test_record", [])
            .map_err(|_| "清空测试历史失败。")?;
        Ok(())
    }
}

//! 当前 Windows 用户的登录自启：不在数据库保存第二份开关状态，系统注册表是事实来源。
//! 只管理固定名称的本程序启动项；可执行路径来自当前进程，不接受前端传入命令。
//! 启动时不自动开启自启；显式切换失败时恢复原值，并由前端再次查询实际状态。作者：ahui。

/// 查询与修改串行化，避免跨窗口读取到本应用正在提交的中间状态。
static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 返回当前路径的启动项是否启用；读取失败必须显示错误，不能伪装成关闭。
#[tauri::command]
pub fn get_autostart() -> Result<bool, String> {
    let _guard = GATE.lock().map_err(|_| "开机自启状态暂时不可用。")?;
    platform::get().map_err(|_| "无法读取开机自启状态，请检查 Windows 权限后重试。".into())
}

/// 立即启用/关闭当前用户登录自启，不触碰测试门禁、自动周期、配置、历史或凭据。
/// enabled 是唯一输入；操作完成后重新读取系统状态，失败由界面保留原状态并提示。
#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<bool, String> {
    let _guard = GATE.lock().map_err(|_| "开机自启设置暂时不可用。")?;
    platform::set(enabled)
        .map_err(|_| "修改开机自启失败，请检查 Windows 权限或安全软件设置后重试。".to_string())?;
    platform::get().map_err(|_| "开机自启已修改，但状态读取失败，请重新进入本页确认。".into())
}

#[cfg(windows)]
mod platform {
    use std::{io, path::Path};
    use windows_registry::{Key, Type, CURRENT_USER};

    const ENTRY: &str = "测智雷达";
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    /// Windows 启动命令必须完整引用路径，防止空格路径被拆成其他可执行文件。
    /// Run 值限制为 260 个 UTF-16 单元；过长明确失败，不生成不可启动的命令。
    fn command(path: &Path) -> io::Result<String> {
        let path = path.to_str().ok_or(io::ErrorKind::InvalidData)?;
        if path.contains(['"', '\0', '\r', '\n']) || !Path::new(path).is_absolute() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let quoted = format!("\"{path}\"");
        if quoted.encode_utf16().count() > 260 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(quoted)
    }

    /// 不存在表示未配置；权限、类型等错误保留，不能静默显示未启用。
    fn optional<T>(result: windows_registry::Result<T>) -> io::Result<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            // windows-result 转换为 io::Error 时保留 HRESULT，不能依赖 ErrorKind::NotFound。
            Err(error) if matches!(error.code().0 as u32, 0x80070002 | 0x80070003) => Ok(None),
            Err(error) => Err(io::Error::from(error)),
        }
    }

    /// 显式路径与当前 EXE 一致才算启用；兼容 Windows 任务管理器的禁用标记。
    /// 未知 StartupApproved 数据返回错误，避免将策略限制误报为已启用。
    fn read(root: &Key, run: &str, approved: &str, expected: &str) -> io::Result<bool> {
        let Some(key) = optional(root.open(run))? else {
            return Ok(false);
        };
        if optional(key.get_string(ENTRY))?.as_deref() != Some(expected) {
            return Ok(false);
        }
        let Some(key) = optional(root.open(approved))? else {
            return Ok(true);
        };
        let Some(value) = optional(key.get_value(ENTRY))? else {
            return Ok(true);
        };
        if value.ty() != Type::Bytes || value.len() != 12 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        match u32::from_le_bytes(value[..4].try_into().unwrap()) {
            2 | 6 => Ok(true),
            3 | 7 => Ok(false),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }

    /// 先检查两个位置的权限并保存原值，再修改本程序项；失败尝试恢复所有原值。
    /// 不依赖部分 Windows 环境关闭的注册表事务服务；关闭不会创建不存在的键。
    fn write(
        root: &Key,
        run: &str,
        approved: &str,
        command: &str,
        enabled: bool,
    ) -> io::Result<()> {
        let mut entries = Vec::new();
        for (path, approval) in [(run, false), (approved, true)] {
            let mut options = root.options();
            options.read().write();
            if enabled {
                options.create();
            }
            if let Some(key) = optional(options.open(path))? {
                let previous = optional(key.get_value(ENTRY))?;
                entries.push((key, previous, approval));
            }
        }
        let update = (|| {
            for (key, _, approval) in &entries {
                if !enabled {
                    optional(key.remove_value(ENTRY))?;
                } else if *approval {
                    key.set_bytes(ENTRY, Type::Bytes, &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
                        .map_err(io::Error::from)?;
                } else {
                    key.set_string(ENTRY, command).map_err(io::Error::from)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = update {
            let mut restored = true;
            for (key, previous, _) in &entries {
                let result = match previous {
                    Some(value) => key
                        .set_bytes(ENTRY, value.ty(), value)
                        .map_err(io::Error::from),
                    None => optional(key.remove_value(ENTRY)).map(|_| ()),
                };
                restored &= result.is_ok();
            }
            return Err(if restored {
                error
            } else {
                io::Error::other("自启写入失败且无法完整恢复原值")
            });
        }
        Ok(())
    }

    pub(super) fn get() -> io::Result<bool> {
        read(
            CURRENT_USER,
            RUN,
            APPROVED,
            &command(&std::env::current_exe()?)?,
        )
    }

    pub(super) fn set(enabled: bool) -> io::Result<()> {
        // 关闭时不依赖路径长度/可用性，保证移动程序后仍能清理旧启动项。
        let target = if enabled {
            command(&std::env::current_exe()?)?
        } else {
            String::new()
        };
        write(CURRENT_USER, RUN, APPROVED, &target, enabled)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 只使用随机私有测试键，不操作 Windows 真正的 Run 或真实用户自启项。
        #[test]
        fn registry_roundtrip_respects_disabled_and_moved_exe() {
            let path = format!(
                r"Software\com.ahui.model-probe\Tests\{}",
                uuid::Uuid::new_v4()
            );
            let root = CURRENT_USER.create(&path).unwrap();
            let result = std::panic::catch_unwind(|| {
                let target = command(Path::new(r"C:\应用 文件\测智雷达.exe")).unwrap();
                assert_eq!(target, "\"C:\\应用 文件\\测智雷达.exe\"");
                assert!(!read(&root, "Run", "Approved", &target).unwrap());
                write(&root, "Run", "Approved", &target, true).unwrap();
                root.create("Run")
                    .unwrap()
                    .set_string("其他程序", "keep")
                    .unwrap();
                assert!(read(&root, "Run", "Approved", &target).unwrap());
                assert!(!read(&root, "Run", "Approved", "\"C:\\moved.exe\"").unwrap());
                root.create("Approved")
                    .unwrap()
                    .set_bytes(ENTRY, Type::Bytes, &[3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0])
                    .unwrap();
                assert!(!read(&root, "Run", "Approved", &target).unwrap());
                write(&root, "Run", "Approved", &target, true).unwrap();
                assert!(read(&root, "Run", "Approved", &target).unwrap());
                write(&root, "Run", "Approved", "", false).unwrap();
                write(&root, "Run", "Approved", "", false).unwrap();
                assert!(!read(&root, "Run", "Approved", &target).unwrap());
                assert_eq!(
                    root.open("Run").unwrap().get_string("其他程序").unwrap(),
                    "keep"
                );
                assert!(command(Path::new("relative.exe")).is_err());
                assert!(command(Path::new(&format!("C:\\{}.exe", "a".repeat(260)))).is_err());
            });
            drop(root);
            CURRENT_USER.remove_tree(&path).unwrap();
            result.unwrap();
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub(super) fn get() -> std::io::Result<bool> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    pub(super) fn set(_: bool) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

//! Windows 发布版隐藏控制台；构建错误与诊断保留在开发终端。作者：ahui。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    model_probe_lib::run();
}

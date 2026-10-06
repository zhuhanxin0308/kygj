#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 启动失败仅输出稳定诊断，避免把本地路径或环境详情写入用户界面。
    if gravity_workbench_desktop::run().is_err() {
        eprintln!("科研工作台未能启动，请检查桌面运行环境！");
        std::process::exit(1);
    }
}

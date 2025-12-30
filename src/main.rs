use std::env;

use tui_logger::{LevelFilter, TuiLoggerFile, TuiLoggerLevelOutput};

fn main() -> anyhow::Result<()> {
    // initialize logging
    tui_logger::init_logger(LevelFilter::Trace)?;
    tui_logger::set_default_level(LevelFilter::Trace);
    let mut dir = env::temp_dir();
    dir.push("request_tui-debug.log");
    let file_options = TuiLoggerFile::new(dir.to_str().unwrap())
        .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
        .output_file(false)
        .output_separator(':');
    tui_logger::set_log_file(file_options);
    log::debug!(target:"App", "Logging to {}", dir.to_str().unwrap());
    log::debug!(target:"App", "Logging initialized");

    // 使用Crossterm后端初始化终端
    let mut terminal = ratatui::init();
    request_tui::run_app(&mut terminal)?;
    ratatui::restore();
    Ok(())
}

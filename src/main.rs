#![cfg_attr(
    all(target_os = "windows", not(debug_assertions),),
    windows_subsystem = "windows"
)]

use crate::{config::Config, glazewm::start_listen_for_workspaces, window::Window};

mod config;
mod glazewm;
mod window;

fn begin_execution() -> anyhow::Result<()> {
    let config = Config::load();
    let mut window = Window::new(config)?;
    window.prepare()?;

    let hwnd = unsafe { window.hwnd.raw_copy() };
    start_listen_for_workspaces(hwnd)?;

    window.run_loop()
}

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .format_file(true)
        .format_line_number(true)
        .init();

    begin_execution().unwrap_or_else(|err| {
        println!("{:?}", err.backtrace());
        log::error!("Application error: {}", err);
    });

    log::info!("Application exiting normally");

    Ok(())
}

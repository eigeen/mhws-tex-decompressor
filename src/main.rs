mod app;
mod chunk;
mod component;
mod metadata;
mod updater;
mod util;

use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> color_eyre::Result<()> {
    std::panic::set_hook(Box::new(panic_hook));

    let mut app = app::App::default();
    if let Err(e) = app.run().await {
        eprintln!("{}: {:#}", "Error".red().bold(), e);
        wait_for_exit();
        std::process::exit(1);
    }
    wait_for_exit();

    Ok(())
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    eprintln!("{}: {}", "Panic".red().bold(), info);
    wait_for_exit();
    std::process::exit(1);
}

fn wait_for_exit() {
    let _: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Press Enter to exit")
        .allow_empty(true)
        .interact_text()
        .unwrap();
}

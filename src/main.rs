mod app;
mod chunk;
mod metadata;
mod util;

use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};

fn main() {
    std::panic::set_hook(Box::new(panic_hook));

    let mut app = app::App::default();
    if let Err(e) = app.run() {
        eprintln!("{}: {:#}", "Error".red().bold(), e);
        wait_for_exit();
        std::process::exit(1);
    }
    wait_for_exit();
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

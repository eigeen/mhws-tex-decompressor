use std::time::Duration;

use iocraft::prelude::*;

use crate::{
    component::Spinner,
    updater::{Release, Updater},
};

#[derive(Clone, Copy)]
enum State {
    Checking,
    WaitForDownload,
    Downloading,
    Exit,
}

#[component]
pub fn UpdateCheck(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();

    let mut should_exit = hooks.use_state(|| false);
    let mut state = hooks.use_state(|| State::Checking);
    let mut exit_element = hooks.use_state::<Option<AnyElement<'static>>, _>(|| None);
    let mut release_info = hooks.use_state(Release::default);

    let updater = Updater::get();

    let mut set_exit = move || {
        state.set(State::Exit);
        should_exit.set(true);
    };

    hooks.use_terminal_events(move |event| {
        if let TerminalEvent::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = event
            && let KeyCode::Char('q') = code
        {
            set_exit();
        }
    });

    hooks.use_future(async move {
        let mut runner = async || -> color_eyre::Result<()> {
            let Some(r) = updater.check_update().await? else {
                // no update available, exit
                set_exit();
                exit_element.set(Some(element! {
                    View {
                        Text(content: "No update available", color: Color::Green, weight: Weight::Bold)
                    }
                }.into_any()));
                return Ok(());
            };

            // update available, wait for user comfirmation
            release_info.set(r);
            state.set(State::WaitForDownload);

            Ok(())
        };

        if let Err(e) = runner().await {
            set_exit();
            exit_element.set(Some(
                element! {
                    View {
                        Text(content: "Error: ", color: Color::Red, weight: Weight::Bold)
                        Text(content: e.to_string())
                    }
                }
                .into_any(),
            ));
        };
    });

    if should_exit.get() {
        system.exit();
    }

    match state.get() {
        State::Checking => element! {
            View() {
                Spinner()
                Text(content: " Checking for updates... ", weight: Weight::Bold)
                Text(content: "(Press 'Q' to skip)", color: Color::Grey)
            }
        }
        .into_any(),
        State::WaitForDownload => {
            // TODO: use dialoguer temporary, will replace with custom dialog later
            let selection =
                dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt(format!(
                        "Update {} available. Do you want to download and install?",
                        release_info.read().version
                    ))
                    .items(["Yes", "No"])
                    .default(0)
                    .interact()
                    .unwrap();

            if selection == 0 {
                state.set(State::Downloading);
            } else {
                set_exit();
            }
            element! { View }.into_any()
        }
        State::Downloading => {
            element! {
                DownloadProgress()
            }
        }
        .into_any(),
        State::Exit => {
            if let Some(elem) = exit_element.write().take() {
                elem
            } else {
                element! { View }.into_any()
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DownloadState {
    Downloading,
    PendingApply,
    Error,
    Exit,
}

#[component]
fn DownloadProgress(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut system = hooks.use_context_mut::<SystemContext>();

    let mut should_exit = hooks.use_state(|| false);
    let mut state = hooks.use_state(|| DownloadState::Downloading);
    let mut error_msg = hooks.use_state(|| "".to_string());

    let updater = Updater::get();

    // TODO: use indicatif temporarily.
    let bar = hooks.use_state(|| {
        let bar = indicatif::ProgressBar::new(0);
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{pos}/{len} {wide_bar}")
                .unwrap(),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        bar
    });

    hooks.use_future(async move {
        let result = updater
            .download_update(move |curr, total| {
                bar.read().set_length(total);
                bar.read().set_position(curr);
            })
            .await;

        bar.read().finish();

        if let Err(e) = result {
            should_exit.set(true);
            state.set(DownloadState::Error);
            error_msg.set(e.to_string());
        } else {
            state.set(DownloadState::PendingApply);
        }
    });

    if should_exit.get() {
        system.exit();
    }

    match state.get() {
        DownloadState::Downloading => element! {
            View
        }
        .into_any(),
        DownloadState::PendingApply => {
            // TODO: use dialoguer temporarily, will replace with custom dialog later
            let selection =
                dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Update downloaded. Do you want to exit and apply the update?")
                    .items(["Yes", "No"])
                    .default(0)
                    .interact()
                    .unwrap();

            if selection == 0 {
                updater.perform_update_and_close().unwrap();
                unreachable!()
            } else {
                should_exit.set(true);
                state.set(DownloadState::Exit);
                element! { View }.into_any()
            }
        }
        DownloadState::Error => element! {
            View {
                Text(content: "Error: ", color: Color::Red, weight: Weight::Bold)
                Text(content: error_msg.read().clone())
            }
        }
        .into_any(),
        DownloadState::Exit => element! { View }.into_any(),
    }
}

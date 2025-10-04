use std::time::Duration;

use iocraft::prelude::*;

const SPINNING_FRAMES_MOON: [&str; 8] = ["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"];

#[derive(Props)]
pub struct SpinnerProps {
    pub interval: Duration,
}

impl Default for SpinnerProps {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(150),
        }
    }
}

#[component]
pub fn Spinner(mut hooks: Hooks, props: &SpinnerProps) -> impl Into<AnyElement<'static>> {
    let mut frame = hooks.use_state(|| 0);

    let mut timer = tokio::time::interval(props.interval);
    hooks.use_future(async move {
        loop {
            timer.tick().await;
            let next_frame = frame.get() + 1;
            frame.set(next_frame % SPINNING_FRAMES_MOON.len());
        }
    });

    element! {
        Text(content: SPINNING_FRAMES_MOON[frame.get()])
    }
}

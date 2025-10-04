use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct ProgressBarProps {}

#[component]
pub fn ProgressBar(mut hooks: Hooks, props: &ProgressBarProps) -> impl Into<AnyElement<'static>> {
    element! {
        View
    }
}

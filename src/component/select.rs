use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct SelectProps<'a> {
    pub prompt: Option<&'a str>,
    pub options: Option<&'a [String]>,
    pub selected_out: Option<&'a mut usize>,
}

#[component]
pub fn Select<'a>(mut hooks: Hooks, props: &SelectProps<'a>) -> impl Into<AnyElement<'static>> {
    element! {
        View(
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
        ) {
            #(if let Some(prompt) = props.prompt {
                element! {
                    Text()
                }.into_any()
            } else {
                element! { View }.into_any()
            })
        }
    }
}

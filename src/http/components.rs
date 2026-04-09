use std::fmt::Display;

use maud::html;

/// Returns a html component
pub fn component_card(
    title: impl maud::Render,
    body: impl maud::Render,
    error: bool,
) -> maud::Markup {
    html! {
        div class="flex flex-col text-center m-auto w-full max-w-xl border border-gray-500 rounded-md p-4" {
            div."text-xl"."border-b"."font-bold"."pb-2"."text-red-500"[error] {
                (title)
            }
            p class="pt-4" {
                (body)
            }
        }
    }
}

#[derive(Default)]
pub enum PropColor {
    None,
    Gray,
    #[default]
    Blue,
    Red,
    Yellow,
}
impl PropColor {
    fn as_class(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Gray => "bg-gray-500",
            Self::Blue => "bg-blue-500",
            Self::Red => "bg-red-500",
            Self::Yellow => "bg-yellow-500",
        }
    }
}

#[derive(Default)]
pub struct ButtonProps<'a> {
    pub id: Option<&'a str>,
    pub class: Option<&'a str>,
    pub disabled: bool,
    pub color: PropColor,
    pub hx_get: Option<&'a str>,
    pub hx_post: Option<&'a str>,
    pub hx_target: Option<&'a str>,
}

pub fn component_button(props: ButtonProps<'_>, content: impl maud::Render) -> maud::Markup {
    let classes = format!(
        "rounded-md border px-2 {} {}",
        props.color.as_class(),
        props.class.unwrap_or_default()
    );
    html! {
        button
            id=[props.id]
            class=(classes)
            disabled?[props.disabled]
            hx-get=[props.hx_get]
            hx-post=[props.hx_post]
            hx-target=[props.hx_target]
         {
            (content)
        }
    }
}

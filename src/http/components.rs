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
pub enum ButtonColor {
    None,
    Gray,
    #[default]
    Blue,
    Red,
    Yellow,
}
impl ButtonColor {
    fn as_class(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Gray => "bg-gray-500 hover:bg-gray-700",
            Self::Blue => "bg-blue-500 hover:bg-blue-700",
            Self::Red => "bg-red-500 hover:bg-red-700",
            Self::Yellow => "bg-yellow-500 hover:bg-yellow-700",
        }
    }
}

#[derive(Default)]
pub struct ButtonProps<'a> {
    pub id: Option<&'a str>,
    pub class: Option<&'a str>,
    pub disabled: bool,
    pub color: ButtonColor,
    pub hx_get: Option<&'a str>,
    pub hx_post: Option<&'a str>,
    pub hx_target: Option<&'a str>,
}

pub fn component_button(props: ButtonProps<'_>, content: impl maud::Render) -> maud::Markup {
    html! {
        button
            id=[props.id]
            class={
                "rounded-md border px-2 cursor-pointer transition-colors "
                (props.color.as_class())
                " "
                (props.class.unwrap_or_default())
            }
            disabled?[props.disabled]
            hx-get=[props.hx_get]
            hx-post=[props.hx_post]
            hx-target=[props.hx_target]
         {
            (content)
        }
    }
}

#[derive(Default)]
pub struct InputProps<'a> {
    pub id: Option<&'a str>,
    pub class: Option<&'a str>,
    pub disabled: bool,
    pub value: Option<&'a str>,
    pub placeholder: Option<&'a str>,
}

pub fn component_input(props: InputProps) -> maud::Markup {
    html! {
        input
            id=[props.id]
            class={
                "rounded-md border px-2 "
                (props.class.unwrap_or_default())
            }
            disabled?[props.disabled]
            value=[props.value]
            placeholder=[props.placeholder]
        {

        }
    }
}

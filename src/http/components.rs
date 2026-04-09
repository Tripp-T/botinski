use maud::html;

/// Returns a html component
pub fn component_card<S: AsRef<str>>(title: S, body: S, error: bool) -> maud::Markup {
    html! {
        div class="flex flex-col text-center m-auto w-full max-w-xl border border-gray-500 rounded-md p-4" {
            div."text-xl"."border-b"."font-bold"."pb-2"."text-red-500"[error] {
                (title.as_ref())
            }
            p class="pt-4" {
                (body.as_ref())
            }
        }
    }
}

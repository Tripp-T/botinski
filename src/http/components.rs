use maud::html;

/// Returns a html component
pub fn component_card<S: AsRef<str>>(title: S, description: S) -> maud::Markup {
    html! {
        div class="flex flex-col text-center m-auto w-full max-w-xl border border-gray-500 rounded-md p-4" {
            div class="text-xl border-b font-bold pb-2" {
                (title.as_ref())
            }
            p class="pt-4" {
                (description.as_ref())
            }
        }
    }
}

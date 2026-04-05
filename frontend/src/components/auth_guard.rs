use crate::state::auth::use_auth;
use leptos::children::ChildrenFn;
use leptos::prelude::*;
use leptos_router::components::Redirect;

#[component]
pub fn AuthGuard(children: ChildrenFn) -> impl IntoView {
    let auth = use_auth();
    let logged_in = move || auth.get().is_some();

    view! {
        <Show when=logged_in fallback=|| view! { <Redirect path="/login"/> }>
            {children()}
        </Show>
    }
}

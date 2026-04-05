use crate::state::auth::use_auth;
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NavBar() -> impl IntoView {
    let auth = use_auth();
    let logged_in = move || auth.get().is_some();

    view! {
        <nav class="navbar">
            <div class="navbar-brand">
                <A href="/">"SaaS Enterprise Suite"</A>
            </div>
            <div class="navbar-links">
                <Show when=move || !logged_in()>
                    <A href="/login" attr:class="btn btn-primary">"Login"</A>
                </Show>
                <Show when=logged_in>
                    <A href="/hcm">"HCM"</A>
                    <A href="/erp">"ERP"</A>
                    <A href="/scm">"SCM"</A>
                    <button class="btn btn-secondary" on:click=move |_| {
                        auth.set(None);
                    }>"Logout"</button>
                </Show>
            </div>
        </nav>
    }
}

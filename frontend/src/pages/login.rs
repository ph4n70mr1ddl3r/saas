use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use crate::state::auth::{use_auth, AuthState};

#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(None::<String>);

    // If already logged in, redirect to dashboard
    let nav_clone = navigate.clone();
    Effect::new(move |_| {
        if auth.get().is_some() {
            nav_clone("/hcm", NavigateOptions::default());
        }
    });

    let on_login = move |_| {
        let user = username.get();
        let pass = password.get();
        if user.is_empty() || pass.is_empty() {
            set_error.set(Some("Username and password are required".to_string()));
            return;
        }
        // For now, set a mock auth state. Real auth would call the IAM service.
        auth.set(Some(AuthState {
            access_token: "mock-token".to_string(),
            user_id: "1".to_string(),
            username: user,
            display_name: "User".to_string(),
        }));
        navigate("/hcm", NavigateOptions::default());
    };

    view! {
        <div class="login-form">
            <h2>"Login"</h2>
            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
            <input type="text" placeholder="Username"
                on:input=move |ev| set_username.set(event_target_value(&ev))
                prop:value=move || username.get()
            />
            <input type="password" placeholder="Password"
                on:input=move |ev| set_password.set(event_target_value(&ev))
                prop:value=move || password.get()
            />
            <button class="btn btn-primary" on:click=on_login>
                "Sign In"
            </button>
        </div>
    }
}

use crate::state::auth::{use_auth, AuthState};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

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

        let auth = auth.clone();
        let navigate = navigate.clone();
        spawn_local(async move {
            let client = reqwest::Client::new();
            let result = client
                .post(&format!(
                    "{}/api/v1/auth/login",
                    std::env::var("API_BASE_URL")
                        .unwrap_or_else(|_| "http://localhost:8000".to_string())
                ))
                .json(&serde_json::json!({
                    "username": user,
                    "password": pass,
                }))
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if let Some(data) = body.get("data") {
                            let access_token =
                                data["access_token"].as_str().unwrap_or("").to_string();
                            let user_obj = data.get("user");
                            let user_id = user_obj
                                .and_then(|u| u.get("id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let display_name = user_obj
                                .and_then(|u| u.get("display_name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&user)
                                .to_string();

                            if access_token.is_empty() {
                                set_error.set(Some("Invalid login response".to_string()));
                            } else {
                                auth.set(Some(AuthState {
                                    access_token,
                                    user_id,
                                    username: user.clone(),
                                    display_name,
                                }));
                                navigate("/hcm", NavigateOptions::default());
                            }
                        } else {
                            set_error.set(Some("Invalid login response".to_string()));
                        }
                    }
                }
                Ok(resp) => {
                    set_error.set(Some(format!("Login failed: {}", resp.status())));
                }
                Err(e) => {
                    set_error.set(Some(format!("Connection error: {}", e)));
                }
            }
        });
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

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn EmployeeDetailPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.with(|p| p.get("id").unwrap_or_default());

    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Employee Detail"</h2>
                <p>"Employee ID: " {move || id()}</p>
            </div>
        </AuthGuard>
    }
}

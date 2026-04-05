use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn ProcurementPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Procurement"</h2>
                <p>"Purchase orders and suppliers"</p>
            </div>
        </AuthGuard>
    }
}

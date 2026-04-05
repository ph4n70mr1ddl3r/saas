use crate::components::auth_guard::AuthGuard;
use leptos::prelude::*;

#[component]
pub fn ManufacturingPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Manufacturing"</h2>
                <p>"Work orders, BOM, and production"</p>
            </div>
        </AuthGuard>
    }
}

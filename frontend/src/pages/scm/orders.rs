use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn OrdersPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Order Management"</h2>
                <p>"Sales orders, fulfillment, and returns"</p>
            </div>
        </AuthGuard>
    }
}

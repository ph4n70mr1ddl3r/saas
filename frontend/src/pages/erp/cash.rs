use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn CashPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Cash Management"</h2>
                <p>"Bank accounts and reconciliation"</p>
            </div>
        </AuthGuard>
    }
}

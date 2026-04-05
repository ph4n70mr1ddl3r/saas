use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn PayablesPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Accounts Payable"</h2>
                <p>"Vendor invoices and payments"</p>
            </div>
        </AuthGuard>
    }
}

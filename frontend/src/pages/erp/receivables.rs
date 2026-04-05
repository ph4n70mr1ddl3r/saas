use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn ReceivablesPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Accounts Receivable"</h2>
                <p>"Customer invoices and receipts"</p>
            </div>
        </AuthGuard>
    }
}

use crate::components::auth_guard::AuthGuard;
use leptos::prelude::*;

#[component]
pub fn LedgerPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"General Ledger"</h2>
                <p>"Chart of accounts and journal entries"</p>
            </div>
        </AuthGuard>
    }
}

use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn PayrollPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Payroll"</h2>
                <p>"Payroll management coming soon"</p>
            </div>
        </AuthGuard>
    }
}

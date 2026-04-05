use crate::components::auth_guard::AuthGuard;
use leptos::prelude::*;

#[component]
pub fn BenefitsPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Benefits"</h2>
                <p>"Benefits management coming soon"</p>
            </div>
        </AuthGuard>
    }
}

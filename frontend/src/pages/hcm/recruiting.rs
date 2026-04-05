use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn RecruitingPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Recruiting"</h2>
                <p>"Recruiting management coming soon"</p>
            </div>
        </AuthGuard>
    }
}

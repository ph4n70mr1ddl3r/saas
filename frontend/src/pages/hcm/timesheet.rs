use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn TimesheetPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Timesheets"</h2>
                <p>"Time & labor management coming soon"</p>
            </div>
        </AuthGuard>
    }
}

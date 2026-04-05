use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

#[component]
pub fn DepartmentListPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Departments"</h2>
                <p>"Department management coming soon"</p>
            </div>
        </AuthGuard>
    }
}

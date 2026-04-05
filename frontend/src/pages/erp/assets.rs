use crate::components::auth_guard::AuthGuard;
use leptos::prelude::*;

#[component]
pub fn AssetsPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Fixed Assets"</h2>
                <p>"Asset tracking and depreciation"</p>
            </div>
        </AuthGuard>
    }
}

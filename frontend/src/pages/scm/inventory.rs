use crate::components::auth_guard::AuthGuard;
use leptos::prelude::*;

#[component]
pub fn InventoryPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="page">
                <h2>"Inventory"</h2>
                <p>"Stock levels, warehouses, and movements"</p>
            </div>
        </AuthGuard>
    }
}

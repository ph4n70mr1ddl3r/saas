use crate::components::auth_guard::AuthGuard;
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn DashboardPage() -> impl IntoView {
    view! {
        <AuthGuard>
            <div class="dashboard">
                <h1>"Dashboard"</h1>
                <div class="dashboard-grid">
                    <div class="dashboard-card">
                        <h3>"Human Capital Management"</h3>
                        <p>"Manage employees, payroll, benefits, and recruiting"</p>
                        <A href="/hcm">"Go to HCM"</A>
                    </div>
                    <div class="dashboard-card">
                        <h3>"Enterprise Resource Planning"</h3>
                        <p>"Financial management, ledger, payables, and receivables"</p>
                        <A href="/erp">"Go to ERP"</A>
                    </div>
                    <div class="dashboard-card">
                        <h3>"Supply Chain Management"</h3>
                        <p>"Inventory, procurement, orders, and manufacturing"</p>
                        <A href="/scm">"Go to SCM"</A>
                    </div>
                </div>
            </div>
        </AuthGuard>
    }
}

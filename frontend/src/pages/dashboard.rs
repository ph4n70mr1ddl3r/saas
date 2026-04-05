use leptos::prelude::*;
use crate::components::auth_guard::AuthGuard;

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
                        <a href="/hcm">"Go to HCM"</a>
                    </div>
                    <div class="dashboard-card">
                        <h3>"Enterprise Resource Planning"</h3>
                        <p>"Financial management, ledger, payables, and receivables"</p>
                        <a href="/erp">"Go to ERP"</a>
                    </div>
                    <div class="dashboard-card">
                        <h3>"Supply Chain Management"</h3>
                        <p>"Inventory, procurement, orders, and manufacturing"</p>
                        <a href="/scm">"Go to SCM"</a>
                    </div>
                </div>
            </div>
        </AuthGuard>
    }
}

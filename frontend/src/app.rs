use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::components::*;
use leptos_router::{StaticSegment, ParamSegment};

use crate::components::layout::Layout;
use crate::pages;
use crate::pages::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/saas-frontend.css"/>
        <Title text="SaaS Enterprise Suite"/>
        <Router>
            <Routes fallback=|| view! { <p>"Not found"</p> }>
                <Route path=StaticSegment("/") view=DashboardPage/>
                <Route path=StaticSegment("/login") view=LoginPage/>
                <ParentRoute path=StaticSegment("/hcm") view=|| view! { <Layout module="hcm"> <HcmRouter/> </Layout> }>
                    <Route path=StaticSegment("/") view=pages::hcm::EmployeeListPage/>
                    <Route path=StaticSegment("/employees") view=pages::hcm::EmployeeListPage/>
                    <Route path=(StaticSegment("/employees"), ParamSegment("id")) view=pages::hcm::EmployeeDetailPage/>
                    <Route path=StaticSegment("/departments") view=pages::hcm::DepartmentListPage/>
                    <Route path=StaticSegment("/payroll") view=pages::hcm::PayrollPage/>
                    <Route path=StaticSegment("/benefits") view=pages::hcm::BenefitsPage/>
                    <Route path=StaticSegment("/timesheets") view=pages::hcm::TimesheetPage/>
                    <Route path=StaticSegment("/recruiting") view=pages::hcm::RecruitingPage/>
                </ParentRoute>
                <ParentRoute path=StaticSegment("/erp") view=|| view! { <Layout module="erp"> <ErpRouter/> </Layout> }>
                    <Route path=StaticSegment("/") view=pages::erp::LedgerPage/>
                    <Route path=StaticSegment("/ledger") view=pages::erp::LedgerPage/>
                    <Route path=StaticSegment("/payables") view=pages::erp::PayablesPage/>
                    <Route path=StaticSegment("/receivables") view=pages::erp::ReceivablesPage/>
                    <Route path=StaticSegment("/assets") view=pages::erp::AssetsPage/>
                    <Route path=StaticSegment("/cash") view=pages::erp::CashPage/>
                </ParentRoute>
                <ParentRoute path=StaticSegment("/scm") view=|| view! { <Layout module="scm"> <ScmRouter/> </Layout> }>
                    <Route path=StaticSegment("/") view=pages::scm::InventoryPage/>
                    <Route path=StaticSegment("/inventory") view=pages::scm::InventoryPage/>
                    <Route path=StaticSegment("/procurement") view=pages::scm::ProcurementPage/>
                    <Route path=StaticSegment("/orders") view=pages::scm::OrdersPage/>
                    <Route path=StaticSegment("/manufacturing") view=pages::scm::ManufacturingPage/>
                </ParentRoute>
            </Routes>
        </Router>
    }
}

#[component]
fn HcmRouter() -> impl IntoView {
    view! { <Outlet/> }
}

#[component]
fn ErpRouter() -> impl IntoView {
    view! { <Outlet/> }
}

#[component]
fn ScmRouter() -> impl IntoView {
    view! { <Outlet/> }
}

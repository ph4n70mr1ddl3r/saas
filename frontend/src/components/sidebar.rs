use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn SideBar(module: &'static str) -> impl IntoView {
    let links = move || match module {
        "hcm" => vec![
            ("Employees", "/hcm/employees"),
            ("Departments", "/hcm/departments"),
            ("Payroll", "/hcm/payroll"),
            ("Benefits", "/hcm/benefits"),
            ("Timesheets", "/hcm/timesheets"),
            ("Recruiting", "/hcm/recruiting"),
        ],
        "erp" => vec![
            ("General Ledger", "/erp/ledger"),
            ("Accounts Payable", "/erp/payables"),
            ("Accounts Receivable", "/erp/receivables"),
            ("Fixed Assets", "/erp/assets"),
            ("Cash Management", "/erp/cash"),
        ],
        "scm" => vec![
            ("Inventory", "/scm/inventory"),
            ("Procurement", "/scm/procurement"),
            ("Order Management", "/scm/orders"),
            ("Manufacturing", "/scm/manufacturing"),
        ],
        _ => vec![],
    };

    view! {
        <aside class="sidebar">
            <ul class="sidebar-menu">
                <For each=links key=|l| l.0.clone() children=move |(label, href): (&'static str, &'static str)| {
                    view! {
                        <li>
                            <A href={href}>{label}</A>
                        </li>
                    }
                }/>
            </ul>
        </aside>
    }
}

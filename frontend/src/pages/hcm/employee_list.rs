use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use crate::components::auth_guard::AuthGuard;
use crate::state::auth::use_auth;

#[derive(Clone, Debug, serde::Deserialize)]
struct Employee {
    id: String,
    first_name: String,
    last_name: String,
    email: String,
    job_title: String,
    status: String,
    department_id: String,
}

#[component]
pub fn EmployeeListPage() -> impl IntoView {
    let (employees, set_employees) = signal(Vec::<Employee>::new());
    let (loading, set_loading) = signal(true);
    let auth = use_auth();

    Effect::new(move |_| {
        spawn_local(async move {
            let client = reqwest::Client::new();
            let mut req = client.get(&format!("{}/api/v1/employees",
                std::env::var("API_BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string())));
            if let Some(auth_state) = auth.get() {
                req = req.header("Authorization", format!("Bearer {}", auth_state.access_token));
            }
            let result = req.send().await;
            if let Ok(resp) = result {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(data) = body.get("data") {
                        if let Ok(emps) = serde_json::from_value::<Vec<Employee>>(data.clone()) {
                            set_employees.set(emps);
                        }
                    }
                }
            }
            set_loading.set(false);
        });
    });

    view! {
        <AuthGuard>
            <div class="page">
                <div class="page-header">
                    <h2>"Employees"</h2>
                </div>
                <Show when=move || loading.get() fallback=|| view! {}>
                    <p>"Loading..."</p>
                </Show>
                <Show when=move || !loading.get()>
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>"Name"</th>
                                <th>"Email"</th>
                                <th>"Job Title"</th>
                                <th>"Status"</th>
                                <th>"Actions"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For each=move || employees.get() key=|e| e.id.clone() children=move |emp: Employee| {
                                view! {
                                    <tr>
                                        <td>{format!("{} {}", emp.first_name, emp.last_name)}</td>
                                        <td>{emp.email.clone()}</td>
                                        <td>{emp.job_title.clone()}</td>
                                        <td>{emp.status.clone()}</td>
                                        <td><A href={format!("/hcm/employees/{}", emp.id)}>View</A></td>
                                    </tr>
                                }
                            }/>
                        </tbody>
                    </table>
                </Show>
            </div>
        </AuthGuard>
    }
}

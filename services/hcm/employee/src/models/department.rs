use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDepartment {
    pub name: String,
    pub parent_id: Option<String>,
    pub manager_id: Option<String>,
    pub cost_center: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDepartment {
    pub name: Option<String>,
    pub parent_id: Option<String>,
    pub manager_id: Option<String>,
    pub cost_center: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DepartmentResponse {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub manager_id: Option<String>,
    pub cost_center: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrgChartNode {
    pub id: String,
    pub employee_number: String,
    pub first_name: String,
    pub last_name: String,
    pub job_title: String,
    pub department_id: String,
    pub reports_to: Option<String>,
    pub department_name: Option<String>,
}

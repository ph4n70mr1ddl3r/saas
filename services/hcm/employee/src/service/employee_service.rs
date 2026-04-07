use crate::models::department::*;
use crate::models::employee::*;
use crate::models::employment_history::*;
use crate::repository::department_repo::DepartmentRepo;
use crate::repository::employee_repo::EmployeeRepo;
use crate::repository::employment_history_repo::EmploymentHistoryRepo;
use saas_common::error::AppResult;
use saas_common::pagination::PaginationParams;
use saas_common::response::ApiListResponse;
use saas_nats_bus::NatsBus;
use saas_proto::events::ApplicationStatusChanged;
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct EmployeeService {
    emp_repo: EmployeeRepo,
    dept_repo: DepartmentRepo,
    history_repo: EmploymentHistoryRepo,
    bus: NatsBus,
}

impl EmployeeService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            emp_repo: EmployeeRepo::new(pool.clone()),
            dept_repo: DepartmentRepo::new(pool.clone()),
            history_repo: EmploymentHistoryRepo::new(pool),
            bus,
        }
    }

    pub async fn list_employees(
        &self,
        pag: &PaginationParams,
        filters: &EmployeeFilters,
    ) -> AppResult<ApiListResponse<EmployeeResponse>> {
        let (employees, total) = self.emp_repo.list(pag, filters).await?;
        Ok(ApiListResponse {
            data: employees,
            total,
            page: pag.page(),
            per_page: pag.per_page(),
        })
    }

    pub async fn get_employee(&self, id: &str) -> AppResult<EmployeeResponse> {
        self.emp_repo.get_by_id(id).await
    }

    pub async fn create_employee(&self, input: CreateEmployee) -> AppResult<EmployeeResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        let emp = self.emp_repo.create(&input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.employee.created",
                saas_proto::events::EmployeeCreated {
                    employee_id: emp.id.clone(),
                    first_name: emp.first_name.clone(),
                    last_name: emp.last_name.clone(),
                    email: emp.email.clone(),
                    department_id: emp.department_id.clone(),
                    hire_date: emp.hire_date.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.employee.created",
                e
            );
        }
        Ok(emp)
    }

    pub async fn update_employee(
        &self,
        id: &str,
        input: UpdateEmployee,
    ) -> AppResult<EmployeeResponse> {
        let mut changes = Vec::new();
        if input.first_name.is_some() { changes.push("first_name".into()); }
        if input.last_name.is_some() { changes.push("last_name".into()); }
        if input.email.is_some() { changes.push("email".into()); }
        if input.phone.is_some() { changes.push("phone".into()); }
        if input.department_id.is_some() { changes.push("department_id".into()); }
        if input.reports_to.is_some() { changes.push("reports_to".into()); }
        if input.job_title.is_some() { changes.push("job_title".into()); }
        if input.status.is_some() { changes.push("status".into()); }

        // Record employment history for job-relevant fields
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let existing = self.emp_repo.get_by_id(id).await.ok();
        let track_fields: Vec<(&str, Option<&str>)> = vec![
            ("job_title", input.job_title.as_deref()),
            ("department_id", input.department_id.as_deref()),
            ("reports_to", input.reports_to.as_deref()),
            ("status", input.status.as_deref()),
        ];
        for (field_name, new_val) in &track_fields {
            if let Some(nv) = new_val {
                let old_val = existing.as_ref().map(|e| match *field_name {
                    "job_title" => e.job_title.clone(),
                    "department_id" => e.department_id.clone(),
                    "reports_to" => e.reports_to.clone().unwrap_or_default(),
                    "status" => e.status.clone(),
                    _ => String::new(),
                });
                let _ = self.history_repo.create(&CreateEmploymentHistoryRequest {
                    employee_id: id.to_string(),
                    field_name: field_name.to_string(),
                    old_value: old_val,
                    new_value: Some(nv.to_string()),
                    effective_date: today.clone(),
                }).await;
            }
        }

        let emp = self
            .emp_repo
            .update(
                id,
                input.first_name.as_deref(),
                input.last_name.as_deref(),
                input.email.as_deref(),
                input.phone.as_deref(),
                input.department_id.as_deref(),
                input.reports_to.as_deref(),
                input.job_title.as_deref(),
                input.status.as_deref(),
            )
            .await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.employee.updated",
                saas_proto::events::EmployeeUpdated {
                    employee_id: emp.id.clone(),
                    changes,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.employee.updated",
                e
            );
        }
        Ok(emp)
    }

    pub async fn delete_employee(&self, id: &str) -> AppResult<EmployeeResponse> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.emp_repo.terminate(id, &today).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.employee.terminated",
                saas_proto::events::EmployeeTerminated {
                    employee_id: id.to_string(),
                    termination_date: today,
                    reason: None,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.employee.terminated",
                e
            );
        }
        self.emp_repo.get_by_id(id).await
    }

    pub async fn get_direct_reports(&self, manager_id: &str) -> AppResult<Vec<EmployeeResponse>> {
        self.emp_repo.get_direct_reports(manager_id).await
    }

    pub async fn list_departments(&self) -> AppResult<Vec<DepartmentResponse>> {
        self.dept_repo.list().await
    }

    pub async fn get_department(&self, id: &str) -> AppResult<DepartmentResponse> {
        self.dept_repo.get_by_id(id).await
    }

    pub async fn create_department(
        &self,
        input: CreateDepartment,
    ) -> AppResult<DepartmentResponse> {
        self.dept_repo.create(&input).await
    }

    pub async fn update_department(
        &self,
        id: &str,
        input: UpdateDepartment,
    ) -> AppResult<DepartmentResponse> {
        self.dept_repo
            .update(
                id,
                input.name.as_deref(),
                input.parent_id.as_deref(),
                input.manager_id.as_deref(),
                input.cost_center.as_deref(),
            )
            .await
    }

    pub async fn get_org_chart(&self) -> AppResult<Vec<OrgChartNode>> {
        self.emp_repo.get_org_chart().await
    }

    pub async fn list_employment_history(&self, employee_id: &str) -> AppResult<Vec<EmploymentHistory>> {
        self.history_repo.list_by_employee(employee_id).await
    }

    /// Handle hiring event from recruiting service — auto-create an employee record.
    pub async fn handle_application_hired(&self, event: &ApplicationStatusChanged) -> AppResult<EmployeeResponse> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let employee_number = format!("EMP-AUTO-{}", &event.application_id[..8.min(event.application_id.len())]);

        let input = CreateEmployee {
            first_name: event.candidate_first_name.clone(),
            last_name: event.candidate_last_name.clone(),
            email: event.candidate_email.clone(),
            phone: None,
            hire_date: today,
            department_id: event.department_id.clone(),
            reports_to: None,
            job_title: event.job_title.clone(),
            employee_number: employee_number,
        };

        self.create_employee(input).await
    }
}

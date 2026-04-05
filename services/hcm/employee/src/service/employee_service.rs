use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use saas_common::pagination::PaginationParams;
use saas_common::response::ApiListResponse;
use crate::repository::employee_repo::EmployeeRepo;
use crate::repository::department_repo::DepartmentRepo;
use crate::models::employee::*;
use validator::Validate;
use crate::models::department::*;

#[derive(Clone)]
pub struct EmployeeService {
    emp_repo: EmployeeRepo,
    dept_repo: DepartmentRepo,
    bus: NatsBus,
}

impl EmployeeService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self { emp_repo: EmployeeRepo::new(pool.clone()), dept_repo: DepartmentRepo::new(pool), bus }
    }

    pub async fn list_employees(&self, pag: &PaginationParams, filters: &EmployeeFilters) -> AppResult<ApiListResponse<EmployeeResponse>> {
        let (employees, total) = self.emp_repo.list(pag, filters).await?;
        Ok(ApiListResponse { data: employees, total, page: pag.page(), per_page: pag.per_page() })
    }

    pub async fn get_employee(&self, id: &str) -> AppResult<EmployeeResponse> {
        self.emp_repo.get_by_id(id).await
    }

    pub async fn create_employee(&self, input: CreateEmployee) -> AppResult<EmployeeResponse> {
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        let emp = self.emp_repo.create(&input).await?;
        if let Err(e) = self.bus.publish("hcm.employee.created", saas_proto::events::EmployeeCreated {
            employee_id: emp.id.clone(),
            first_name: emp.first_name.clone(),
            last_name: emp.last_name.clone(),
            email: emp.email.clone(),
            department_id: emp.department_id.clone(),
            hire_date: emp.hire_date.clone(),
        }).await {
            tracing::error!("CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.", "hcm.employee.created", e);
        }
        Ok(emp)
    }

    pub async fn update_employee(&self, id: &str, input: UpdateEmployee) -> AppResult<EmployeeResponse> {
        let emp = self.emp_repo.update(id, input.first_name.as_deref(), input.last_name.as_deref(),
            input.email.as_deref(), input.phone.as_deref(), input.department_id.as_deref(),
            input.reports_to.as_deref(), input.job_title.as_deref(), input.status.as_deref()).await?;
        Ok(emp)
    }

    pub async fn delete_employee(&self, id: &str) -> AppResult<EmployeeResponse> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.emp_repo.terminate(id, &today).await?;
        if let Err(e) = self.bus.publish("hcm.employee.terminated", saas_proto::events::EmployeeTerminated {
            employee_id: id.to_string(), termination_date: today, reason: None,
        }).await {
            tracing::error!("CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.", "hcm.employee.terminated", e);
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

    pub async fn create_department(&self, input: CreateDepartment) -> AppResult<DepartmentResponse> {
        self.dept_repo.create(&input).await
    }

    pub async fn update_department(&self, id: &str, input: UpdateDepartment) -> AppResult<DepartmentResponse> {
        self.dept_repo.update(id, input.name.as_deref(), input.parent_id.as_deref(),
            input.manager_id.as_deref(), input.cost_center.as_deref()).await
    }

    pub async fn get_org_chart(&self) -> AppResult<Vec<OrgChartNode>> {
        self.emp_repo.get_org_chart().await
    }
}

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
use saas_proto::events::UserCreated;
use saas_proto::events::UserUpdated;
use saas_proto::events::UserDeactivated;
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

    /// Handle IAM user created event — auto-create an employee record.
    /// Uses username as the employee name, email from the user account,
    /// default department "unassigned" (created if it doesn't exist),
    /// and today's date as hire date.
    /// Handles duplicate creation gracefully by catching and logging errors.
    pub async fn handle_user_created(&self, user_id: &str, username: &str, email: &str) {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let employee_number = format!("EMP-IAM-{}", &user_id[..8.min(user_id.len())]);

        // Look up or create the "unassigned" department
        let department_id = match self.ensure_unassigned_department().await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(
                    "Failed to ensure unassigned department for IAM user {}: {}",
                    user_id, e
                );
                return;
            }
        };

        let input = CreateEmployee {
            first_name: username.to_string(),
            last_name: "-".to_string(),
            email: email.to_string(),
            phone: None,
            hire_date: today,
            department_id,
            reports_to: None,
            job_title: "Unassigned".to_string(),
            employee_number,
        };

        tracing::info!(
            "Auto-creating employee from IAM user created event: user_id={}, username={}, email={}",
            user_id, username, email
        );

        match self.create_employee(input).await {
            Ok(emp) => {
                tracing::info!(
                    "Successfully auto-created employee {} from IAM user {}",
                    emp.id, user_id
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to auto-create employee from IAM user {} (may already exist): {}",
                    user_id, e
                );
            }
        }
    }

    /// Ensure an "unassigned" department exists, creating it if needed.
    /// Returns the department ID.
    async fn ensure_unassigned_department(&self) -> AppResult<String> {
        let departments = self.dept_repo.list().await?;
        if let Some(dept) = departments.iter().find(|d| d.name == "Unassigned") {
            return Ok(dept.id.clone());
        }
        let dept = self
            .dept_repo
            .create(&CreateDepartment {
                name: "Unassigned".to_string(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await?;
        Ok(dept.id)
    }

    /// Handle IAM user updated event — sync email changes to employee record.
    pub async fn handle_user_updated(&self, user_id: &str, username: &str, email: &str) {
        tracing::info!(
            "IAM user updated event: user_id={}, username={}, email={}",
            user_id, username, email
        );

        let pag = PaginationParams { page: Some(1), per_page: Some(1000) };
        let filters = EmployeeFilters { department_id: None, status: None };
        let result = match self.emp_repo.list(&pag, &filters).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to list employees for IAM user update sync: {}", e);
                return;
            }
        };

        let emp_number = format!("EMP-IAM-{}", &user_id[..8.min(user_id.len())]);
        if let Some(emp) = result.0.iter().find(|e| e.employee_number == emp_number) {
            if emp.email != email {
                tracing::info!(
                    "Syncing email update for employee {} (IAM user {}): {} -> {}",
                    emp.id, user_id, emp.email, email
                );
                let update = UpdateEmployee {
                    first_name: None,
                    last_name: None,
                    email: Some(email.to_string()),
                    phone: None,
                    department_id: None,
                    reports_to: None,
                    job_title: None,
                    status: None,
                };
                if let Err(e) = self.update_employee(&emp.id, update).await {
                    tracing::error!(
                        "Failed to sync email for employee {} from IAM update: {}", emp.id, e
                    );
                }
            }
        } else {
            tracing::warn!(
                "No employee found matching IAM user {} (looked for employee_number={})",
                user_id, emp_number
            );
        }
    }

    /// Handle IAM user deactivated event — log for HR awareness.
    /// Auto-termination is an HR decision, not automatic.
    pub async fn handle_user_deactivated(&self, user_id: &str, username: &str) {
        tracing::info!(
            "IAM user deactivated event: user_id={}, username={} — employee termination may be required",
            user_id, username
        );

        let pag = PaginationParams { page: Some(1), per_page: Some(1000) };
        let filters = EmployeeFilters { department_id: None, status: None };
        let result = match self.emp_repo.list(&pag, &filters).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to list employees for IAM user deactivation check: {}", e);
                return;
            }
        };

        let emp_number = format!("EMP-IAM-{}", &user_id[..8.min(user_id.len())]);
        if let Some(emp) = result.0.iter().find(|e| e.employee_number == emp_number) {
            tracing::warn!(
                "Employee {} ({}) corresponds to deactivated IAM user {} — HR should review for termination",
                emp.id, emp.email, user_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;

    async fn setup() -> EmployeeService {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_departments.sql"),
            include_str!("../../migrations/002_create_employees.sql"),
            include_str!("../../migrations/003_create_employment_history.sql"),
        ];
        let migration_names = [
            "001_create_departments.sql",
            "002_create_employees.sql",
            "003_create_employment_history.sql",
        ];
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (filename TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (i, sql) in sql_files.iter().enumerate() {
            let name = migration_names[i];
            let already_applied: bool =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _migrations WHERE filename = ?")
                    .bind(name)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    > 0;
            if !already_applied {
                let mut tx = pool.begin().await.unwrap();
                sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query("INSERT INTO _migrations (filename, applied_at) VALUES (?, ?)")
                    .bind(name)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }
        }

        let bus = saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
            .await
            .unwrap();
        EmployeeService::new(pool, bus)
    }

    #[tokio::test]
    async fn test_handle_user_created_auto_creates_employee() {
        let svc = setup().await;

        // Call handle_user_created — it will auto-create the "Unassigned" department
        svc.handle_user_created("user-abc-123", "jdoe", "jdoe@example.com").await;

        // Verify employee was created
        let employees = svc.list_employees(
            &PaginationParams { page: Some(1), per_page: Some(50) },
            &EmployeeFilters { department_id: None, status: None },
        ).await.unwrap();

        assert_eq!(employees.data.len(), 1);
        let emp = &employees.data[0];
        assert_eq!(emp.first_name, "jdoe");
        assert_eq!(emp.last_name, "-");
        assert_eq!(emp.email, "jdoe@example.com");
        assert_eq!(emp.job_title, "Unassigned");
        assert!(emp.employee_number.starts_with("EMP-IAM-"));

        // Verify the Unassigned department was auto-created
        let departments = svc.list_departments().await.unwrap();
        assert_eq!(departments.len(), 1);
        assert_eq!(departments[0].name, "Unassigned");
    }

    #[tokio::test]
    async fn test_handle_user_created_duplicate_graceful() {
        let svc = setup().await;

        // First creation should succeed
        svc.handle_user_created("user-dup-001", "jsmith", "jsmith@example.com").await;

        // Duplicate creation (same email) should not panic — handled gracefully
        svc.handle_user_created("user-dup-002", "jsmith", "jsmith@example.com").await;

        // Verify only one employee was created with that email
        let employees = svc.list_employees(
            &PaginationParams { page: Some(1), per_page: Some(50) },
            &EmployeeFilters { department_id: None, status: None },
        ).await.unwrap();

        let jscount = employees.data.iter().filter(|e| e.email == "jsmith@example.com").count();
        assert_eq!(jscount, 1);
    }

    #[tokio::test]
    async fn test_handle_user_updated_syncs_email() {
        let svc = setup().await;

        // Auto-create employee from IAM user
        svc.handle_user_created("user-upd-001", "jane", "jane@old.com").await;

        // Verify initial email
        let employees = svc.list_employees(
            &PaginationParams { page: Some(1), per_page: Some(50) },
            &EmployeeFilters { department_id: None, status: None },
        ).await.unwrap();
        let emp = &employees.data[0];
        assert_eq!(emp.email, "jane@old.com");
        let emp_id = emp.id.clone();

        // Simulate IAM user update with new email
        svc.handle_user_updated("user-upd-001", "jane", "jane@new.com").await;

        // Verify email was synced
        let updated = svc.get_employee(&emp_id).await.unwrap();
        assert_eq!(updated.email, "jane@new.com");
    }

    #[tokio::test]
    async fn test_handle_user_updated_no_matching_employee() {
        let svc = setup().await;

        // Should not panic when no employee exists
        svc.handle_user_updated("user-nomatch", "nobody", "nobody@example.com").await;
    }

    #[tokio::test]
    async fn test_handle_user_deactivated_flags_employee() {
        let svc = setup().await;

        // Auto-create employee from IAM user
        svc.handle_user_created("user-deact-01", "bob", "bob@example.com").await;

        // Should not panic — just logs a warning
        svc.handle_user_deactivated("user-deact-01", "bob").await;

        // Employee should still exist (not auto-terminated)
        let employees = svc.list_employees(
            &PaginationParams { page: Some(1), per_page: Some(50) },
            &EmployeeFilters { department_id: None, status: None },
        ).await.unwrap();
        assert_eq!(employees.data.len(), 1);
    }
}

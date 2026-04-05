use crate::models::employee::{CreateEmployee, EmployeeFilters, EmployeeResponse};
use saas_common::error::{AppError, AppResult};
use saas_common::pagination::PaginationParams;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct EmployeeRepo {
    pool: SqlitePool,
}

impl EmployeeRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        pag: &PaginationParams,
        filters: &EmployeeFilters,
    ) -> AppResult<(Vec<EmployeeResponse>, u64)> {
        let offset = pag.offset();
        let limit = pag.per_page();
        let rows = sqlx::query_as::<_, EmployeeResponse>(
            "SELECT * FROM employees WHERE (? IS NULL OR department_id = ?) AND (? IS NULL OR status = ?) ORDER BY last_name, first_name LIMIT ? OFFSET ?"
        )
        .bind(&filters.department_id).bind(&filters.department_id)
        .bind(&filters.status).bind(&filters.status)
        .bind(limit).bind(offset)
        .fetch_all(&self.pool).await?;

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM employees WHERE (? IS NULL OR department_id = ?) AND (? IS NULL OR status = ?)"
        )
        .bind(&filters.department_id).bind(&filters.department_id)
        .bind(&filters.status).bind(&filters.status)
        .fetch_one(&self.pool).await?;

        Ok((rows, count.0 as u64))
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<EmployeeResponse> {
        sqlx::query_as::<_, EmployeeResponse>("SELECT * FROM employees WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Employee {} not found", id)))
    }

    pub async fn create(&self, input: &CreateEmployee) -> AppResult<EmployeeResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO employees (id, first_name, last_name, email, phone, hire_date, termination_date, status, department_id, reports_to, job_title, employee_number, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NULL, 'active', ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.first_name).bind(&input.last_name).bind(&input.email)
        .bind(&input.phone).bind(&input.hire_date).bind(&input.department_id)
        .bind(&input.reports_to).bind(&input.job_title).bind(&input.employee_number)
        .bind(&now).bind(&now)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn update(
        &self,
        id: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        email: Option<&str>,
        phone: Option<&str>,
        department_id: Option<&str>,
        reports_to: Option<&str>,
        job_title: Option<&str>,
        status: Option<&str>,
    ) -> AppResult<EmployeeResponse> {
        let current = self.get_by_id(id).await?;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE employees SET first_name=?, last_name=?, email=?, phone=?, department_id=?, reports_to=?, job_title=?, status=?, updated_at=? WHERE id=?"
        )
        .bind(first_name.unwrap_or(&current.first_name))
        .bind(last_name.unwrap_or(&current.last_name))
        .bind(email.unwrap_or(&current.email))
        .bind(phone.or(current.phone.as_deref()))
        .bind(department_id.unwrap_or(&current.department_id))
        .bind(reports_to.or(current.reports_to.as_deref()))
        .bind(job_title.unwrap_or(&current.job_title))
        .bind(status.unwrap_or(&current.status))
        .bind(&now).bind(id)
        .execute(&self.pool).await?;
        self.get_by_id(id).await
    }

    pub async fn terminate(&self, id: &str, termination_date: &str) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE employees SET status='terminated', termination_date=?, updated_at=? WHERE id=?",
        )
        .bind(termination_date)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_direct_reports(&self, manager_id: &str) -> AppResult<Vec<EmployeeResponse>> {
        let rows = sqlx::query_as::<_, EmployeeResponse>(
            "SELECT * FROM employees WHERE reports_to = ? ORDER BY last_name, first_name",
        )
        .bind(manager_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_org_chart(&self) -> AppResult<Vec<crate::models::department::OrgChartNode>> {
        let rows = sqlx::query_as::<_, crate::models::department::OrgChartNode>(
            "SELECT e.id, e.employee_number, e.first_name, e.last_name, e.job_title, e.department_id, e.reports_to, d.name as department_name FROM employees e LEFT JOIN departments d ON e.department_id = d.id WHERE e.status = 'active' ORDER BY e.last_name, e.first_name"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }
}

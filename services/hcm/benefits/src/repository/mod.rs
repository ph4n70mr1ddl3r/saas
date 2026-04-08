use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct BenefitsRepo {
    pool: SqlitePool,
}

impl BenefitsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Benefit Plans ---

    pub async fn list_plans(&self) -> AppResult<Vec<BenefitPlan>> {
        let rows = sqlx::query_as::<_, BenefitPlan>(
            "SELECT id, name, plan_type, description, employer_contribution_cents, employee_contribution_cents, is_active, created_at FROM benefit_plans ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_plan(&self, id: &str) -> AppResult<BenefitPlan> {
        sqlx::query_as::<_, BenefitPlan>(
            "SELECT id, name, plan_type, description, employer_contribution_cents, employee_contribution_cents, is_active, created_at FROM benefit_plans WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Benefit plan '{}' not found", id)))
    }

    pub async fn create_plan(&self, input: &CreatePlanRequest) -> AppResult<BenefitPlan> {
        let id = uuid::Uuid::new_v4().to_string();
        let employer_contribution = input.employer_contribution_cents.unwrap_or(0);
        let employee_contribution = input.employee_contribution_cents.unwrap_or(0);
        let is_active = input.is_active.unwrap_or(true);
        sqlx::query(
            "INSERT INTO benefit_plans (id, name, plan_type, description, employer_contribution_cents, employee_contribution_cents, is_active) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.plan_type)
        .bind(&input.description)
        .bind(employer_contribution)
        .bind(employee_contribution)
        .bind(is_active)
        .execute(&self.pool)
        .await?;
        self.get_plan(&id).await
    }

    pub async fn update_plan(&self, id: &str, input: &UpdatePlanRequest) -> AppResult<BenefitPlan> {
        let existing = self.get_plan(id).await?;
        let name = input.name.as_deref().unwrap_or(&existing.name);
        let plan_type = input.plan_type.as_deref().unwrap_or(&existing.plan_type);
        let description = input.description.as_ref().or(existing.description.as_ref());
        let employer_contribution = input
            .employer_contribution_cents
            .unwrap_or(existing.employer_contribution_cents);
        let employee_contribution = input
            .employee_contribution_cents
            .unwrap_or(existing.employee_contribution_cents);
        let is_active = input.is_active.unwrap_or(existing.is_active);

        sqlx::query(
            "UPDATE benefit_plans SET name = ?, plan_type = ?, description = ?, employer_contribution_cents = ?, employee_contribution_cents = ?, is_active = ? WHERE id = ?"
        )
        .bind(name)
        .bind(plan_type)
        .bind(description)
        .bind(employer_contribution)
        .bind(employee_contribution)
        .bind(is_active)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_plan(id).await
    }

    // --- Enrollments ---

    pub async fn list_enrollments(&self) -> AppResult<Vec<Enrollment>> {
        let rows = sqlx::query_as::<_, Enrollment>(
            "SELECT id, employee_id, plan_id, status, enrolled_at, cancelled_at FROM enrollments ORDER BY enrolled_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_enrollment(&self, id: &str) -> AppResult<Enrollment> {
        sqlx::query_as::<_, Enrollment>(
            "SELECT id, employee_id, plan_id, status, enrolled_at, cancelled_at FROM enrollments WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Enrollment '{}' not found", id)))
    }

    pub async fn list_enrollments_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Enrollment>> {
        let rows = sqlx::query_as::<_, Enrollment>(
            "SELECT id, employee_id, plan_id, status, enrolled_at, cancelled_at FROM enrollments WHERE employee_id = ? ORDER BY enrolled_at DESC"
        )
        .bind(employee_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_active_enrollment(
        &self,
        employee_id: &str,
        plan_id: &str,
    ) -> AppResult<Option<Enrollment>> {
        let row = sqlx::query_as::<_, Enrollment>(
            "SELECT id, employee_id, plan_id, status, enrolled_at, cancelled_at FROM enrollments WHERE employee_id = ? AND plan_id = ? AND status = 'active'"
        )
        .bind(employee_id)
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn create_enrollment(
        &self,
        input: &CreateEnrollmentRequest,
    ) -> AppResult<Enrollment> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO enrollments (id, employee_id, plan_id) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(&input.employee_id)
            .bind(&input.plan_id)
            .execute(&self.pool)
            .await?;
        self.get_enrollment(&id).await
    }

    pub async fn list_active_enrollments_by_plan(
        &self,
        plan_id: &str,
    ) -> AppResult<Vec<Enrollment>> {
        let rows = sqlx::query_as::<_, Enrollment>(
            "SELECT id, employee_id, plan_id, status, enrolled_at, cancelled_at FROM enrollments WHERE plan_id = ? AND status = 'active'"
        )
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn cancel_enrollment(&self, id: &str) -> AppResult<Enrollment> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE enrollments SET status = 'cancelled', cancelled_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_enrollment(id).await
    }
}

use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use crate::models::*;
use crate::repository::BenefitsRepo;

#[derive(Clone)]
pub struct BenefitsService {
    repo: BenefitsRepo,
    #[allow(dead_code)]
    bus: NatsBus,
}

impl BenefitsService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: BenefitsRepo::new(pool),
            bus,
        }
    }

    // --- Plans ---

    pub async fn list_plans(&self) -> AppResult<Vec<BenefitPlan>> {
        self.repo.list_plans().await
    }

    pub async fn get_plan(&self, id: &str) -> AppResult<BenefitPlan> {
        self.repo.get_plan(id).await
    }

    pub async fn create_plan(&self, input: CreatePlanRequest) -> AppResult<BenefitPlan> {
        self.repo.create_plan(&input).await
    }

    pub async fn update_plan(&self, id: &str, input: UpdatePlanRequest) -> AppResult<BenefitPlan> {
        self.repo.update_plan(id, &input).await
    }

    // --- Enrollments ---

    pub async fn list_enrollments(&self) -> AppResult<Vec<Enrollment>> {
        self.repo.list_enrollments().await
    }

    pub async fn create_enrollment(&self, input: CreateEnrollmentRequest) -> AppResult<Enrollment> {
        let plan = self.repo.get_plan(&input.plan_id).await?;
        if !plan.is_active {
            return Err(AppError::Validation(format!("Plan '{}' is not active", input.plan_id)));
        }
        self.repo.create_enrollment(&input).await
    }

    pub async fn list_enrollments_by_employee(&self, employee_id: &str) -> AppResult<Vec<Enrollment>> {
        self.repo.list_enrollments_by_employee(employee_id).await
    }

    pub async fn cancel_enrollment(&self, id: &str) -> AppResult<Enrollment> {
        let enrollment = self.repo.get_enrollment(id).await?;
        if enrollment.status == "cancelled" {
            return Err(AppError::Validation(format!("Enrollment '{}' is already cancelled", id)));
        }
        self.repo.cancel_enrollment(id).await
    }
}

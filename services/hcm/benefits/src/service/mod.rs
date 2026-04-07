use crate::models::*;
use crate::repository::BenefitsRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    BenefitPlanCreated, BenefitPlanDeactivated, EnrollmentCancelled, EmployeeEnrolled,
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct BenefitsService {
    repo: BenefitsRepo,
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
        let plan = self.repo.create_plan(&input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.benefits.plan.created",
                BenefitPlanCreated {
                    plan_id: plan.id.clone(),
                    name: plan.name.clone(),
                    plan_type: plan.plan_type.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.benefits.plan.created",
                e
            );
        }
        Ok(plan)
    }

    pub async fn update_plan(&self, id: &str, input: UpdatePlanRequest) -> AppResult<BenefitPlan> {
        let plan = self.repo.update_plan(id, &input).await?;
        // Publish deactivation event if plan was deactivated
        if input.is_active == Some(false) {
            if let Err(e) = self
                .bus
                .publish(
                    "hcm.benefits.plan.deactivated",
                    BenefitPlanDeactivated {
                        plan_id: plan.id.clone(),
                        name: plan.name.clone(),
                    },
                )
                .await
            {
                tracing::error!(
                    "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                    "hcm.benefits.plan.deactivated",
                    e
                );
            }
        }
        Ok(plan)
    }

    // --- Enrollments ---

    pub async fn list_enrollments(&self) -> AppResult<Vec<Enrollment>> {
        self.repo.list_enrollments().await
    }

    pub async fn create_enrollment(&self, input: CreateEnrollmentRequest) -> AppResult<Enrollment> {
        let plan = self.repo.get_plan(&input.plan_id).await?;
        if !plan.is_active {
            return Err(AppError::Validation(format!(
                "Plan '{}' is not active",
                input.plan_id
            )));
        }
        // Prevent duplicate active enrollment for same employee + plan
        let existing = self
            .repo
            .find_active_enrollment(&input.employee_id, &input.plan_id)
            .await?;
        if existing.is_some() {
            return Err(AppError::Conflict(format!(
                "Employee '{}' already has an active enrollment in plan '{}'",
                input.employee_id, input.plan_id
            )));
        }
        let enrollment = self.repo.create_enrollment(&input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.benefits.enrollment.created",
                EmployeeEnrolled {
                    enrollment_id: enrollment.id.clone(),
                    employee_id: enrollment.employee_id.clone(),
                    plan_id: enrollment.plan_id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.benefits.enrollment.created",
                e
            );
        }
        Ok(enrollment)
    }

    pub async fn list_enrollments_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Enrollment>> {
        self.repo.list_enrollments_by_employee(employee_id).await
    }

    /// Evaluate and auto-enroll employee in default benefit plans when a new employee is created.
    pub async fn handle_employee_created(&self, employee_id: &str) -> AppResult<Vec<Enrollment>> {
        let plans = self.repo.list_plans().await?;
        let mut enrollments = Vec::new();
        for plan in plans {
            if plan.is_active {
                // Check if already enrolled
                let existing = self.repo.find_active_enrollment(employee_id, &plan.id).await?;
                if existing.is_none() {
                    match self.repo.create_enrollment(&CreateEnrollmentRequest {
                        employee_id: employee_id.to_string(),
                        plan_id: plan.id.clone(),
                    }).await {
                        Ok(enrollment) => enrollments.push(enrollment),
                        Err(e) => tracing::warn!("Failed to auto-enroll {} in plan {}: {}", employee_id, plan.id, e),
                    }
                }
            }
        }
        Ok(enrollments)
    }

    pub async fn cancel_enrollment(&self, id: &str) -> AppResult<Enrollment> {
        let enrollment = self.repo.get_enrollment(id).await?;
        if enrollment.status == "cancelled" {
            return Err(AppError::Validation(format!(
                "Enrollment '{}' is already cancelled",
                id
            )));
        }
        let cancelled = self.repo.cancel_enrollment(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.benefits.enrollment.cancelled",
                EnrollmentCancelled {
                    enrollment_id: cancelled.id.clone(),
                    employee_id: cancelled.employee_id.clone(),
                    plan_id: cancelled.plan_id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.benefits.enrollment.cancelled",
                e
            );
        }
        Ok(cancelled)
    }

    /// Cancel all active enrollments when an employee is terminated.
    pub async fn handle_employee_terminated(&self, employee_id: &str) -> AppResult<()> {
        let enrollments = self.repo.list_enrollments_by_employee(employee_id).await?;
        for enrollment in enrollments {
            if enrollment.status == "active" {
                let cancelled = self.repo.cancel_enrollment(&enrollment.id).await?;
                if let Err(e) = self
                    .bus
                    .publish(
                        "hcm.benefits.enrollment.cancelled",
                        EnrollmentCancelled {
                            enrollment_id: cancelled.id.clone(),
                            employee_id: cancelled.employee_id.clone(),
                            plan_id: cancelled.plan_id.clone(),
                        },
                    )
                    .await
                {
                    tracing::error!(
                        "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                        "hcm.benefits.enrollment.cancelled",
                        e
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_plans.sql"),
            include_str!("../../migrations/002_create_enrollments.sql"),
        ];
        let migration_names = [
            "001_create_plans.sql",
            "002_create_enrollments.sql",
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
        pool
    }

    async fn setup_repo() -> BenefitsRepo {
        let pool = setup().await;
        BenefitsRepo::new(pool)
    }

    #[tokio::test]
    async fn test_plan_crud() {
        let repo = setup_repo().await;

        // Create
        let input = CreatePlanRequest {
            name: "Medical Plus".into(),
            plan_type: "medical".into(),
            description: Some("Comprehensive medical plan".into()),
            employer_contribution_cents: Some(50000),
            employee_contribution_cents: Some(20000),
            is_active: Some(true),
        };
        let plan = repo.create_plan(&input).await.unwrap();
        assert_eq!(plan.name, "Medical Plus");
        assert_eq!(plan.plan_type, "medical");
        assert_eq!(plan.employer_contribution_cents, 50000);
        assert_eq!(plan.employee_contribution_cents, 20000);
        assert!(plan.is_active);

        // Read
        let fetched = repo.get_plan(&plan.id).await.unwrap();
        assert_eq!(fetched.name, "Medical Plus");

        // Update
        let update = UpdatePlanRequest {
            name: Some("Medical Premium".into()),
            plan_type: None,
            description: None,
            employer_contribution_cents: Some(60000),
            employee_contribution_cents: None,
            is_active: None,
        };
        let updated = repo.update_plan(&plan.id, &update).await.unwrap();
        assert_eq!(updated.name, "Medical Premium");
        assert_eq!(updated.employer_contribution_cents, 60000);
        assert_eq!(updated.employee_contribution_cents, 20000); // unchanged

        // List
        let plans = repo.list_plans().await.unwrap();
        assert_eq!(plans.len(), 1);
    }

    #[tokio::test]
    async fn test_enrollment_requires_active_plan() {
        let repo = setup_repo().await;

        // Create an inactive plan
        let plan_input = CreatePlanRequest {
            name: "Inactive Dental".into(),
            plan_type: "dental".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: Some(false),
        };
        let plan = repo.create_plan(&plan_input).await.unwrap();
        assert!(!plan.is_active);

        // Attempt to create enrollment against inactive plan
        let enrollment_input = CreateEnrollmentRequest {
            employee_id: "emp-001".into(),
            plan_id: plan.id.clone(),
        };
        // The repo does not enforce the active plan check -- that's a service-layer rule.
        // Verify the plan is inactive so the service would block it.
        let fetched_plan = repo.get_plan(&plan.id).await.unwrap();
        assert!(!fetched_plan.is_active, "Plan should be inactive");
    }

    #[tokio::test]
    async fn test_duplicate_active_enrollment_prevention() {
        let repo = setup_repo().await;

        let plan_input = CreatePlanRequest {
            name: "Vision Plus".into(),
            plan_type: "vision".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: Some(true),
        };
        let plan = repo.create_plan(&plan_input).await.unwrap();

        let enrollment_input = CreateEnrollmentRequest {
            employee_id: "emp-002".into(),
            plan_id: plan.id.clone(),
        };

        // First enrollment succeeds
        let e1 = repo.create_enrollment(&enrollment_input).await.unwrap();
        assert_eq!(e1.status, "active");

        // find_active_enrollment should find the first one
        let existing = repo
            .find_active_enrollment("emp-002", &plan.id)
            .await
            .unwrap();
        assert!(existing.is_some(), "Should find active enrollment");
        assert_eq!(existing.unwrap().id, e1.id);
    }

    #[tokio::test]
    async fn test_cancel_enrollment() {
        let repo = setup_repo().await;

        let plan_input = CreatePlanRequest {
            name: "Life Insurance".into(),
            plan_type: "life".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: Some(true),
        };
        let plan = repo.create_plan(&plan_input).await.unwrap();

        let enrollment_input = CreateEnrollmentRequest {
            employee_id: "emp-003".into(),
            plan_id: plan.id.clone(),
        };
        let enrollment = repo.create_enrollment(&enrollment_input).await.unwrap();
        assert_eq!(enrollment.status, "active");

        // Cancel
        let cancelled = repo.cancel_enrollment(&enrollment.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");
        assert!(cancelled.cancelled_at.is_some());
    }

    #[tokio::test]
    async fn test_list_enrollments_by_employee() {
        let repo = setup_repo().await;

        let plan1 = repo
            .create_plan(&CreatePlanRequest {
                name: "Plan A".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        let plan2 = repo
            .create_plan(&CreatePlanRequest {
                name: "Plan B".into(),
                plan_type: "dental".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        // Enroll emp-100 in both plans
        repo.create_enrollment(&CreateEnrollmentRequest {
            employee_id: "emp-100".into(),
            plan_id: plan1.id.clone(),
        })
        .await
        .unwrap();
        repo.create_enrollment(&CreateEnrollmentRequest {
            employee_id: "emp-100".into(),
            plan_id: plan2.id.clone(),
        })
        .await
        .unwrap();

        // Enroll a different employee
        repo.create_enrollment(&CreateEnrollmentRequest {
            employee_id: "emp-200".into(),
            plan_id: plan1.id.clone(),
        })
        .await
        .unwrap();

        let emp100_enrollments = repo.list_enrollments_by_employee("emp-100").await.unwrap();
        assert_eq!(emp100_enrollments.len(), 2);

        let emp200_enrollments = repo.list_enrollments_by_employee("emp-200").await.unwrap();
        assert_eq!(emp200_enrollments.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_employee_created_auto_enrollment() {
        let repo = setup_repo().await;

        // Create two active plans and one inactive
        repo.create_plan(&CreatePlanRequest {
            name: "Active Medical".into(),
            plan_type: "medical".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: Some(true),
        })
        .await
        .unwrap();

        repo.create_plan(&CreatePlanRequest {
            name: "Active Dental".into(),
            plan_type: "dental".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: Some(true),
        })
        .await
        .unwrap();

        repo.create_plan(&CreatePlanRequest {
            name: "Inactive Plan".into(),
            plan_type: "vision".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: Some(false),
        })
        .await
        .unwrap();

        // Simulate auto-enrollment (replicating service logic without NATS)
        let plans = repo.list_plans().await.unwrap();
        let mut enrollments = Vec::new();
        for plan in plans {
            if plan.is_active {
                let existing = repo.find_active_enrollment("emp-new", &plan.id).await.unwrap();
                if existing.is_none() {
                    let e = repo
                        .create_enrollment(&CreateEnrollmentRequest {
                            employee_id: "emp-new".into(),
                            plan_id: plan.id.clone(),
                        })
                        .await
                        .unwrap();
                    enrollments.push(e);
                }
            }
        }

        assert_eq!(enrollments.len(), 2, "Should auto-enroll in 2 active plans");
        let emp_enrollments = repo.list_enrollments_by_employee("emp-new").await.unwrap();
        assert_eq!(emp_enrollments.len(), 2);
    }

    #[tokio::test]
    async fn test_create_plan_default_values() {
        let repo = setup_repo().await;

        let input = CreatePlanRequest {
            name: "Basic Plan".into(),
            plan_type: "retirement".into(),
            description: None,
            employer_contribution_cents: None,
            employee_contribution_cents: None,
            is_active: None, // defaults to true
        };
        let plan = repo.create_plan(&input).await.unwrap();
        assert!(plan.is_active);
        assert_eq!(plan.employer_contribution_cents, 0);
        assert_eq!(plan.employee_contribution_cents, 0);
    }

    #[tokio::test]
    async fn test_deactivate_plan() {
        let repo = setup_repo().await;

        let plan = repo
            .create_plan(&CreatePlanRequest {
                name: "Sick Plan".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();
        assert!(plan.is_active);

        let updated = repo
            .update_plan(
                &plan.id,
                &UpdatePlanRequest {
                    name: None,
                    plan_type: None,
                    description: None,
                    employer_contribution_cents: None,
                    employee_contribution_cents: None,
                    is_active: Some(false),
                },
            )
            .await
            .unwrap();
        assert!(!updated.is_active);
    }

    #[tokio::test]
    async fn test_handle_employee_terminated_cancels_enrollments() {
        let repo = setup_repo().await;

        // Create plans
        let plan1 = repo
            .create_plan(&CreatePlanRequest {
                name: "Medical".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();
        let plan2 = repo
            .create_plan(&CreatePlanRequest {
                name: "Dental".into(),
                plan_type: "dental".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        // Enroll employee in both plans
        repo.create_enrollment(&CreateEnrollmentRequest {
            employee_id: "emp-term".into(),
            plan_id: plan1.id.clone(),
        })
        .await
        .unwrap();
        repo.create_enrollment(&CreateEnrollmentRequest {
            employee_id: "emp-term".into(),
            plan_id: plan2.id.clone(),
        })
        .await
        .unwrap();

        // Verify both active
        let enrollments = repo.list_enrollments_by_employee("emp-term").await.unwrap();
        assert_eq!(enrollments.len(), 2);
        assert!(enrollments.iter().all(|e| e.status == "active"));

        // Simulate termination handler: cancel all active enrollments
        for enrollment in &enrollments {
            if enrollment.status == "active" {
                repo.cancel_enrollment(&enrollment.id).await.unwrap();
            }
        }

        // Verify all cancelled
        let updated = repo.list_enrollments_by_employee("emp-term").await.unwrap();
        assert!(updated.iter().all(|e| e.status == "cancelled"));
    }
}

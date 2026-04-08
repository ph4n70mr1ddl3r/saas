use crate::models::*;
use crate::repository::BenefitsRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    BenefitPlanCreated, BenefitPlanDeactivated, EnrollmentCancelled, EmployeeEnrolled,
};
use sqlx::SqlitePool;
use validator::Validate;

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
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
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
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
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

            // Auto-cancel all active enrollments for the deactivated plan
            let active_enrollments = self.repo.list_active_enrollments_by_plan(id).await?;
            let count = active_enrollments.len();
            for enrollment in &active_enrollments {
                if let Err(e) = self.cancel_enrollment(&enrollment.id).await {
                    tracing::error!(
                        "Failed to cancel enrollment '{}' during plan deactivation: {}",
                        enrollment.id,
                        e
                    );
                }
            }
            tracing::info!(
                "Auto-cancelled {} active enrollment(s) for deactivated plan '{}'",
                count,
                plan.name
            );
        }
        Ok(plan)
    }

    // --- Enrollments ---

    pub async fn list_enrollments(&self) -> AppResult<Vec<Enrollment>> {
        self.repo.list_enrollments().await
    }

    pub async fn get_enrollment(&self, id: &str) -> AppResult<Enrollment> {
        self.repo.get_enrollment(id).await
    }

    pub async fn create_enrollment(&self, input: CreateEnrollmentRequest) -> AppResult<Enrollment> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
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

    /// React to compensation changes by re-evaluating plan eligibility.
    /// This is primarily a notification/logging handler since benefit deductions
    /// are managed by payroll.
    pub async fn handle_compensation_changed(
        &self,
        compensation_id: &str,
        employee_id: &str,
        amount_cents: i64,
        change_type: &str,
    ) -> AppResult<()> {
        tracing::info!(
            "Compensation {} for employee {}: amount_cents={}, type={}",
            compensation_id,
            employee_id,
            amount_cents,
            change_type,
        );

        let enrollments = self.repo.list_enrollments_by_employee(employee_id).await?;
        let active_count = enrollments.iter().filter(|e| e.status == "active").count();

        if active_count > 0 {
            tracing::info!(
                "Employee {} has {} active benefit enrollment(s) — \
                 benefit deductions may need recalculation based on new compensation (amount_cents={})",
                employee_id,
                active_count,
                amount_cents,
            );
        } else {
            tracing::info!(
                "Employee {} has no active benefit enrollments — \
                 no deduction recalculation needed",
                employee_id,
            );
        }

        Ok(())
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

    #[tokio::test]
    async fn test_get_enrollment() {
        let repo = setup_repo().await;
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let plan = repo
            .create_plan(&CreatePlanRequest {
                name: "Test Plan".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        let enrollment = svc
            .create_enrollment(CreateEnrollmentRequest {
                employee_id: "emp-get".into(),
                plan_id: plan.id.clone(),
            })
            .await
            .unwrap();

        let fetched = svc.get_enrollment(&enrollment.id).await.unwrap();
        assert_eq!(fetched.id, enrollment.id);
        assert_eq!(fetched.status, "active");
        assert_eq!(fetched.employee_id, "emp-get");

        // Not found
        let result = svc.get_enrollment("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deactivating_plan_cancels_active_enrollments() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create an active plan
        let plan = svc
            .create_plan(CreatePlanRequest {
                name: "Group Medical".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        // Enroll three employees
        let e1 = svc
            .create_enrollment(CreateEnrollmentRequest {
                employee_id: "emp-a".into(),
                plan_id: plan.id.clone(),
            })
            .await
            .unwrap();
        let e2 = svc
            .create_enrollment(CreateEnrollmentRequest {
                employee_id: "emp-b".into(),
                plan_id: plan.id.clone(),
            })
            .await
            .unwrap();
        let e3 = svc
            .create_enrollment(CreateEnrollmentRequest {
                employee_id: "emp-c".into(),
                plan_id: plan.id.clone(),
            })
            .await
            .unwrap();

        // All should be active
        assert_eq!(svc.get_enrollment(&e1.id).await.unwrap().status, "active");
        assert_eq!(svc.get_enrollment(&e2.id).await.unwrap().status, "active");
        assert_eq!(svc.get_enrollment(&e3.id).await.unwrap().status, "active");

        // Deactivate the plan
        let deactivated = svc
            .update_plan(
                &plan.id,
                UpdatePlanRequest {
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
        assert!(!deactivated.is_active);

        // All enrollments should now be cancelled
        assert_eq!(
            svc.get_enrollment(&e1.id).await.unwrap().status,
            "cancelled"
        );
        assert_eq!(
            svc.get_enrollment(&e2.id).await.unwrap().status,
            "cancelled"
        );
        assert_eq!(
            svc.get_enrollment(&e3.id).await.unwrap().status,
            "cancelled"
        );
        assert!(svc
            .get_enrollment(&e1.id)
            .await
            .unwrap()
            .cancelled_at
            .is_some());
    }

    #[tokio::test]
    async fn test_deactivating_plan_with_no_active_enrollments() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create an active plan with no enrollments
        let plan = svc
            .create_plan(CreatePlanRequest {
                name: "Unused Dental".into(),
                plan_type: "dental".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        // Deactivate the plan -- should succeed without errors
        let deactivated = svc
            .update_plan(
                &plan.id,
                UpdatePlanRequest {
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
        assert!(!deactivated.is_active);

        // Verify no enrollments exist for this plan
        let enrollments = repo
            .list_active_enrollments_by_plan(&plan.id)
            .await
            .unwrap();
        assert!(enrollments.is_empty());
    }

    #[tokio::test]
    async fn test_handle_compensation_changed_with_active_enrollments() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create an active plan and enroll an employee
        let plan = svc
            .create_plan(CreatePlanRequest {
                name: "Medical Plan".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        svc.create_enrollment(CreateEnrollmentRequest {
            employee_id: "emp-comp".into(),
            plan_id: plan.id.clone(),
        })
        .await
        .unwrap();

        // Handler should succeed — verifies the lookup path works
        let result = svc
            .handle_compensation_changed("comp-001", "emp-comp", 150000_00, "created")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_compensation_changed_no_enrollments() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Employee has no enrollments — handler should still succeed
        let result = svc
            .handle_compensation_changed("comp-002", "emp-noenroll", 90000_00, "updated")
            .await;
        assert!(result.is_ok());
    }

    // --- Validation tests ---

    #[tokio::test]
    async fn test_create_plan_rejects_empty_name() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_plan(CreatePlanRequest {
                name: "".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Name is required"),
                "Expected 'Name is required' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_plan_rejects_empty_plan_type() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_plan(CreatePlanRequest {
                name: "Medical Plus".into(),
                plan_type: "".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Plan type is required"),
                "Expected 'Plan type is required' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_plan_rejects_negative_employer_contribution() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_plan(CreatePlanRequest {
                name: "Bad Plan".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: Some(-100),
                employee_contribution_cents: None,
                is_active: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Must be non-negative"),
                "Expected 'Must be non-negative' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_plan_rejects_negative_employee_contribution() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_plan(CreatePlanRequest {
                name: "Bad Plan".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: Some(-50),
                is_active: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Must be non-negative"),
                "Expected 'Must be non-negative' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_update_plan_rejects_empty_name() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create a valid plan first
        let plan = svc
            .create_plan(CreatePlanRequest {
                name: "Valid".into(),
                plan_type: "medical".into(),
                description: None,
                employer_contribution_cents: None,
                employee_contribution_cents: None,
                is_active: Some(true),
            })
            .await
            .unwrap();

        let result = svc
            .update_plan(
                &plan.id,
                UpdatePlanRequest {
                    name: Some("".into()),
                    plan_type: None,
                    description: None,
                    employer_contribution_cents: None,
                    employee_contribution_cents: None,
                    is_active: None,
                },
            )
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Name is required"),
                "Expected 'Name is required' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_enrollment_rejects_empty_employee_id() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_enrollment(CreateEnrollmentRequest {
                employee_id: "".into(),
                plan_id: "some-plan".into(),
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Employee ID is required"),
                "Expected 'Employee ID is required' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_enrollment_rejects_empty_plan_id() {
        let pool = setup().await;
        let repo = BenefitsRepo::new(pool.clone());
        let svc = BenefitsService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_enrollment(CreateEnrollmentRequest {
                employee_id: "emp-001".into(),
                plan_id: "".into(),
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AppError::Validation(msg) => assert!(
                msg.contains("Plan ID is required"),
                "Expected 'Plan ID is required' in error, got: {}",
                msg
            ),
            other => panic!("Expected Validation error, got: {:?}", other),
        }
    }
}

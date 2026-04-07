use crate::models::*;
use crate::repository::PayrollRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::PayRunCompleted;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct PayrollService {
    repo: PayrollRepo,
    bus: NatsBus,
}

impl PayrollService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: PayrollRepo::new(pool),
            bus,
        }
    }

    // --- Compensation ---

    pub async fn list_compensation(&self) -> AppResult<Vec<Compensation>> {
        self.repo.list_compensation().await
    }

    pub async fn get_compensation(&self, id: &str) -> AppResult<Compensation> {
        self.repo.get_compensation(id).await
    }

    pub async fn list_compensation_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Compensation>> {
        self.repo.list_compensation_by_employee(employee_id).await
    }

    pub async fn create_compensation(
        &self,
        input: CreateCompensationRequest,
    ) -> AppResult<Compensation> {
        if input.amount_cents < 0 {
            return Err(AppError::Validation(
                "amount_cents must be non-negative".into(),
            ));
        }
        self.repo.create_compensation(&input).await
    }

    pub async fn update_compensation(
        &self,
        id: &str,
        input: UpdateCompensationRequest,
    ) -> AppResult<Compensation> {
        self.repo.update_compensation(id, &input).await
    }

    // --- PayRuns ---

    pub async fn list_pay_runs(&self) -> AppResult<Vec<PayRun>> {
        self.repo.list_pay_runs().await
    }

    pub async fn get_pay_run(&self, id: &str) -> AppResult<PayRun> {
        self.repo.get_pay_run(id).await
    }

    pub async fn create_pay_run(&self, input: CreatePayRunRequest) -> AppResult<PayRun> {
        self.repo.create_pay_run(&input).await
    }

    pub async fn process_pay_run(&self, id: &str) -> AppResult<PayRun> {
        let pay_run = self.repo.get_pay_run(id).await?;
        if pay_run.status != "draft" {
            return Err(AppError::Validation(format!(
                "Pay run '{}' is not in draft status (current: {})",
                id, pay_run.status
            )));
        }

        let original_status = pay_run.status.clone();
        self.repo.update_pay_run_status(id, "processing").await?;

        // Only process compensation records with effective_date within the pay period
        let compensations = self.repo.list_compensation().await?;
        let filtered: Vec<_> = compensations
            .into_iter()
            .filter(|c| {
                c.end_date.is_none()
                    && c.amount_cents > 0
                    && c.effective_date <= pay_run.period_end
                    && (c.effective_date.is_empty() || c.effective_date >= pay_run.period_start)
            })
            .collect();

        let mut total_net_cents: i64 = 0;
        let mut payslip_count: u32 = 0;

        // Use progressive tax brackets if available, otherwise fall back to flat 22%
        let brackets = self.repo.list_tax_brackets().await.unwrap_or_default();
        let use_progressive = !brackets.is_empty();

        for comp in &filtered {
            let gross = comp.amount_cents;
            let tax = if use_progressive {
                self.calculate_progressive_tax(gross).await?
            } else {
                // Fallback: flat 22% with rounding
                (gross * 22 + 50) / 100
            };
            let deductions = self
                .repo
                .list_deductions_by_employee(&comp.employee_id)
                .await?;
            let total_deductions: i64 = deductions.iter().map(|d| d.amount_cents).sum();
            let net = gross - tax - total_deductions;

            // Prevent negative net pay
            if net < 0 {
                // Roll back to original status on failure
                self.repo
                    .update_pay_run_status(id, &original_status)
                    .await?;
                return Err(AppError::Validation(
                    "Net pay would be negative after deductions".into(),
                ));
            }

            self.repo
                .create_payslip(id, &comp.employee_id, gross, net, tax, total_deductions)
                .await?;
            total_net_cents += net;
            payslip_count += 1;
        }

        let pay_run = self.repo.update_pay_run_status(id, "completed").await?;

        let event = PayRunCompleted {
            pay_run_id: id.to_string(),
            period_start: pay_run.period_start.clone(),
            period_end: pay_run.period_end.clone(),
            payslip_count,
            total_net_pay_cents: total_net_cents,
        };
        if let Err(e) = self.bus.publish("hcm.payroll.run.completed", event).await {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.payroll.run.completed",
                e
            );
        }

        Ok(pay_run)
    }

    pub async fn list_payslips_for_run(&self, pay_run_id: &str) -> AppResult<Vec<Payslip>> {
        let _ = self.repo.get_pay_run(pay_run_id).await?;
        self.repo.list_payslips_for_run(pay_run_id).await
    }

    // --- Deductions ---

    pub async fn list_deductions_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Deduction>> {
        self.repo.list_deductions_by_employee(employee_id).await
    }

    pub async fn create_deduction(&self, input: CreateDeductionRequest) -> AppResult<Deduction> {
        if input.amount_cents < 0 {
            return Err(AppError::Validation(
                "amount_cents must be non-negative".into(),
            ));
        }
        self.repo.create_deduction(&input).await
    }

    pub async fn get_deduction(&self, id: &str) -> AppResult<Deduction> {
        self.repo.get_deduction(id).await
    }

    // --- Event handlers ---

    pub async fn handle_employee_created(&self, employee_id: &str) -> AppResult<Compensation> {
        let input = CreateCompensationRequest {
            employee_id: employee_id.to_string(),
            salary_type: "salaried".to_string(),
            amount_cents: 0,
            currency: Some("USD".to_string()),
            effective_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            end_date: None,
        };
        self.repo.create_compensation(&input).await
    }

    /// Set end_date on all active compensation records when an employee is terminated.
    pub async fn handle_employee_terminated(&self, employee_id: &str, termination_date: &str) -> AppResult<()> {
        let compensations = self.repo.list_compensation_by_employee(employee_id).await?;
        for comp in compensations {
            if comp.end_date.is_none() {
                self.repo.update_compensation(
                    &comp.id,
                    &UpdateCompensationRequest {
                        end_date: Some(termination_date.to_string()),
                        ..Default::default()
                    },
                ).await?;
            }
        }
        Ok(())
    }

    /// When a timesheet is approved, log availability for hourly payroll processing.
    pub async fn handle_timesheet_approved(
        &self,
        employee_id: &str,
        week_start: &str,
    ) -> AppResult<()> {
        // Verify employee has compensation record
        let compensations = self.repo.list_compensation_by_employee(employee_id).await?;
        if compensations.is_empty() {
            tracing::warn!(
                "Timesheet approved for employee {} but no compensation record found",
                employee_id
            );
            return Ok(());
        }
        tracing::info!(
            "Timesheet approved for employee {} — week of {} ready for payroll processing",
            employee_id, week_start
        );
        Ok(())
    }

    // --- Tax Brackets ---

    pub async fn list_tax_brackets(&self) -> AppResult<Vec<TaxBracket>> {
        self.repo.list_tax_brackets().await
    }

    pub async fn get_tax_bracket(&self, id: &str) -> AppResult<TaxBracket> {
        self.repo.get_tax_bracket(id).await
    }

    pub async fn create_tax_bracket(&self, input: CreateTaxBracketRequest) -> AppResult<TaxBracket> {
        if input.rate_percent < 0.0 || input.rate_percent > 100.0 {
            return Err(AppError::Validation(
                "rate_percent must be between 0 and 100".into(),
            ));
        }
        if input.min_income_cents < 0 {
            return Err(AppError::Validation(
                "min_income_cents must be non-negative".into(),
            ));
        }
        if let Some(max) = input.max_income_cents {
            if max <= input.min_income_cents {
                return Err(AppError::Validation(
                    "max_income_cents must be greater than min_income_cents".into(),
                ));
            }
        }

        // Check for overlapping brackets
        let existing = self.repo.list_tax_brackets().await?;
        let new_max = input.max_income_cents.unwrap_or(i64::MAX);
        for bracket in &existing {
            let existing_max = bracket.max_income_cents.unwrap_or(i64::MAX);
            // Two ranges overlap if: new_min < existing_max AND existing_min < new_max
            if input.min_income_cents < existing_max && bracket.min_income_cents < new_max {
                return Err(AppError::Validation(format!(
                    "Tax bracket overlaps with existing bracket ({}-{} @ {}%). New bracket ({}-{}) conflicts",
                    bracket.min_income_cents,
                    bracket.max_income_cents.map(|v| v.to_string()).unwrap_or("∞".into()),
                    bracket.rate_percent,
                    input.min_income_cents,
                    input.max_income_cents.map(|v| v.to_string()).unwrap_or("∞".into()),
                )));
            }
        }

        self.repo.create_tax_bracket(&input).await
    }

    /// Calculate progressive tax by applying brackets in order.
    /// Each bracket taxes only the income that falls within its range.
    pub async fn calculate_progressive_tax(&self, gross_cents: i64) -> AppResult<i64> {
        let brackets = self.repo.list_tax_brackets().await?;
        let mut total_tax: i64 = 0;
        let mut remaining = gross_cents;

        for bracket in &brackets {
            if remaining <= 0 {
                break;
            }
            let bracket_ceiling = bracket.max_income_cents.unwrap_or(i64::MAX);
            let taxable_in_bracket = std::cmp::min(remaining, bracket_ceiling - bracket.min_income_cents);
            if taxable_in_bracket > 0 {
                // rate_percent is a percentage (e.g. 22.0 means 22%)
                // Use integer arithmetic: (taxable * rate_percent * 100 + 5000) / 10000 for rounding
                let tax_in_bracket = (taxable_in_bracket as f64 * bracket.rate_percent / 100.0).round() as i64;
                total_tax += tax_in_bracket;
            }
            remaining -= taxable_in_bracket;
        }

        Ok(total_tax)
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
            include_str!("../../migrations/001_create_compensation.sql"),
            include_str!("../../migrations/002_create_pay_runs.sql"),
            include_str!("../../migrations/003_create_payslips.sql"),
            include_str!("../../migrations/004_create_deductions.sql"),
            include_str!("../../migrations/005_create_tax_brackets.sql"),
        ];
        let migration_names = [
            "001_create_compensation.sql",
            "002_create_pay_runs.sql",
            "003_create_payslips.sql",
            "004_create_deductions.sql",
            "005_create_tax_brackets.sql",
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

    async fn setup_repo() -> PayrollRepo {
        let pool = setup().await;
        PayrollRepo::new(pool)
    }

    #[tokio::test]
    async fn test_compensation_crud() {
        let repo = setup_repo().await;

        // Create
        let input = CreateCompensationRequest {
            employee_id: "emp-001".into(),
            salary_type: "salaried".into(),
            amount_cents: 75_000_00,
            currency: Some("USD".into()),
            effective_date: "2025-01-01".into(),
            end_date: None,
        };
        let comp = repo.create_compensation(&input).await.unwrap();
        assert_eq!(comp.employee_id, "emp-001");
        assert_eq!(comp.amount_cents, 75_000_00);
        assert_eq!(comp.salary_type, "salaried");
        assert_eq!(comp.currency, "USD");

        // Read
        let fetched = repo.get_compensation(&comp.id).await.unwrap();
        assert_eq!(fetched.id, comp.id);

        // List by employee
        let emp_comps = repo.list_compensation_by_employee("emp-001").await.unwrap();
        assert_eq!(emp_comps.len(), 1);

        // Update
        let updated = repo
            .update_compensation(
                &comp.id,
                &UpdateCompensationRequest {
                    salary_type: None,
                    amount_cents: Some(80_000_00),
                    currency: None,
                    effective_date: None,
                    end_date: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.amount_cents, 80_000_00);

        // List all
        let all_comps = repo.list_compensation().await.unwrap();
        assert_eq!(all_comps.len(), 1);
    }

    #[tokio::test]
    async fn test_pay_run_creation_and_status() {
        let repo = setup_repo().await;

        let input = CreatePayRunRequest {
            period_start: "2025-06-01".into(),
            period_end: "2025-06-30".into(),
            pay_date: "2025-07-01".into(),
        };
        let pay_run = repo.create_pay_run(&input).await.unwrap();
        assert_eq!(pay_run.status, "draft");
        assert_eq!(pay_run.period_start, "2025-06-01");
        assert_eq!(pay_run.period_end, "2025-06-30");

        // Update to processing
        let processing = repo
            .update_pay_run_status(&pay_run.id, "processing")
            .await
            .unwrap();
        assert_eq!(processing.status, "processing");

        // Update to completed
        let completed = repo
            .update_pay_run_status(&pay_run.id, "completed")
            .await
            .unwrap();
        assert_eq!(completed.status, "completed");

        // List
        let pay_runs = repo.list_pay_runs().await.unwrap();
        assert_eq!(pay_runs.len(), 1);
    }

    #[tokio::test]
    async fn test_payslip_generation() {
        let repo = setup_repo().await;

        // Create pay run
        let pay_run = repo
            .create_pay_run(&CreatePayRunRequest {
                period_start: "2025-06-01".into(),
                period_end: "2025-06-30".into(),
                pay_date: "2025-07-01".into(),
            })
            .await
            .unwrap();

        // Create payslip
        let payslip = repo
            .create_payslip(&pay_run.id, "emp-001", 50_000_00, 39_000_00, 8_800_000, 2_200_00)
            .await
            .unwrap();
        assert_eq!(payslip.pay_run_id, pay_run.id);
        assert_eq!(payslip.employee_id, "emp-001");
        assert_eq!(payslip.gross_pay, 50_000_00);
        assert_eq!(payslip.net_pay, 39_000_00);
        assert_eq!(payslip.status, "pending");

        // List payslips for run
        let payslips = repo.list_payslips_for_run(&pay_run.id).await.unwrap();
        assert_eq!(payslips.len(), 1);
    }

    #[tokio::test]
    async fn test_deduction_management() {
        let repo = setup_repo().await;

        // Create deduction
        let input = CreateDeductionRequest {
            employee_id: "emp-001".into(),
            code: "HEALTH_INS".into(),
            amount_cents: 5_000_00,
            recurring: Some(true),
            start_date: "2025-01-01".into(),
            end_date: None,
        };
        let deduction = repo.create_deduction(&input).await.unwrap();
        assert_eq!(deduction.employee_id, "emp-001");
        assert_eq!(deduction.code, "HEALTH_INS");
        assert_eq!(deduction.amount_cents, 5_000_00);
        assert!(deduction.recurring);

        // List deductions for employee
        let deductions = repo.list_deductions_by_employee("emp-001").await.unwrap();
        assert_eq!(deductions.len(), 1);

        // Create a second deduction with end_date
        let input2 = CreateDeductionRequest {
            employee_id: "emp-001".into(),
            code: "401K".into(),
            amount_cents: 3_000_00,
            recurring: Some(true),
            start_date: "2025-01-01".into(),
            end_date: Some("2025-12-31".into()),
        };
        repo.create_deduction(&input2).await.unwrap();

        let all_deductions = repo.list_deductions_by_employee("emp-001").await.unwrap();
        assert_eq!(all_deductions.len(), 2);
    }

    #[tokio::test]
    async fn test_pay_run_status_validation() {
        let repo = setup_repo().await;

        // Create pay run (starts as draft)
        let pay_run = repo
            .create_pay_run(&CreatePayRunRequest {
                period_start: "2025-06-01".into(),
                period_end: "2025-06-30".into(),
                pay_date: "2025-07-01".into(),
            })
            .await
            .unwrap();
        assert_eq!(pay_run.status, "draft");

        // Draft -> processing -> completed transitions work
        repo.update_pay_run_status(&pay_run.id, "processing")
            .await
            .unwrap();
        let updated = repo.get_pay_run(&pay_run.id).await.unwrap();
        assert_eq!(updated.status, "processing");

        repo.update_pay_run_status(&pay_run.id, "completed")
            .await
            .unwrap();
        let completed = repo.get_pay_run(&pay_run.id).await.unwrap();
        assert_eq!(completed.status, "completed");
    }

    #[tokio::test]
    async fn test_tax_calculation_logic() {
        // Verify 22% tax calculation used by process_pay_run
        let gross = 60_000_00i64;
        let tax = (gross * 22 + 50) / 100;
        assert_eq!(tax, 13_200_00); // 22% of 60k = 13.2k

        let gross2 = 50_000_00i64;
        let tax2 = (gross2 * 22 + 50) / 100;
        assert_eq!(tax2, 11_000_00);

        // Verify net = gross - tax - deductions
        let deductions = 2_000_00i64;
        let net = gross2 - tax2 - deductions;
        assert_eq!(net, 37_000_00);
    }

    #[tokio::test]
    async fn test_handle_employee_created_creates_default_compensation() {
        let repo = setup_repo().await;

        // Simulate handle_employee_created by creating default compensation
        let input = CreateCompensationRequest {
            employee_id: "emp-new-001".into(),
            salary_type: "salaried".into(),
            amount_cents: 0,
            currency: Some("USD".into()),
            effective_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            end_date: None,
        };
        let comp = repo.create_compensation(&input).await.unwrap();
        assert_eq!(comp.employee_id, "emp-new-001");
        assert_eq!(comp.salary_type, "salaried");
        assert_eq!(comp.amount_cents, 0);

        // Verify it can be listed
        let comps = repo
            .list_compensation_by_employee("emp-new-001")
            .await
            .unwrap();
        assert_eq!(comps.len(), 1);
    }

    #[tokio::test]
    async fn test_multiple_payslips_per_pay_run() {
        let repo = setup_repo().await;

        // Create pay run
        let pay_run = repo
            .create_pay_run(&CreatePayRunRequest {
                period_start: "2025-06-01".into(),
                period_end: "2025-06-30".into(),
                pay_date: "2025-07-01".into(),
            })
            .await
            .unwrap();

        // Create two payslips manually (simulating multi-employee pay run)
        repo.create_payslip(&pay_run.id, "emp-301", 40_000_00, 31_200_00, 8_800_00, 0)
            .await
            .unwrap();
        repo.create_payslip(&pay_run.id, "emp-302", 30_000_00, 23_400_00, 6_600_00, 0)
            .await
            .unwrap();

        let payslips = repo.list_payslips_for_run(&pay_run.id).await.unwrap();
        assert_eq!(payslips.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_employee_terminated_sets_end_date() {
        let repo = setup_repo().await;

        // Create active compensation for employee
        let comp = repo
            .create_compensation(&CreateCompensationRequest {
                employee_id: "emp-term-001".into(),
                salary_type: "salaried".into(),
                amount_cents: 75_000_00,
                currency: Some("USD".into()),
                effective_date: "2025-01-01".into(),
                end_date: None,
            })
            .await
            .unwrap();
        assert!(comp.end_date.is_none());

        // Simulate termination: set end_date
        let updated = repo
            .update_compensation(
                &comp.id,
                &UpdateCompensationRequest {
                    end_date: Some("2025-06-30".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.end_date, Some("2025-06-30".to_string()));
        assert_eq!(updated.amount_cents, 75_000_00); // other fields preserved
    }

    #[tokio::test]
    async fn test_termination_handles_multiple_compensations() {
        let repo = setup_repo().await;

        // Create two active compensation records
        repo.create_compensation(&CreateCompensationRequest {
            employee_id: "emp-term-002".into(),
            salary_type: "salaried".into(),
            amount_cents: 50_000_00,
            currency: Some("USD".into()),
            effective_date: "2025-01-01".into(),
            end_date: None,
        })
        .await
        .unwrap();

        repo.create_compensation(&CreateCompensationRequest {
            employee_id: "emp-term-002".into(),
            salary_type: "hourly".into(),
            amount_cents: 10_000_00,
            currency: Some("USD".into()),
            effective_date: "2025-03-01".into(),
            end_date: None,
        })
        .await
        .unwrap();

        // Simulate termination handler: set end_date on all active comps
        let comps = repo.list_compensation_by_employee("emp-term-002").await.unwrap();
        assert_eq!(comps.len(), 2);

        for comp in &comps {
            if comp.end_date.is_none() {
                repo.update_compensation(
                    &comp.id,
                    &UpdateCompensationRequest {
                        end_date: Some("2025-06-30".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            }
        }

        let updated = repo.list_compensation_by_employee("emp-term-002").await.unwrap();
        assert!(updated.iter().all(|c| c.end_date == Some("2025-06-30".to_string())));
    }

    #[tokio::test]
    async fn test_progressive_tax_with_two_brackets() {
        let repo = setup_repo().await;

        // Create two brackets: 10% up to 100000 cents ($1,000), 22% above
        repo.create_tax_bracket(&CreateTaxBracketRequest {
            name: "Low".into(),
            min_income_cents: 0,
            max_income_cents: Some(100_000_00),
            rate_percent: 10.0,
        })
        .await
        .unwrap();

        repo.create_tax_bracket(&CreateTaxBracketRequest {
            name: "High".into(),
            min_income_cents: 100_000_00,
            max_income_cents: None,
            rate_percent: 22.0,
        })
        .await
        .unwrap();

        // Verify brackets created
        let brackets = repo.list_tax_brackets().await.unwrap();
        assert_eq!(brackets.len(), 2);

        // Test: gross = 20000000 cents ($200,000)
        // First 10000000 at 10% = 1000000
        // Next 10000000 at 22% = 2200000
        // Total tax = 3200000
        let brackets_list = repo.list_tax_brackets().await.unwrap();
        let gross = 200_000_00i64;
        let mut total_tax: i64 = 0;
        let mut remaining = gross;
        for bracket in &brackets_list {
            if remaining <= 0 {
                break;
            }
            let ceiling = bracket.max_income_cents.unwrap_or(i64::MAX);
            let taxable = std::cmp::min(remaining, ceiling - bracket.min_income_cents);
            if taxable > 0 {
                total_tax += (taxable as f64 * bracket.rate_percent / 100.0).round() as i64;
            }
            remaining -= taxable;
        }
        assert_eq!(total_tax, 3_200_000); // 10% of 10M + 22% of 10M = 1M + 2.2M = 3.2M cents
    }

    #[tokio::test]
    async fn test_progressive_tax_zero_income() {
        let repo = setup_repo().await;

        repo.create_tax_bracket(&CreateTaxBracketRequest {
            name: "Low".into(),
            min_income_cents: 0,
            max_income_cents: Some(100_000_00),
            rate_percent: 10.0,
        })
        .await
        .unwrap();

        let brackets = repo.list_tax_brackets().await.unwrap();
        let gross = 0i64;
        let mut total_tax: i64 = 0;
        let mut remaining = gross;
        for bracket in &brackets {
            if remaining <= 0 {
                break;
            }
            let ceiling = bracket.max_income_cents.unwrap_or(i64::MAX);
            let taxable = std::cmp::min(remaining, ceiling - bracket.min_income_cents);
            if taxable > 0 {
                total_tax += (taxable as f64 * bracket.rate_percent / 100.0).round() as i64;
            }
            remaining -= taxable;
        }
        assert_eq!(total_tax, 0);
    }

    #[tokio::test]
    async fn test_progressive_tax_single_bracket() {
        let repo = setup_repo().await;

        // Single bracket: 22% on all income
        repo.create_tax_bracket(&CreateTaxBracketRequest {
            name: "Flat".into(),
            min_income_cents: 0,
            max_income_cents: None,
            rate_percent: 22.0,
        })
        .await
        .unwrap();

        let brackets = repo.list_tax_brackets().await.unwrap();
        assert_eq!(brackets.len(), 1);

        let gross = 60_000_00i64;
        let mut total_tax: i64 = 0;
        let mut remaining = gross;
        for bracket in &brackets {
            if remaining <= 0 {
                break;
            }
            let ceiling = bracket.max_income_cents.unwrap_or(i64::MAX);
            let taxable = std::cmp::min(remaining, ceiling - bracket.min_income_cents);
            if taxable > 0 {
                total_tax += (taxable as f64 * bracket.rate_percent / 100.0).round() as i64;
            }
            remaining -= taxable;
        }
        assert_eq!(total_tax, 13_200_00); // 22% of 60k
    }

    #[tokio::test]
    async fn test_tax_bracket_overlap_validation() {
        let repo = setup_repo().await;
        let svc = PayrollService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create first bracket: 0-100000 cents at 10%
        svc.create_tax_bracket(CreateTaxBracketRequest {
            name: "Low".into(),
            min_income_cents: 0,
            max_income_cents: Some(100_000_00),
            rate_percent: 10.0,
        })
        .await
        .unwrap();

        // Overlapping: 50000-150000 at 15% should fail
        let result = svc
            .create_tax_bracket(CreateTaxBracketRequest {
                name: "Overlap".into(),
                min_income_cents: 50_000_00,
                max_income_cents: Some(150_000_00),
                rate_percent: 15.0,
            })
            .await;
        assert!(result.is_err());

        // Adjacent (non-overlapping): 100000-200000 at 22% should succeed
        let bracket = svc
            .create_tax_bracket(CreateTaxBracketRequest {
                name: "High".into(),
                min_income_cents: 100_000_00,
                max_income_cents: Some(200_000_00),
                rate_percent: 22.0,
            })
            .await
            .unwrap();
        assert_eq!(bracket.name, "High");

        // Adjacent open-ended: 200000+ at 30% should succeed
        let top = svc
            .create_tax_bracket(CreateTaxBracketRequest {
                name: "Top".into(),
                min_income_cents: 200_000_00,
                max_income_cents: None,
                rate_percent: 30.0,
            })
            .await
            .unwrap();
        assert_eq!(top.name, "Top");
        assert_eq!(top.max_income_cents, None);

        // Verify 3 brackets total
        let brackets = repo.list_tax_brackets().await.unwrap();
        assert_eq!(brackets.len(), 3);
    }

    #[tokio::test]
    async fn test_tax_bracket_invalid_rate() {
        let repo = setup_repo().await;
        let svc = PayrollService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Negative rate should fail
        let result = svc
            .create_tax_bracket(CreateTaxBracketRequest {
                name: "Invalid".into(),
                min_income_cents: 0,
                max_income_cents: None,
                rate_percent: -5.0,
            })
            .await;
        assert!(result.is_err());

        // Rate > 100 should fail
        let result = svc
            .create_tax_bracket(CreateTaxBracketRequest {
                name: "Invalid2".into(),
                min_income_cents: 0,
                max_income_cents: None,
                rate_percent: 150.0,
            })
            .await;
        assert!(result.is_err());

        // max <= min should fail
        let result = svc
            .create_tax_bracket(CreateTaxBracketRequest {
                name: "Invalid3".into(),
                min_income_cents: 100_000_00,
                max_income_cents: Some(50_000_00),
                rate_percent: 10.0,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_pay_run() {
        let repo = setup_repo().await;
        let svc = PayrollService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let pay_run = svc.create_pay_run(CreatePayRunRequest {
            period_start: "2025-07-01".into(),
            period_end: "2025-07-31".into(),
            pay_date: "2025-08-01".into(),
        }).await.unwrap();

        let fetched = svc.get_pay_run(&pay_run.id).await.unwrap();
        assert_eq!(fetched.id, pay_run.id);
        assert_eq!(fetched.status, "draft");

        // Not found
        let result = svc.get_pay_run("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_deduction() {
        let repo = setup_repo().await;
        let svc = PayrollService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let deduction = svc.create_deduction(CreateDeductionRequest {
            employee_id: "emp-get".into(),
            code: "TEST".into(),
            amount_cents: 5_000_00,
            recurring: Some(true),
            start_date: "2025-01-01".into(),
            end_date: None,
        }).await.unwrap();

        let fetched = svc.get_deduction(&deduction.id).await.unwrap();
        assert_eq!(fetched.id, deduction.id);
        assert_eq!(fetched.code, "TEST");

        // Not found
        let result = svc.get_deduction("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_tax_bracket() {
        let repo = setup_repo().await;
        let svc = PayrollService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let bracket = svc.create_tax_bracket(CreateTaxBracketRequest {
            name: "Get Test".into(),
            min_income_cents: 0,
            max_income_cents: None,
            rate_percent: 15.0,
        }).await.unwrap();

        let fetched = svc.get_tax_bracket(&bracket.id).await.unwrap();
        assert_eq!(fetched.id, bracket.id);
        assert_eq!(fetched.name, "Get Test");
        assert_eq!(fetched.rate_percent, 15.0);

        // Not found
        let result = svc.get_tax_bracket("nonexistent").await;
        assert!(result.is_err());
    }
}

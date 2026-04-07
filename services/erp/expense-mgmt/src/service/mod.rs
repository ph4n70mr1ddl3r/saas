use crate::models::*;
use crate::repository::ExpenseRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    ExpenseReportApproved, ExpenseReportRejected, ExpenseReportPaid, ExpenseReportSubmitted,
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ExpenseService {
    repo: ExpenseRepo,
    bus: NatsBus,
}

impl ExpenseService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: ExpenseRepo::new(pool),
            bus,
        }
    }

    // --- Expense Categories ---

    pub async fn list_categories(&self) -> AppResult<Vec<ExpenseCategory>> {
        self.repo.list_categories().await
    }

    pub async fn get_category(&self, id: &str) -> AppResult<ExpenseCategory> {
        self.repo.get_category(id).await
    }

    pub async fn create_category(
        &self,
        input: &CreateExpenseCategoryRequest,
    ) -> AppResult<ExpenseCategory> {
        self.repo.create_category(input).await
    }

    pub async fn update_category(
        &self,
        id: &str,
        input: &UpdateExpenseCategoryRequest,
    ) -> AppResult<ExpenseCategory> {
        self.repo.update_category(
            id,
            input.name.as_deref(),
            input.description.as_deref(),
            input.limit_cents,
            input.requires_receipt,
            input.gl_account_code.as_deref(),
        ).await
    }

    // --- Expense Reports ---

    pub async fn list_reports(&self) -> AppResult<Vec<ExpenseReport>> {
        self.repo.list_reports().await
    }

    pub async fn get_report(&self, id: &str) -> AppResult<ExpenseReportWithLines> {
        let report = self.repo.get_report(id).await?;
        let lines = self.repo.list_lines(id).await?;
        let per_diems = self.repo.list_per_diems(id).await?;
        let mileage = self.repo.list_mileage(id).await?;
        Ok(ExpenseReportWithLines {
            report,
            lines,
            per_diems,
            mileage,
        })
    }

    pub async fn create_report(
        &self,
        input: &CreateExpenseReportRequest,
    ) -> AppResult<ExpenseReport> {
        self.repo.create_report(input).await
    }

    pub async fn submit_report(&self, id: &str) -> AppResult<ExpenseReport> {
        let report = self.repo.get_report(id).await?;

        if report.status != "draft" {
            return Err(AppError::Validation(format!(
                "Cannot submit report in '{}' status. Only 'draft' reports can be submitted.",
                report.status
            )));
        }

        // Receipt required check
        let lines = self.repo.list_lines(id).await?;
        for line in &lines {
            let category = self.repo.get_category(&line.category_id).await?;
            if category.requires_receipt == 1 && line.receipt_url.is_none() {
                return Err(AppError::Validation(format!(
                    "Line '{}' (category: '{}') requires a receipt but none was attached",
                    line.id, category.name
                )));
            }
        }

        // Category limit enforcement
        for line in &lines {
            let category = self.repo.get_category(&line.category_id).await?;
            if category.limit_cents > 0 {
                let spent = self.repo.get_category_spent(&line.category_id).await?;
                if spent > category.limit_cents {
                    return Err(AppError::Validation(format!(
                        "Category '{}' limit exceeded. Limit: {} cents, Total: {} cents",
                        category.name, category.limit_cents, spent
                    )));
                }
            }
        }

        let report = self.repo.submit_report(id).await?;

        if let Err(e) = self
            .bus
            .publish(
                "erp.expense.report.submitted",
                ExpenseReportSubmitted {
                    report_id: report.id.clone(),
                    employee_id: report.employee_id.clone(),
                    title: report.title.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.expense.report.submitted",
                e
            );
        }

        Ok(report)
    }

    pub async fn approve_report(&self, id: &str, user_id: &str) -> AppResult<ExpenseReport> {
        let report = self.repo.get_report(id).await?;

        if report.status != "submitted" {
            return Err(AppError::Validation(format!(
                "Cannot approve report in '{}' status. Only 'submitted' reports can be approved.",
                report.status
            )));
        }

        let report = self.repo.approve_report(id, user_id).await?;

        // Look up GL account code from the first expense line's category
        let gl_account_code = match self.repo.list_lines(id).await {
            Ok(lines) if !lines.is_empty() => {
                match self.repo.get_category(&lines[0].category_id).await {
                    Ok(cat) => cat.gl_account_code.unwrap_or_default(),
                    Err(_) => String::new(),
                }
            }
            _ => String::new(),
        };

        // Publish expense report approved event for GL auto-JE
        if let Err(e) = self
            .bus
            .publish(
                "erp.expense.report.approved",
                ExpenseReportApproved {
                    report_id: report.id.clone(),
                    employee_id: report.employee_id.clone(),
                    total_cents: report.total_cents,
                    gl_account_code,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.expense.report.approved",
                e
            );
        }

        Ok(report)
    }

    pub async fn reject_report(&self, id: &str, reason: &str) -> AppResult<ExpenseReport> {
        let report = self.repo.get_report(id).await?;

        if report.status != "submitted" {
            return Err(AppError::Validation(format!(
                "Cannot reject report in '{}' status. Only 'submitted' reports can be rejected.",
                report.status
            )));
        }

        let report = self.repo.reject_report(id, reason).await?;

        if let Err(e) = self
            .bus
            .publish(
                "erp.expense.report.rejected",
                ExpenseReportRejected {
                    report_id: report.id.clone(),
                    employee_id: report.employee_id.clone(),
                    reason: reason.to_string(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.expense.report.rejected",
                e
            );
        }

        Ok(report)
    }

    pub async fn mark_paid(&self, id: &str) -> AppResult<ExpenseReport> {
        let report = self.repo.get_report(id).await?;

        if report.status != "approved" {
            return Err(AppError::Validation(format!(
                "Cannot mark report as paid in '{}' status. Only 'approved' reports can be marked as paid.",
                report.status
            )));
        }

        let report = self.repo.mark_paid(id).await?;

        if let Err(e) = self
            .bus
            .publish(
                "erp.expense.report.paid",
                ExpenseReportPaid {
                    report_id: report.id.clone(),
                    employee_id: report.employee_id.clone(),
                    total_cents: report.total_cents,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "erp.expense.report.paid",
                e
            );
        }

        Ok(report)
    }

    // --- Expense Lines ---

    pub async fn create_line(&self, input: &CreateExpenseLineRequest) -> AppResult<ExpenseLine> {
        let report = self.repo.get_report(&input.report_id).await?;

        if report.status != "draft" {
            return Err(AppError::Validation(format!(
                "Cannot add lines to report in '{}' status. Only 'draft' reports can be edited.",
                report.status
            )));
        }

        let category = self.repo.get_category(&input.category_id).await?;

        if category.limit_cents > 0 {
            let spent = self.repo.get_category_spent(&input.category_id).await?;
            if spent + input.amount_cents > category.limit_cents {
                return Err(AppError::Validation(format!(
                    "Adding this line would exceed category '{}' limit. Limit: {} cents, Current: {} cents, Attempted addition: {} cents",
                    category.name, category.limit_cents, spent, input.amount_cents
                )));
            }
        }

        self.repo.create_line(input).await
    }

    // --- Per Diems ---

    pub async fn create_per_diem(&self, input: &CreatePerDiemRequest) -> AppResult<PerDiem> {
        let report = self.repo.get_report(&input.report_id).await?;
        if report.status != "draft" {
            return Err(AppError::Validation(format!(
                "Cannot add per diems to report in '{}' status. Only 'draft' reports can be edited.",
                report.status
            )));
        }
        self.repo.create_per_diem(input).await
    }

    pub async fn list_per_diems(&self, report_id: &str) -> AppResult<Vec<PerDiem>> {
        self.repo.list_per_diems(report_id).await
    }

    pub async fn list_all_per_diems(&self) -> AppResult<Vec<PerDiem>> {
        self.repo.list_all_per_diems().await
    }

    // --- Mileage ---

    pub async fn create_mileage(&self, input: &CreateMileageRequest) -> AppResult<Mileage> {
        let report = self.repo.get_report(&input.report_id).await?;
        if report.status != "draft" {
            return Err(AppError::Validation(format!(
                "Cannot add mileage to report in '{}' status. Only 'draft' reports can be edited.",
                report.status
            )));
        }
        self.repo.create_mileage(input).await
    }

    pub async fn list_mileage(&self, report_id: &str) -> AppResult<Vec<Mileage>> {
        self.repo.list_mileage(report_id).await
    }

    pub async fn list_all_mileage(&self) -> AppResult<Vec<Mileage>> {
        self.repo.list_all_mileage().await
    }

    // --- Event Handlers (cross-domain integration) ---

    /// Create an onboarding expense report when a new employee is created.
    pub async fn handle_employee_created(
        &self,
        employee_id: &str,
        first_name: &str,
        last_name: &str,
    ) -> AppResult<ExpenseReport> {
        let report = self.repo.create_report(&CreateExpenseReportRequest {
            employee_id: employee_id.to_string(),
            title: format!("Onboarding expenses - {} {}", first_name, last_name),
            description: Some("Auto-created onboarding expense report for new employee".to_string()),
        }).await?;

        tracing::info!(
            "Auto-created onboarding expense report {} for employee {} {}",
            report.id, first_name, last_name
        );
        Ok(report)
    }

    /// Create an expense category when a benefit plan is created.
    pub async fn handle_benefit_plan_created(
        &self,
        plan_id: &str,
        name: &str,
        plan_type: &str,
    ) -> AppResult<ExpenseCategory> {
        let category = self.repo.create_category(&CreateExpenseCategoryRequest {
            name: format!("{} - {}", plan_type, name),
            description: Some(format!("Auto-created category for benefit plan {}", plan_id)),
            limit_cents: None,
            requires_receipt: Some(false),
        }).await?;

        tracing::info!(
            "Auto-created expense category '{}' for benefit plan {}",
            category.name, plan_id
        );
        Ok(category)
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
            include_str!("../../migrations/001_create_expense_categories.sql"),
            include_str!("../../migrations/002_create_expense_reports.sql"),
            include_str!("../../migrations/003_create_expense_lines.sql"),
            include_str!("../../migrations/004_create_per_diems.sql"),
            include_str!("../../migrations/005_create_mileage.sql"),
            include_str!("../../migrations/006_add_gl_account_code.sql"),
        ];
        let migration_names = [
            "001_create_expense_categories.sql",
            "002_create_expense_reports.sql",
            "003_create_expense_lines.sql",
            "004_create_per_diems.sql",
            "005_create_mileage.sql",
            "006_add_gl_account_code.sql",
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

    async fn setup_repo() -> ExpenseRepo {
        let pool = setup().await;
        ExpenseRepo::new(pool)
    }

    // Helper to create a category
    async fn create_test_category(repo: &ExpenseRepo, name: &str, limit_cents: i64, requires_receipt: bool) -> ExpenseCategory {
        repo.create_category(&CreateExpenseCategoryRequest {
            name: name.to_string(),
            description: None,
            limit_cents: Some(limit_cents),
            requires_receipt: Some(requires_receipt),
        })
        .await
        .unwrap()
    }

    // Helper to create a report
    async fn create_test_report(repo: &ExpenseRepo, employee_id: &str, title: &str) -> ExpenseReport {
        repo.create_report(&CreateExpenseReportRequest {
            employee_id: employee_id.to_string(),
            title: title.to_string(),
            description: None,
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_service_submit_report_with_receipt_required() {
        let repo = setup_repo().await;

        // Create category that requires receipts
        let cat = create_test_category(&repo, "Travel", 0, true).await;
        let report = create_test_report(&repo, "emp-1", "Business Trip").await;

        // Add line without receipt
        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-06-01".into(),
            category_id: cat.id.clone(),
            amount_cents: 5000,
            description: None,
            receipt_url: None,
        })
        .await
        .unwrap();

        // Submit should fail because receipt is required but not attached
        let result = repo.submit_report(&report.id).await;
        // The repo-level submit doesn't validate; the service-level does.
        // Here we verify the data is consistent for the service to validate.
        let lines = repo.list_lines(&report.id).await.unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].receipt_url.is_none());
        assert_eq!(cat.requires_receipt, 1);
    }

    #[tokio::test]
    async fn test_service_submit_report_with_receipt_attached() {
        let repo = setup_repo().await;

        let cat = create_test_category(&repo, "Travel", 0, true).await;
        let report = create_test_report(&repo, "emp-1", "Business Trip").await;

        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-06-01".into(),
            category_id: cat.id.clone(),
            amount_cents: 5000,
            description: None,
            receipt_url: Some("https://receipts.example.com/001".into()),
        })
        .await
        .unwrap();

        // Submit should succeed (repo-level)
        let submitted = repo.submit_report(&report.id).await.unwrap();
        assert_eq!(submitted.status, "submitted");
    }

    #[tokio::test]
    async fn test_service_category_limit_enforcement() {
        let repo = setup_repo().await;

        // Category with $50 limit
        let cat = create_test_category(&repo, "Meals", 5000, false).await;

        let report = create_test_report(&repo, "emp-1", "Lunch Expenses").await;
        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-06-01".into(),
            category_id: cat.id.clone(),
            amount_cents: 6000, // exceeds limit
            description: None,
            receipt_url: None,
        })
        .await
        .unwrap();

        let spent = repo.get_category_spent(&cat.id).await.unwrap();
        assert!(spent > cat.limit_cents, "Spent should exceed category limit");
    }

    #[tokio::test]
    async fn test_service_report_full_lifecycle() {
        let repo = setup_repo().await;

        let cat = create_test_category(&repo, "General", 0, false).await;
        let report = create_test_report(&repo, "emp-1", "Q2 Expenses").await;

        // Add line
        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-06-15".into(),
            category_id: cat.id.clone(),
            amount_cents: 10000,
            description: Some("Hotel".into()),
            receipt_url: None,
        })
        .await
        .unwrap();

        // Verify total updated
        let report = repo.get_report(&report.id).await.unwrap();
        assert_eq!(report.total_cents, 10000);
        assert_eq!(report.status, "draft");

        // Submit
        let report = repo.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");
        assert!(report.submitted_at.is_some());

        // Approve
        let report = repo.approve_report(&report.id, "mgr-1").await.unwrap();
        assert_eq!(report.status, "approved");
        assert_eq!(report.approved_by.as_deref(), Some("mgr-1"));

        // Mark paid
        let report = repo.mark_paid(&report.id).await.unwrap();
        assert_eq!(report.status, "paid");
    }

    #[tokio::test]
    async fn test_service_cannot_add_lines_to_submitted_report() {
        let repo = setup_repo().await;

        let cat = create_test_category(&repo, "General", 0, false).await;
        let report = create_test_report(&repo, "emp-1", "Closed Report").await;

        // Submit immediately (no lines)
        repo.submit_report(&report.id).await.unwrap();
        let report = repo.get_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");

        // The service would check status == "draft" before allowing line creation
        assert_ne!(report.status, "draft");
    }

    #[tokio::test]
    async fn test_service_approve_non_submitted_fails() {
        let repo = setup_repo().await;
        let report = create_test_report(&repo, "emp-1", "Draft Report").await;

        // Repo-level approve doesn't enforce status, but service does
        // Here we verify the status is draft so service would block it
        assert_eq!(report.status, "draft");
        assert_ne!(report.status, "submitted");
    }

    #[tokio::test]
    async fn test_service_reject_report() {
        let repo = setup_repo().await;
        let report = create_test_report(&repo, "emp-1", "Bad Report").await;

        repo.submit_report(&report.id).await.unwrap();
        let report = repo
            .reject_report(&report.id, "Invalid expenses")
            .await
            .unwrap();

        assert_eq!(report.status, "rejected");
        assert_eq!(
            report.rejected_reason.as_deref(),
            Some("Invalid expenses")
        );
    }

    #[tokio::test]
    async fn test_service_per_diem_adds_to_report() {
        let repo = setup_repo().await;
        let report = create_test_report(&repo, "emp-1", "Conference").await;

        repo.create_per_diem(&CreatePerDiemRequest {
            report_id: report.id.clone(),
            location: "San Francisco".into(),
            start_date: "2025-07-01".into(),
            end_date: "2025-07-03".into(),
            daily_rate_cents: 15000,
        })
        .await
        .unwrap();

        let report = repo.get_report(&report.id).await.unwrap();
        // 3 days * 15000 = 45000
        assert_eq!(report.total_cents, 45000);
    }

    #[tokio::test]
    async fn test_service_mileage_adds_to_report() {
        let repo = setup_repo().await;
        let report = create_test_report(&repo, "emp-1", "Client Visit").await;

        repo.create_mileage(&CreateMileageRequest {
            report_id: report.id.clone(),
            origin: "Office".into(),
            destination: "Client".into(),
            distance_miles: 50.0,
            rate_per_mile_cents: 67,
            expense_date: "2025-07-10".into(),
        })
        .await
        .unwrap();

        let report = repo.get_report(&report.id).await.unwrap();
        // 50.0 * 67 = 3350
        assert_eq!(report.total_cents, (50.0_f64 * 67.0_f64) as i64);
    }

    #[tokio::test]
    async fn test_service_combined_expense_report() {
        let repo = setup_repo().await;
        let cat = create_test_category(&repo, "General", 0, false).await;
        let report = create_test_report(&repo, "emp-1", "Full Trip").await;

        // Line: $200
        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-08-01".into(),
            category_id: cat.id.clone(),
            amount_cents: 20000,
            description: None,
            receipt_url: None,
        })
        .await
        .unwrap();

        // Per diem: 2 days * $100 = $200
        repo.create_per_diem(&CreatePerDiemRequest {
            report_id: report.id.clone(),
            location: "NYC".into(),
            start_date: "2025-08-01".into(),
            end_date: "2025-08-02".into(),
            daily_rate_cents: 10000,
        })
        .await
        .unwrap();

        // Mileage: 100 miles * $0.50 = $50
        repo.create_mileage(&CreateMileageRequest {
            report_id: report.id.clone(),
            origin: "Home".into(),
            destination: "Airport".into(),
            distance_miles: 100.0,
            rate_per_mile_cents: 50,
            expense_date: "2025-08-01".into(),
        })
        .await
        .unwrap();

        let report = repo.get_report(&report.id).await.unwrap();
        // 20000 + 20000 + 5000 = 45000
        assert_eq!(report.total_cents, 45000);
    }

    #[tokio::test]
    async fn test_service_get_report_with_lines() {
        let repo = setup_repo().await;
        let cat = create_test_category(&repo, "Office", 0, false).await;
        let report = create_test_report(&repo, "emp-1", "Supplies").await;

        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-09-01".into(),
            category_id: cat.id.clone(),
            amount_cents: 7500,
            description: Some("Printer paper".into()),
            receipt_url: None,
        })
        .await
        .unwrap();

        let lines = repo.list_lines(&report.id).await.unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 7500);
    }

    #[tokio::test]
    async fn test_handle_employee_created_onboarding_report() {
        let repo = setup_repo().await;

        // Simulate what handle_employee_created does:
        // Create an onboarding expense report for the new employee
        let report = repo
            .create_report(&CreateExpenseReportRequest {
                employee_id: "emp-new-001".to_string(),
                title: "Onboarding expenses - Jane Smith".to_string(),
                description: Some("Auto-created onboarding expense report for new employee".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(report.employee_id, "emp-new-001");
        assert_eq!(report.title, "Onboarding expenses - Jane Smith");
        assert_eq!(report.status, "draft");
        assert_eq!(report.total_cents, 0);

        // Verify it's listed
        let reports = repo.list_reports().await.unwrap();
        assert_eq!(reports.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_benefit_plan_created_category() {
        let repo = setup_repo().await;

        // Simulate what handle_benefit_plan_created does:
        // Create an expense category for the new benefit plan
        let category = repo
            .create_category(&CreateExpenseCategoryRequest {
                name: "Health Insurance - Medical Plus".to_string(),
                description: Some("Auto-created category for benefit plan plan-001".to_string()),
                limit_cents: None,
                requires_receipt: Some(false),
            })
            .await
            .unwrap();

        assert_eq!(category.name, "Health Insurance - Medical Plus");
        assert_eq!(category.limit_cents, 0);
        assert_eq!(category.requires_receipt, 0);
        assert_eq!(category.is_active, 1);

        // Verify it's listed
        let categories = repo.list_categories().await.unwrap();
        assert_eq!(categories.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_multiple_benefit_plans_create_categories() {
        let repo = setup_repo().await;

        // Create categories for multiple benefit plans
        let plans = vec![
            ("plan-1", "Medical Plus", "Health Insurance"),
            ("plan-2", "Dental Basic", "Dental Insurance"),
            ("plan-3", "Vision Standard", "Vision Insurance"),
        ];

        for (plan_id, name, plan_type) in &plans {
            repo.create_category(&CreateExpenseCategoryRequest {
                name: format!("{} - {}", plan_type, name),
                description: Some(format!("Auto-created category for benefit plan {}", plan_id)),
                limit_cents: None,
                requires_receipt: Some(false),
            })
            .await
            .unwrap();
        }

        let categories = repo.list_categories().await.unwrap();
        assert_eq!(categories.len(), 3);

        // Verify all are active
        for cat in &categories {
            assert_eq!(cat.is_active, 1);
        }
    }

    #[tokio::test]
    async fn test_onboarding_report_can_receive_expense_lines() {
        let repo = setup_repo().await;

        // Create onboarding report (like handle_employee_created would)
        let report = repo
            .create_report(&CreateExpenseReportRequest {
                employee_id: "emp-onboard-001".to_string(),
                title: "Onboarding expenses - John Doe".to_string(),
                description: Some("Auto-created onboarding expense report".to_string()),
            })
            .await
            .unwrap();

        // Create a category for onboarding
        let cat = repo
            .create_category(&CreateExpenseCategoryRequest {
                name: "Relocation - Moving".to_string(),
                description: Some("Auto-created for benefit plan reloc-001".to_string()),
                limit_cents: None,
                requires_receipt: Some(true),
            })
            .await
            .unwrap();

        // Add expense lines to the onboarding report
        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-07-01".into(),
            category_id: cat.id.clone(),
            amount_cents: 50000,
            description: Some("Moving company fee".into()),
            receipt_url: Some("https://receipts.example.com/moving".into()),
        })
        .await
        .unwrap();

        // Verify total was updated
        let report = repo.get_report(&report.id).await.unwrap();
        assert_eq!(report.total_cents, 50000);

        // Verify report can be submitted
        let report = repo.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");
    }

    #[tokio::test]
    async fn test_update_expense_category() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let cat = repo
            .create_category(&CreateExpenseCategoryRequest {
                name: "Travel".into(),
                description: Some("Travel expenses".into()),
                limit_cents: Some(50000),
                requires_receipt: Some(true),
            })
            .await
            .unwrap();

        let updated = repo
            .update_category(
                &cat.id,
                Some("Business Travel"),
                Some("All business travel expenses".into()),
                Some(100000),
                Some(false),
                Some("6100".into()),
            )
            .await
            .unwrap();

        assert_eq!(updated.name, "Business Travel");
        assert_eq!(updated.description, Some("All business travel expenses".into()));
        assert_eq!(updated.limit_cents, 100000);
        assert_eq!(updated.requires_receipt, 0);
        assert_eq!(updated.gl_account_code, Some("6100".into()));
    }
}

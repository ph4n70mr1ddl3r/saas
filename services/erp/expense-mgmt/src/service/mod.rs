use crate::models::*;
use crate::repository::ExpenseRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ExpenseService {
    repo: ExpenseRepo,
    #[allow(dead_code)]
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

        // Receipt required check: if any line's category requires_receipt and line.receipt_url is None, reject submit
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
                // spent already includes this line (since it's in a draft/submitted report)
                // We just need to check total doesn't exceed limit
                if spent > category.limit_cents {
                    return Err(AppError::Validation(format!(
                        "Category '{}' limit exceeded. Limit: {} cents, Total: {} cents",
                        category.name, category.limit_cents, spent
                    )));
                }
            }
        }

        self.repo.submit_report(id).await
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

        // Publish event (log for now since proto events will be added separately)
        tracing::info!(
            report_id = %report.id,
            approved_by = %user_id,
            total_cents = report.total_cents,
            "Expense report approved"
        );

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

        self.repo.reject_report(id, reason).await
    }

    pub async fn mark_paid(&self, id: &str) -> AppResult<ExpenseReport> {
        let report = self.repo.get_report(id).await?;

        if report.status != "approved" {
            return Err(AppError::Validation(format!(
                "Cannot mark report as paid in '{}' status. Only 'approved' reports can be marked as paid.",
                report.status
            )));
        }

        self.repo.mark_paid(id).await
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

        // Validate category exists
        self.repo.get_category(&input.category_id).await?;

        // Category limit enforcement
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

    pub async fn list_all_per_diems(&self) -> AppResult<Vec<PerDiem>> {
        self.repo.list_all_per_diems().await
    }

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

    // --- Mileage ---

    pub async fn list_all_mileage(&self) -> AppResult<Vec<Mileage>> {
        self.repo.list_all_mileage().await
    }

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
        ];
        let migration_names = [
            "001_create_expense_categories.sql",
            "002_create_expense_reports.sql",
            "003_create_expense_lines.sql",
            "004_create_per_diems.sql",
            "005_create_mileage.sql",
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

    fn make_category_request(
        name: &str,
        limit_cents: i64,
        requires_receipt: bool,
    ) -> CreateExpenseCategoryRequest {
        CreateExpenseCategoryRequest {
            name: name.to_string(),
            description: None,
            limit_cents: Some(limit_cents),
            requires_receipt: Some(requires_receipt),
        }
    }

    fn make_report_request(employee_id: &str, title: &str) -> CreateExpenseReportRequest {
        CreateExpenseReportRequest {
            employee_id: employee_id.to_string(),
            title: title.to_string(),
            description: None,
        }
    }

    #[tokio::test]
    async fn test_category_crud_via_service() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let cat = svc
            .create_category(&make_category_request("Meals", 10000, false))
            .await
            .unwrap();
        assert_eq!(cat.name, "Meals");

        let fetched = svc.get_category(&cat.id).await.unwrap();
        assert_eq!(fetched.id, cat.id);

        let list = svc.list_categories().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_report_lifecycle_draft_to_paid() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Biz Trip"))
            .await
            .unwrap();
        assert_eq!(report.status, "draft");

        // Submit
        let report = svc.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");

        // Approve
        let report = svc.approve_report(&report.id, "mgr-1").await.unwrap();
        assert_eq!(report.status, "approved");
        assert_eq!(report.approved_by.as_deref(), Some("mgr-1"));

        // Mark paid
        let report = svc.mark_paid(&report.id).await.unwrap();
        assert_eq!(report.status, "paid");
    }

    #[tokio::test]
    async fn test_report_draft_to_rejected() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Bad Trip"))
            .await
            .unwrap();

        // Submit
        let report = svc.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");

        // Reject
        let report = svc.reject_report(&report.id, "Over budget").await.unwrap();
        assert_eq!(report.status, "rejected");
        assert_eq!(report.rejected_reason.as_deref(), Some("Over budget"));
    }

    #[tokio::test]
    async fn test_cannot_add_lines_to_submitted_report() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let cat = svc
            .create_category(&make_category_request("Travel", 100000, false))
            .await
            .unwrap();
        let report = svc
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();

        // Submit the report first
        svc.submit_report(&report.id).await.unwrap();

        // Try to add a line to the submitted report
        let result = svc
            .create_line(&CreateExpenseLineRequest {
                report_id: report.id.clone(),
                expense_date: "2025-04-01".to_string(),
                category_id: cat.id.clone(),
                amount_cents: 5000,
                description: None,
                receipt_url: None,
            })
            .await;

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Only 'draft' reports can be edited"));
    }

    #[tokio::test]
    async fn test_receipt_required_validation() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        // Create a category that requires receipts
        let cat = svc
            .create_category(&make_category_request("Airfare", 50000, true))
            .await
            .unwrap();
        let report = svc
            .create_report(&make_report_request("emp-1", "Flight Trip"))
            .await
            .unwrap();

        // Add a line without a receipt
        svc.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-04-01".to_string(),
            category_id: cat.id.clone(),
            amount_cents: 30000,
            description: Some("Flight to NYC".to_string()),
            receipt_url: None, // No receipt
        })
        .await
        .unwrap();

        // Try to submit - should fail because receipt is required
        let result = svc.submit_report(&report.id).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("requires a receipt"));
    }

    #[tokio::test]
    async fn test_receipt_required_with_receipt_provided() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let cat = svc
            .create_category(&make_category_request("Airfare", 50000, true))
            .await
            .unwrap();
        let report = svc
            .create_report(&make_report_request("emp-1", "Flight Trip"))
            .await
            .unwrap();

        // Add a line WITH a receipt
        svc.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-04-01".to_string(),
            category_id: cat.id.clone(),
            amount_cents: 30000,
            description: Some("Flight to NYC".to_string()),
            receipt_url: Some("https://receipts.example.com/123".to_string()),
        })
        .await
        .unwrap();

        // Submit should succeed
        let report = svc.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");
    }

    #[tokio::test]
    async fn test_category_limit_enforcement() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        // Create a category with a 10000 cent limit
        let cat = svc
            .create_category(&make_category_request("Meals", 10000, false))
            .await
            .unwrap();
        let report = svc
            .create_report(&make_report_request("emp-1", "Dinner"))
            .await
            .unwrap();

        // Add a line for 8000 cents - should succeed
        svc.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-04-01".to_string(),
            category_id: cat.id.clone(),
            amount_cents: 8000,
            description: None,
            receipt_url: None,
        })
        .await
        .unwrap();

        // Add another line for 5000 cents - should fail (8000 + 5000 > 10000)
        let result = svc
            .create_line(&CreateExpenseLineRequest {
                report_id: report.id.clone(),
                expense_date: "2025-04-02".to_string(),
                category_id: cat.id.clone(),
                amount_cents: 5000,
                description: None,
                receipt_url: None,
            })
            .await;

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("limit"));
    }

    #[tokio::test]
    async fn test_cannot_submit_twice() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();
        svc.submit_report(&report.id).await.unwrap();

        // Try to submit again
        let result = svc.submit_report(&report.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cannot_approve_draft() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();

        let result = svc.approve_report(&report.id, "mgr-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cannot_mark_paid_without_approval() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();

        let result = svc.mark_paid(&report.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_per_diem_via_service() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Conference"))
            .await
            .unwrap();

        let pd = svc
            .create_per_diem(&CreatePerDiemRequest {
                report_id: report.id.clone(),
                location: "San Francisco".to_string(),
                start_date: "2025-05-01".to_string(),
                end_date: "2025-05-03".to_string(),
                daily_rate_cents: 15000,
            })
            .await
            .unwrap();

        // 3 days * 15000 = 45000
        assert_eq!(pd.total_cents, 45000);

        let report_with_lines = svc.get_report(&report.id).await.unwrap();
        assert_eq!(report_with_lines.report.total_cents, 45000);
    }

    #[tokio::test]
    async fn test_mileage_via_service() {
        let pool = setup().await;
        let bus = NatsBus::connect("nats://localhost:4222", "test")
            .await
            .expect("NATS not available for test");
        let svc = ExpenseService::new(pool, bus);

        let report = svc
            .create_report(&make_report_request("emp-1", "Client Visit"))
            .await
            .unwrap();

        let m = svc
            .create_mileage(&CreateMileageRequest {
                report_id: report.id.clone(),
                origin: "Office".to_string(),
                destination: "Client".to_string(),
                distance_miles: 200.0,
                rate_per_mile_cents: 67,
                expense_date: "2025-05-10".to_string(),
            })
            .await
            .unwrap();

        // 200.0 * 67 = 13400
        assert_eq!(m.total_cents, 13400);
    }
}

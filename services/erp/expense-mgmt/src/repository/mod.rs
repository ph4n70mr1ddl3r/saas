use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ExpenseRepo {
    pool: SqlitePool,
}

impl ExpenseRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Expense Categories ---

    pub async fn list_categories(&self) -> AppResult<Vec<ExpenseCategory>> {
        let rows = sqlx::query_as::<_, ExpenseCategory>(
            "SELECT id, name, description, limit_cents, requires_receipt, is_active, gl_account_code, created_at FROM expense_categories ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_category(&self, id: &str) -> AppResult<ExpenseCategory> {
        sqlx::query_as::<_, ExpenseCategory>(
            "SELECT id, name, description, limit_cents, requires_receipt, is_active, gl_account_code, created_at FROM expense_categories WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Expense category '{}' not found", id)))
    }

    pub async fn create_category(
        &self,
        input: &CreateExpenseCategoryRequest,
    ) -> AppResult<ExpenseCategory> {
        let id = uuid::Uuid::new_v4().to_string();
        let limit_cents = input.limit_cents.unwrap_or(0);
        let requires_receipt = if input.requires_receipt.unwrap_or(false) {
            1
        } else {
            0
        };

        sqlx::query(
            "INSERT INTO expense_categories (id, name, description, limit_cents, requires_receipt) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(limit_cents)
        .bind(requires_receipt)
        .execute(&self.pool)
        .await?;
        self.get_category(&id).await
    }

    pub async fn update_category(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        limit_cents: Option<i64>,
        requires_receipt: Option<bool>,
        gl_account_code: Option<&str>,
    ) -> AppResult<ExpenseCategory> {
        let current = self.get_category(id).await?;
        let name = name.unwrap_or(&current.name);
        let limit = limit_cents.unwrap_or(current.limit_cents);
        let req_receipt = requires_receipt
            .map(|b| if b { 1 } else { 0 })
            .unwrap_or(current.requires_receipt);
        let gl_code = gl_account_code
            .map(|s| if s.is_empty() { None } else { Some(s.to_string()) })
            .unwrap_or(current.gl_account_code.clone());

        sqlx::query(
            "UPDATE expense_categories SET name = ?, description = ?, limit_cents = ?, requires_receipt = ?, gl_account_code = ? WHERE id = ?",
        )
        .bind(name)
        .bind(description.or(current.description.as_deref()))
        .bind(limit)
        .bind(req_receipt)
        .bind(&gl_code)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_category(id).await
    }

    pub async fn get_category_spent(&self, category_id: &str) -> AppResult<i64> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(el.amount_cents), 0) FROM expense_lines el INNER JOIN expense_reports er ON el.report_id = er.id WHERE el.category_id = ? AND er.status IN ('draft','submitted','approved')",
        )
        .bind(category_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(total)
    }

    // --- Expense Reports ---

    pub async fn list_reports(&self) -> AppResult<Vec<ExpenseReport>> {
        let rows = sqlx::query_as::<_, ExpenseReport>(
            "SELECT id, employee_id, title, description, total_cents, status, submitted_at, approved_by, approved_at, rejected_reason, created_at FROM expense_reports ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_report(&self, id: &str) -> AppResult<ExpenseReport> {
        sqlx::query_as::<_, ExpenseReport>(
            "SELECT id, employee_id, title, description, total_cents, status, submitted_at, approved_by, approved_at, rejected_reason, created_at FROM expense_reports WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Expense report '{}' not found", id)))
    }

    pub async fn create_report(
        &self,
        input: &CreateExpenseReportRequest,
    ) -> AppResult<ExpenseReport> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO expense_reports (id, employee_id, title, description) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.title)
        .bind(&input.description)
        .execute(&self.pool)
        .await?;
        self.get_report(&id).await
    }

    pub async fn update_report_status(&self, id: &str, status: &str) -> AppResult<ExpenseReport> {
        sqlx::query("UPDATE expense_reports SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_report(id).await
    }

    pub async fn submit_report(&self, id: &str) -> AppResult<ExpenseReport> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE expense_reports SET status = 'submitted', submitted_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_report(id).await
    }

    pub async fn approve_report(&self, id: &str, approved_by: &str) -> AppResult<ExpenseReport> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE expense_reports SET status = 'approved', approved_by = ?, approved_at = ? WHERE id = ?")
            .bind(approved_by)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_report(id).await
    }

    pub async fn reject_report(&self, id: &str, reason: &str) -> AppResult<ExpenseReport> {
        sqlx::query(
            "UPDATE expense_reports SET status = 'rejected', rejected_reason = ? WHERE id = ?",
        )
        .bind(reason)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_report(id).await
    }

    pub async fn mark_paid(&self, id: &str) -> AppResult<ExpenseReport> {
        sqlx::query("UPDATE expense_reports SET status = 'paid' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_report(id).await
    }

    pub async fn resubmit_report(&self, id: &str) -> AppResult<ExpenseReport> {
        sqlx::query(
            "UPDATE expense_reports SET status = 'draft', rejected_reason = NULL, submitted_at = NULL WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_report(id).await
    }

    pub async fn delete_report(&self, id: &str) -> AppResult<()> {
        // Delete associated lines, per_diems, mileage first
        sqlx::query("DELETE FROM mileage WHERE report_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM per_diems WHERE report_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM expense_lines WHERE report_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM expense_reports WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn recalculate_report_total(&self, report_id: &str) -> AppResult<()> {
        let lines_total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM expense_lines WHERE report_id = ?",
        )
        .bind(report_id)
        .fetch_one(&self.pool)
        .await?;

        let per_diems_total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_cents), 0) FROM per_diems WHERE report_id = ?",
        )
        .bind(report_id)
        .fetch_one(&self.pool)
        .await?;

        let mileage_total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_cents), 0) FROM mileage WHERE report_id = ?",
        )
        .bind(report_id)
        .fetch_one(&self.pool)
        .await?;

        let total = lines_total + per_diems_total + mileage_total;

        sqlx::query("UPDATE expense_reports SET total_cents = ? WHERE id = ?")
            .bind(total)
            .bind(report_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- Expense Lines ---

    pub async fn list_lines(&self, report_id: &str) -> AppResult<Vec<ExpenseLine>> {
        let rows = sqlx::query_as::<_, ExpenseLine>(
            "SELECT id, report_id, expense_date, category_id, amount_cents, description, receipt_url, created_at FROM expense_lines WHERE report_id = ? ORDER BY expense_date",
        )
        .bind(report_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_line(&self, input: &CreateExpenseLineRequest) -> AppResult<ExpenseLine> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO expense_lines (id, report_id, expense_date, category_id, amount_cents, description, receipt_url) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.report_id)
        .bind(&input.expense_date)
        .bind(&input.category_id)
        .bind(input.amount_cents)
        .bind(&input.description)
        .bind(&input.receipt_url)
        .execute(&self.pool)
        .await?;

        self.recalculate_report_total(&input.report_id).await?;

        sqlx::query_as::<_, ExpenseLine>(
            "SELECT id, report_id, expense_date, category_id, amount_cents, description, receipt_url, created_at FROM expense_lines WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }

    // --- Per Diems ---

    pub async fn list_per_diems(&self, report_id: &str) -> AppResult<Vec<PerDiem>> {
        let rows = sqlx::query_as::<_, PerDiem>(
            "SELECT id, report_id, location, start_date, end_date, daily_rate_cents, total_cents, created_at FROM per_diems WHERE report_id = ? ORDER BY start_date",
        )
        .bind(report_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_all_per_diems(&self) -> AppResult<Vec<PerDiem>> {
        let rows = sqlx::query_as::<_, PerDiem>(
            "SELECT id, report_id, location, start_date, end_date, daily_rate_cents, total_cents, created_at FROM per_diems ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_per_diem(&self, input: &CreatePerDiemRequest) -> AppResult<PerDiem> {
        let id = uuid::Uuid::new_v4().to_string();

        // Calculate days from start_date to end_date (inclusive)
        let start = chrono::NaiveDate::parse_from_str(&input.start_date, "%Y-%m-%d")
            .map_err(|e| AppError::Validation(format!("Invalid start_date format: {}", e)))?;
        let end = chrono::NaiveDate::parse_from_str(&input.end_date, "%Y-%m-%d")
            .map_err(|e| AppError::Validation(format!("Invalid end_date format: {}", e)))?;

        if end < start {
            return Err(AppError::Validation(
                "end_date must be >= start_date".into(),
            ));
        }

        let days = (end - start).num_days() + 1;
        let total_cents = days * input.daily_rate_cents;

        sqlx::query(
            "INSERT INTO per_diems (id, report_id, location, start_date, end_date, daily_rate_cents, total_cents) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.report_id)
        .bind(&input.location)
        .bind(&input.start_date)
        .bind(&input.end_date)
        .bind(input.daily_rate_cents)
        .bind(total_cents)
        .execute(&self.pool)
        .await?;

        self.recalculate_report_total(&input.report_id).await?;

        sqlx::query_as::<_, PerDiem>(
            "SELECT id, report_id, location, start_date, end_date, daily_rate_cents, total_cents, created_at FROM per_diems WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }

    // --- Mileage ---

    pub async fn list_mileage(&self, report_id: &str) -> AppResult<Vec<Mileage>> {
        let rows = sqlx::query_as::<_, Mileage>(
            "SELECT id, report_id, origin, destination, distance_miles, rate_per_mile_cents, total_cents, expense_date, created_at FROM mileage WHERE report_id = ? ORDER BY expense_date",
        )
        .bind(report_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_all_mileage(&self) -> AppResult<Vec<Mileage>> {
        let rows = sqlx::query_as::<_, Mileage>(
            "SELECT id, report_id, origin, destination, distance_miles, rate_per_mile_cents, total_cents, expense_date, created_at FROM mileage ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_mileage(&self, input: &CreateMileageRequest) -> AppResult<Mileage> {
        let id = uuid::Uuid::new_v4().to_string();

        let total_cents = (input.distance_miles * input.rate_per_mile_cents as f64) as i64;

        sqlx::query(
            "INSERT INTO mileage (id, report_id, origin, destination, distance_miles, rate_per_mile_cents, total_cents, expense_date) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.report_id)
        .bind(&input.origin)
        .bind(&input.destination)
        .bind(input.distance_miles)
        .bind(input.rate_per_mile_cents)
        .bind(total_cents)
        .bind(&input.expense_date)
        .execute(&self.pool)
        .await?;

        self.recalculate_report_total(&input.report_id).await?;

        sqlx::query_as::<_, Mileage>(
            "SELECT id, report_id, origin, destination, distance_miles, rate_per_mile_cents, total_cents, expense_date, created_at FROM mileage WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;

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
    async fn test_category_crud() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        // Create
        let cat = repo
            .create_category(&make_category_request("Travel", 50000, true))
            .await
            .unwrap();
        assert_eq!(cat.name, "Travel");
        assert_eq!(cat.limit_cents, 50000);
        assert_eq!(cat.requires_receipt, 1);

        // Get
        let fetched = repo.get_category(&cat.id).await.unwrap();
        assert_eq!(fetched.name, "Travel");

        // List
        let list = repo.list_categories().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_report_crud() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "Q1 Travel"))
            .await
            .unwrap();
        assert_eq!(report.employee_id, "emp-1");
        assert_eq!(report.title, "Q1 Travel");
        assert_eq!(report.status, "draft");
        assert_eq!(report.total_cents, 0);

        let fetched = repo.get_report(&report.id).await.unwrap();
        assert_eq!(fetched.id, report.id);

        let list = repo.list_reports().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_create_expense_line_and_recalculate() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let cat = repo
            .create_category(&make_category_request("Meals", 20000, false))
            .await
            .unwrap();
        let report = repo
            .create_report(&make_report_request("emp-1", "March Expenses"))
            .await
            .unwrap();

        let line_input = CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-03-15".to_string(),
            category_id: cat.id.clone(),
            amount_cents: 5000,
            description: Some("Lunch with client".to_string()),
            receipt_url: None,
        };

        let line = repo.create_line(&line_input).await.unwrap();
        assert_eq!(line.amount_cents, 5000);
        assert_eq!(line.report_id, report.id);

        // Report total should be recalculated
        let updated_report = repo.get_report(&report.id).await.unwrap();
        assert_eq!(updated_report.total_cents, 5000);
    }

    #[tokio::test]
    async fn test_per_diem_calculation() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "NYC Trip"))
            .await
            .unwrap();

        let pd_input = CreatePerDiemRequest {
            report_id: report.id.clone(),
            location: "New York".to_string(),
            start_date: "2025-03-10".to_string(),
            end_date: "2025-03-12".to_string(),
            daily_rate_cents: 10000,
        };

        let pd = repo.create_per_diem(&pd_input).await.unwrap();
        // 3 days (10, 11, 12) * 10000 = 30000
        assert_eq!(pd.total_cents, 30000);

        let updated_report = repo.get_report(&report.id).await.unwrap();
        assert_eq!(updated_report.total_cents, 30000);
    }

    #[tokio::test]
    async fn test_mileage_calculation() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "Site Visit"))
            .await
            .unwrap();

        let m_input = CreateMileageRequest {
            report_id: report.id.clone(),
            origin: "Office".to_string(),
            destination: "Client Site".to_string(),
            distance_miles: 150.5,
            rate_per_mile_cents: 65,
            expense_date: "2025-03-20".to_string(),
        };

        let m = repo.create_mileage(&m_input).await.unwrap();
        // 150.5 * 65 = 9782 (truncated as i64)
        assert_eq!(m.total_cents, (150.5_f64 * 65.0_f64) as i64);

        let updated_report = repo.get_report(&report.id).await.unwrap();
        assert_eq!(updated_report.total_cents, (150.5_f64 * 65.0_f64) as i64);
    }

    #[tokio::test]
    async fn test_report_status_transitions() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();
        assert_eq!(report.status, "draft");

        // draft -> submitted
        let report = repo.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");
        assert!(report.submitted_at.is_some());

        // submitted -> approved
        let report = repo.approve_report(&report.id, "mgr-1").await.unwrap();
        assert_eq!(report.status, "approved");
        assert_eq!(report.approved_by.as_deref(), Some("mgr-1"));
        assert!(report.approved_at.is_some());

        // approved -> paid
        let report = repo.mark_paid(&report.id).await.unwrap();
        assert_eq!(report.status, "paid");
    }

    #[tokio::test]
    async fn test_report_rejected_transition() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();

        // draft -> submitted
        let report = repo.submit_report(&report.id).await.unwrap();
        assert_eq!(report.status, "submitted");

        // submitted -> rejected
        let report = repo
            .reject_report(&report.id, "Policy violation")
            .await
            .unwrap();
        assert_eq!(report.status, "rejected");
        assert_eq!(report.rejected_reason.as_deref(), Some("Policy violation"));
    }

    #[tokio::test]
    async fn test_per_diem_invalid_dates() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "Trip"))
            .await
            .unwrap();

        let pd_input = CreatePerDiemRequest {
            report_id: report.id.clone(),
            location: "NYC".to_string(),
            start_date: "2025-03-15".to_string(),
            end_date: "2025-03-10".to_string(), // end before start
            daily_rate_cents: 10000,
        };

        let result = repo.create_per_diem(&pd_input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_per_diem_single_day() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let report = repo
            .create_report(&make_report_request("emp-1", "Day Trip"))
            .await
            .unwrap();

        let pd_input = CreatePerDiemRequest {
            report_id: report.id.clone(),
            location: "Boston".to_string(),
            start_date: "2025-03-10".to_string(),
            end_date: "2025-03-10".to_string(),
            daily_rate_cents: 7500,
        };

        let pd = repo.create_per_diem(&pd_input).await.unwrap();
        // 1 day * 7500 = 7500
        assert_eq!(pd.total_cents, 7500);
    }

    #[tokio::test]
    async fn test_report_total_combines_all_items() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let cat = repo
            .create_category(&make_category_request("Travel", 100000, false))
            .await
            .unwrap();
        let report = repo
            .create_report(&make_report_request("emp-1", "Multi-expense trip"))
            .await
            .unwrap();

        // Add expense line
        repo.create_line(&CreateExpenseLineRequest {
            report_id: report.id.clone(),
            expense_date: "2025-04-01".to_string(),
            category_id: cat.id.clone(),
            amount_cents: 10000,
            description: None,
            receipt_url: None,
        })
        .await
        .unwrap();

        // Add per diem: 2 days * 5000 = 10000
        repo.create_per_diem(&CreatePerDiemRequest {
            report_id: report.id.clone(),
            location: "LA".to_string(),
            start_date: "2025-04-01".to_string(),
            end_date: "2025-04-02".to_string(),
            daily_rate_cents: 5000,
        })
        .await
        .unwrap();

        // Add mileage: 100 miles * 50 cents = 5000
        repo.create_mileage(&CreateMileageRequest {
            report_id: report.id.clone(),
            origin: "Home".to_string(),
            destination: "Office".to_string(),
            distance_miles: 100.0,
            rate_per_mile_cents: 50,
            expense_date: "2025-04-01".to_string(),
        })
        .await
        .unwrap();

        let report = repo.get_report(&report.id).await.unwrap();
        // 10000 + 10000 + 5000 = 25000
        assert_eq!(report.total_cents, 25000);
    }

    #[tokio::test]
    async fn test_category_get_not_found() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let result = repo.get_category("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_report_get_not_found() {
        let pool = setup().await;
        let repo = ExpenseRepo::new(pool);

        let result = repo.get_report("nonexistent").await;
        assert!(result.is_err());
    }
}

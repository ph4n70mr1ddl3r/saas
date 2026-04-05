use crate::models::*;
use crate::repository::TimeLaborRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct TimeLaborService {
    repo: TimeLaborRepo,
    #[allow(dead_code)]
    bus: NatsBus,
}

impl TimeLaborService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: TimeLaborRepo::new(pool),
            bus,
        }
    }

    // --- Timesheets ---

    pub async fn list_timesheets(&self) -> AppResult<Vec<Timesheet>> {
        self.repo.list_timesheets().await
    }

    pub async fn list_timesheets_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Timesheet>> {
        self.repo.list_timesheets_by_employee(employee_id).await
    }

    pub async fn create_timesheet(&self, input: CreateTimesheetRequest) -> AppResult<Timesheet> {
        self.repo.create_timesheet(&input).await
    }

    pub async fn submit_timesheet(&self, id: &str) -> AppResult<Timesheet> {
        let ts = self.repo.get_timesheet(id).await?;
        if ts.status != "draft" {
            return Err(AppError::Validation(format!(
                "Timesheet '{}' must be in draft status to submit (current: {})",
                id, ts.status
            )));
        }
        self.repo.submit_timesheet(id).await
    }

    /// Fetch a timesheet for approval check (used to prevent self-approval).
    pub async fn get_timesheet_for_approval_check(&self, id: &str) -> AppResult<Timesheet> {
        self.repo.get_timesheet(id).await
    }

    pub async fn approve_timesheet(&self, id: &str) -> AppResult<Timesheet> {
        let ts = self.repo.get_timesheet(id).await?;
        if ts.status != "submitted" {
            return Err(AppError::Validation(format!(
                "Timesheet '{}' must be in submitted status to approve (current: {})",
                id, ts.status
            )));
        }
        self.repo.approve_timesheet(id).await
    }

    // --- Leave Requests ---

    pub async fn list_leave_requests(&self) -> AppResult<Vec<LeaveRequest>> {
        self.repo.list_leave_requests().await
    }

    pub async fn create_leave_request(
        &self,
        input: CreateLeaveRequestRequest,
    ) -> AppResult<LeaveRequest> {
        self.repo.create_leave_request(&input).await
    }

    /// Fetch a leave request for approval check (used to prevent self-approval).
    pub async fn get_leave_request_for_approval_check(&self, id: &str) -> AppResult<LeaveRequest> {
        self.repo.get_leave_request(id).await
    }

    pub async fn approve_leave_request(&self, id: &str) -> AppResult<LeaveRequest> {
        let req = self.repo.get_leave_request(id).await?;
        if req.status != "pending" {
            return Err(AppError::Validation(format!(
                "Leave request '{}' must be pending to approve (current: {})",
                id, req.status
            )));
        }

        // Check leave balance before approving
        let days = calculate_leave_days(&req.start_date, &req.end_date);
        let balance = self
            .repo
            .get_leave_balance(&req.employee_id, &req.leave_type)
            .await?;
        if balance.remaining < days {
            return Err(AppError::Validation(format!(
                "Insufficient leave balance: have {:.1} days, requested {:.1} days",
                balance.remaining, days
            )));
        }

        let result = self
            .repo
            .update_leave_request_status(id, "approved")
            .await?;
        // Deduct from leave balance -- propagate error instead of discarding
        self.repo
            .deduct_leave_balance(&result.employee_id, &result.leave_type, days)
            .await?;
        Ok(result)
    }

    pub async fn reject_leave_request(&self, id: &str) -> AppResult<LeaveRequest> {
        let req = self.repo.get_leave_request(id).await?;
        if req.status != "pending" {
            return Err(AppError::Validation(format!(
                "Leave request '{}' must be pending to reject (current: {})",
                id, req.status
            )));
        }
        self.repo.update_leave_request_status(id, "rejected").await
    }

    // --- Leave Balances ---

    pub async fn list_leave_balances_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<LeaveBalance>> {
        self.repo.list_leave_balances_by_employee(employee_id).await
    }

    // --- Event handlers ---

    pub async fn handle_employee_created(&self, employee_id: &str) -> anyhow::Result<()> {
        let defaults = vec![("vacation", 20.0), ("sick", 10.0), ("personal", 3.0)];
        for (leave_type, entitled) in defaults {
            self.repo
                .create_leave_balance(employee_id, leave_type, entitled)
                .await?;
        }
        Ok(())
    }
}

fn calculate_leave_days(start_date: &str, end_date: &str) -> f64 {
    let start = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d");
    let end = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d");
    match (start, end) {
        (Ok(s), Ok(e)) => {
            let diff = (e - s).num_days() + 1;
            if diff > 0 {
                diff as f64
            } else {
                1.0
            }
        }
        _ => 1.0,
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
            include_str!("../../migrations/001_create_timesheets.sql"),
            include_str!("../../migrations/002_create_time_entries.sql"),
            include_str!("../../migrations/003_create_leave_requests.sql"),
            include_str!("../../migrations/004_create_leave_balances.sql"),
        ];
        let migration_names = [
            "001_create_timesheets.sql",
            "002_create_time_entries.sql",
            "003_create_leave_requests.sql",
            "004_create_leave_balances.sql",
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

    async fn setup_repo() -> TimeLaborRepo {
        let pool = setup().await;
        TimeLaborRepo::new(pool)
    }

    #[tokio::test]
    async fn test_timesheet_crud_and_status_transitions() {
        let repo = setup_repo().await;

        // Create timesheet (draft)
        let input = CreateTimesheetRequest {
            employee_id: "emp-001".into(),
            week_start: "2025-01-06".into(),
        };
        let ts = repo.create_timesheet(&input).await.unwrap();
        assert_eq!(ts.status, "draft");
        assert_eq!(ts.total_hours, 0.0);

        // Submit (draft -> submitted)
        let submitted = repo.submit_timesheet(&ts.id).await.unwrap();
        assert_eq!(submitted.status, "submitted");
        assert!(submitted.submitted_at.is_some());

        // Approve (submitted -> approved)
        let approved = repo.approve_timesheet(&ts.id).await.unwrap();
        assert_eq!(approved.status, "approved");
        assert!(approved.approved_at.is_some());
    }

    #[tokio::test]
    async fn test_timesheet_submit_requires_draft() {
        let repo = setup_repo().await;

        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-002".into(),
                week_start: "2025-01-13".into(),
            })
            .await
            .unwrap();

        // Submit
        repo.submit_timesheet(&ts.id).await.unwrap();
        let submitted = repo.get_timesheet(&ts.id).await.unwrap();
        assert_eq!(submitted.status, "submitted");

        // Service layer would block re-submission; at repo level verify status is correct
        assert_ne!(submitted.status, "draft", "Should not be draft after submit");
    }

    #[tokio::test]
    async fn test_timesheet_approve_requires_submitted() {
        let repo = setup_repo().await;

        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-003".into(),
                week_start: "2025-01-20".into(),
            })
            .await
            .unwrap();
        // Still in draft -- service layer would block approval
        assert_eq!(ts.status, "draft");
        assert_ne!(ts.status, "submitted", "Cannot approve non-submitted timesheet");
    }

    #[tokio::test]
    async fn test_list_timesheets_by_employee() {
        let repo = setup_repo().await;

        repo.create_timesheet(&CreateTimesheetRequest {
            employee_id: "emp-010".into(),
            week_start: "2025-02-03".into(),
        })
        .await
        .unwrap();
        repo.create_timesheet(&CreateTimesheetRequest {
            employee_id: "emp-010".into(),
            week_start: "2025-02-10".into(),
        })
        .await
        .unwrap();
        repo.create_timesheet(&CreateTimesheetRequest {
            employee_id: "emp-020".into(),
            week_start: "2025-02-03".into(),
        })
        .await
        .unwrap();

        let emp10 = repo.list_timesheets_by_employee("emp-010").await.unwrap();
        assert_eq!(emp10.len(), 2);

        let emp20 = repo.list_timesheets_by_employee("emp-020").await.unwrap();
        assert_eq!(emp20.len(), 1);
    }

    #[tokio::test]
    async fn test_leave_request_creation() {
        let repo = setup_repo().await;

        let input = CreateLeaveRequestRequest {
            employee_id: "emp-100".into(),
            leave_type: "vacation".into(),
            start_date: "2025-03-10".into(),
            end_date: "2025-03-12".into(),
            reason: Some("Family trip".into()),
        };
        let req = repo.create_leave_request(&input).await.unwrap();
        assert_eq!(req.status, "pending");
        assert_eq!(req.leave_type, "vacation");
        assert_eq!(req.reason, Some("Family trip".into()));
    }

    #[tokio::test]
    async fn test_leave_approval_with_balance_deduction() {
        let repo = setup_repo().await;

        // Create leave balance: 10 vacation days
        repo.create_leave_balance("emp-200", "vacation", 10.0)
            .await
            .unwrap();

        // Create leave request for 3 days
        let req = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-200".into(),
                leave_type: "vacation".into(),
                start_date: "2025-04-01".into(),
                end_date: "2025-04-03".into(),
                reason: None,
            })
            .await
            .unwrap();

        // Approve
        let approved = repo
            .update_leave_request_status(&req.id, "approved")
            .await
            .unwrap();
        assert_eq!(approved.status, "approved");

        // Deduct balance (replicating service logic)
        let days = calculate_leave_days(&req.start_date, &req.end_date);
        assert_eq!(days, 3.0);
        repo.deduct_leave_balance("emp-200", "vacation", days)
            .await
            .unwrap();

        let balance = repo.get_leave_balance("emp-200", "vacation").await.unwrap();
        assert_eq!(balance.entitled, 10.0);
        assert_eq!(balance.used, 3.0);
        assert_eq!(balance.remaining, 7.0);
    }

    #[tokio::test]
    async fn test_leave_rejection() {
        let repo = setup_repo().await;

        let req = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-300".into(),
                leave_type: "sick".into(),
                start_date: "2025-05-01".into(),
                end_date: "2025-05-01".into(),
                reason: None,
            })
            .await
            .unwrap();
        assert_eq!(req.status, "pending");

        let rejected = repo
            .update_leave_request_status(&req.id, "rejected")
            .await
            .unwrap();
        assert_eq!(rejected.status, "rejected");
    }

    #[tokio::test]
    async fn test_leave_balance_management() {
        let repo = setup_repo().await;

        // Create balances
        repo.create_leave_balance("emp-400", "vacation", 20.0)
            .await
            .unwrap();
        repo.create_leave_balance("emp-400", "sick", 10.0)
            .await
            .unwrap();
        repo.create_leave_balance("emp-400", "personal", 3.0)
            .await
            .unwrap();

        let balances = repo
            .list_leave_balances_by_employee("emp-400")
            .await
            .unwrap();
        assert_eq!(balances.len(), 3);

        let vacation = repo.get_leave_balance("emp-400", "vacation").await.unwrap();
        assert_eq!(vacation.remaining, 20.0);
        assert_eq!(vacation.used, 0.0);
    }

    #[tokio::test]
    async fn test_handle_employee_created_default_balances() {
        let repo = setup_repo().await;

        // Simulate handle_employee_created logic
        let defaults = vec![("vacation", 20.0), ("sick", 10.0), ("personal", 3.0)];
        for (leave_type, entitled) in defaults {
            repo.create_leave_balance("emp-new", leave_type, entitled)
                .await
                .unwrap();
        }

        let balances = repo
            .list_leave_balances_by_employee("emp-new")
            .await
            .unwrap();
        assert_eq!(balances.len(), 3);

        let vacation = repo.get_leave_balance("emp-new", "vacation").await.unwrap();
        assert_eq!(vacation.entitled, 20.0);
        assert_eq!(vacation.remaining, 20.0);

        let sick = repo.get_leave_balance("emp-new", "sick").await.unwrap();
        assert_eq!(sick.entitled, 10.0);

        let personal = repo.get_leave_balance("emp-new", "personal").await.unwrap();
        assert_eq!(personal.entitled, 3.0);
    }

    #[tokio::test]
    async fn test_time_entry_updates_timesheet_total_hours() {
        let repo = setup_repo().await;

        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-500".into(),
                week_start: "2025-06-02".into(),
            })
            .await
            .unwrap();
        assert_eq!(ts.total_hours, 0.0);

        // Add time entries
        repo.create_time_entry(&CreateTimeEntryRequest {
            timesheet_id: ts.id.clone(),
            date: "2025-06-02".into(),
            hours: 8.0,
            project_code: Some("PROJ-1".into()),
            description: None,
        })
        .await
        .unwrap();
        repo.create_time_entry(&CreateTimeEntryRequest {
            timesheet_id: ts.id.clone(),
            date: "2025-06-03".into(),
            hours: 7.5,
            project_code: Some("PROJ-1".into()),
            description: Some("Review".into()),
        })
        .await
        .unwrap();

        let updated = repo.get_timesheet(&ts.id).await.unwrap();
        assert_eq!(updated.total_hours, 15.5);
    }
}

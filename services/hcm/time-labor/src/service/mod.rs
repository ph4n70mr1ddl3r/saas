use crate::models::*;
use crate::repository::TimeLaborRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    TimesheetSubmitted, TimesheetApproved, LeaveRequestSubmitted, LeaveRequestApproved,
    LeaveRequestRejected,
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct TimeLaborService {
    repo: TimeLaborRepo,
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
        let ts = self.repo.submit_timesheet(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.timelabor.timesheet.submitted",
                TimesheetSubmitted {
                    timesheet_id: ts.id.clone(),
                    employee_id: ts.employee_id.clone(),
                    week_start: ts.week_start.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.timelabor.timesheet.submitted",
                e
            );
        }
        Ok(ts)
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
        let ts = self.repo.approve_timesheet(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.timelabor.timesheet.approved",
                TimesheetApproved {
                    timesheet_id: ts.id.clone(),
                    employee_id: ts.employee_id.clone(),
                    week_start: ts.week_start.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.timelabor.timesheet.approved",
                e
            );
        }
        Ok(ts)
    }

    // --- Leave Requests ---

    pub async fn list_leave_requests(&self) -> AppResult<Vec<LeaveRequest>> {
        self.repo.list_leave_requests().await
    }

    pub async fn create_leave_request(
        &self,
        input: CreateLeaveRequestRequest,
    ) -> AppResult<LeaveRequest> {
        let req = self.repo.create_leave_request(&input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.timelabor.leave.submitted",
                LeaveRequestSubmitted {
                    request_id: req.id.clone(),
                    employee_id: req.employee_id.clone(),
                    leave_type: req.leave_type.clone(),
                    start_date: req.start_date.clone(),
                    end_date: req.end_date.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.timelabor.leave.submitted",
                e
            );
        }
        Ok(req)
    }

    /// Fetch a leave request for approval check (used to prevent self-approval).
    pub async fn get_leave_request_for_approval_check(
        &self,
        id: &str,
    ) -> AppResult<LeaveRequest> {
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
        // Deduct from leave balance
        self.repo
            .deduct_leave_balance(&result.employee_id, &result.leave_type, days)
            .await?;

        if let Err(e) = self
            .bus
            .publish(
                "hcm.timelabor.leave.approved",
                LeaveRequestApproved {
                    request_id: result.id.clone(),
                    employee_id: result.employee_id.clone(),
                    leave_type: result.leave_type.clone(),
                    days,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.timelabor.leave.approved",
                e
            );
        }

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
        let result = self.repo.update_leave_request_status(id, "rejected").await?;
        if let Err(e) = self
            .bus
            .publish(
                "hcm.timelabor.leave.rejected",
                LeaveRequestRejected {
                    request_id: result.id.clone(),
                    employee_id: result.employee_id.clone(),
                    leave_type: result.leave_type.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "hcm.timelabor.leave.rejected",
                e
            );
        }
        Ok(result)
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

    /// Reject all pending leave requests when an employee is terminated.
    pub async fn handle_employee_terminated(&self, employee_id: &str) -> AppResult<()> {
        let requests = self.repo.list_leave_requests().await?;
        for req in requests {
            if req.employee_id == employee_id && req.status == "pending" {
                let result = self.repo.update_leave_request_status(&req.id, "rejected").await?;
                if let Err(e) = self
                    .bus
                    .publish(
                        "hcm.timelabor.leave.rejected",
                        LeaveRequestRejected {
                            request_id: result.id.clone(),
                            employee_id: result.employee_id.clone(),
                            leave_type: result.leave_type.clone(),
                        },
                    )
                    .await
                {
                    tracing::error!(
                        "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                        "hcm.timelabor.leave.rejected",
                        e
                    );
                }
            }
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
    async fn test_timesheet_crud() {
        let repo = setup_repo().await;
        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-001".into(),
                week_start: "2025-06-02".into(),
            })
            .await
            .unwrap();
        assert_eq!(ts.employee_id, "emp-001");
        assert_eq!(ts.week_start, "2025-06-02");
        assert_eq!(ts.status, "draft");
        assert_eq!(ts.total_hours, 0.0);

        let fetched = repo.get_timesheet(&ts.id).await.unwrap();
        assert_eq!(fetched.id, ts.id);

        let list = repo.list_timesheets().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_timesheet_status_transitions() {
        let repo = setup_repo().await;
        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-002".into(),
                week_start: "2025-06-09".into(),
            })
            .await
            .unwrap();
        assert_eq!(ts.status, "draft");

        let submitted = repo.submit_timesheet(&ts.id).await.unwrap();
        assert_eq!(submitted.status, "submitted");
        assert!(submitted.submitted_at.is_some());

        let approved = repo.approve_timesheet(&ts.id).await.unwrap();
        assert_eq!(approved.status, "approved");
        assert!(approved.approved_at.is_some());
    }

    #[tokio::test]
    async fn test_timesheet_submit_only_from_draft() {
        let repo = setup_repo().await;
        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-003".into(),
                week_start: "2025-06-16".into(),
            })
            .await
            .unwrap();
        repo.submit_timesheet(&ts.id).await.unwrap();
        // Second submit should fail
        let result = repo.submit_timesheet(&ts.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timesheet_approve_only_from_submitted() {
        let repo = setup_repo().await;
        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-004".into(),
                week_start: "2025-06-23".into(),
            })
            .await
            .unwrap();
        // Approve without submitting should fail
        let result = repo.approve_timesheet(&ts.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timesheet_by_employee() {
        let repo = setup_repo().await;
        repo.create_timesheet(&CreateTimesheetRequest {
            employee_id: "emp-010".into(),
            week_start: "2025-07-01".into(),
        })
        .await
        .unwrap();
        repo.create_timesheet(&CreateTimesheetRequest {
            employee_id: "emp-010".into(),
            week_start: "2025-07-08".into(),
        })
        .await
        .unwrap();
        repo.create_timesheet(&CreateTimesheetRequest {
            employee_id: "emp-020".into(),
            week_start: "2025-07-01".into(),
        })
        .await
        .unwrap();

        let emp010 = repo.list_timesheets_by_employee("emp-010").await.unwrap();
        assert_eq!(emp010.len(), 2);
        let emp020 = repo.list_timesheets_by_employee("emp-020").await.unwrap();
        assert_eq!(emp020.len(), 1);
    }

    #[tokio::test]
    async fn test_time_entries_and_total_calculation() {
        let repo = setup_repo().await;
        let ts = repo
            .create_timesheet(&CreateTimesheetRequest {
                employee_id: "emp-030".into(),
                week_start: "2025-08-04".into(),
            })
            .await
            .unwrap();

        repo.create_time_entry(&CreateTimeEntryRequest {
            timesheet_id: ts.id.clone(),
            date: "2025-08-04".into(),
            hours: 8.0,
            project_code: Some("PROJ-1".into()),
            description: Some("Development".into()),
        })
        .await
        .unwrap();

        repo.create_time_entry(&CreateTimeEntryRequest {
            timesheet_id: ts.id.clone(),
            date: "2025-08-05".into(),
            hours: 7.5,
            project_code: Some("PROJ-1".into()),
            description: Some("Code review".into()),
        })
        .await
        .unwrap();

        let ts = repo.get_timesheet(&ts.id).await.unwrap();
        assert_eq!(ts.total_hours, 15.5);

        let entries = repo.list_time_entries(&ts.id).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_leave_request_create_and_approve() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-100", "vacation", 20.0)
            .await
            .unwrap();

        let req = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-100".into(),
                leave_type: "vacation".into(),
                start_date: "2025-07-14".into(),
                end_date: "2025-07-18".into(),
                reason: Some("Family trip".into()),
            })
            .await
            .unwrap();
        assert_eq!(req.employee_id, "emp-100");
        assert_eq!(req.leave_type, "vacation");
        assert_eq!(req.status, "pending");

        let approved = repo.update_leave_request_status(&req.id, "approved").await.unwrap();
        assert_eq!(approved.status, "approved");

        // Deduct balance
        let days = calculate_leave_days("2025-07-14", "2025-07-18");
        assert_eq!(days, 5.0);
        repo.deduct_leave_balance("emp-100", "vacation", days)
            .await
            .unwrap();

        let balance = repo.get_leave_balance("emp-100", "vacation").await.unwrap();
        assert_eq!(balance.used, 5.0);
        assert_eq!(balance.remaining, 15.0);
    }

    #[tokio::test]
    async fn test_leave_request_reject() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-101", "sick", 10.0)
            .await
            .unwrap();

        let req = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-101".into(),
                leave_type: "sick".into(),
                start_date: "2025-08-01".into(),
                end_date: "2025-08-01".into(),
                reason: None,
            })
            .await
            .unwrap();

        let rejected = repo.update_leave_request_status(&req.id, "rejected").await.unwrap();
        assert_eq!(rejected.status, "rejected");
    }

    #[tokio::test]
    async fn test_leave_balance_deduction() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-200", "vacation", 20.0)
            .await
            .unwrap();

        repo.deduct_leave_balance("emp-200", "vacation", 5.0)
            .await
            .unwrap();

        let balance = repo.get_leave_balance("emp-200", "vacation").await.unwrap();
        assert_eq!(balance.entitled, 20.0);
        assert_eq!(balance.used, 5.0);
        assert_eq!(balance.remaining, 15.0);
    }

    #[tokio::test]
    async fn test_leave_balance_list_by_employee() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-300", "vacation", 20.0)
            .await
            .unwrap();
        repo.create_leave_balance("emp-300", "sick", 10.0)
            .await
            .unwrap();
        repo.create_leave_balance("emp-300", "personal", 3.0)
            .await
            .unwrap();

        let balances = repo.list_leave_balances_by_employee("emp-300").await.unwrap();
        assert_eq!(balances.len(), 3);
    }

    #[tokio::test]
    async fn test_calculate_leave_days_function() {
        assert_eq!(calculate_leave_days("2025-07-14", "2025-07-14"), 1.0);
        assert_eq!(calculate_leave_days("2025-07-14", "2025-07-18"), 5.0);
        assert_eq!(calculate_leave_days("invalid", "2025-07-18"), 1.0);
        assert_eq!(calculate_leave_days("2025-07-01", "2025-07-31"), 31.0);
    }

    #[tokio::test]
    async fn test_handle_employee_created_sets_balances() {
        let repo = setup_repo().await;

        let defaults = vec![("vacation", 20.0), ("sick", 10.0), ("personal", 3.0)];
        for (leave_type, entitled) in defaults {
            repo.create_leave_balance("emp-new", leave_type, entitled)
                .await
                .unwrap();
        }

        let balances = repo.list_leave_balances_by_employee("emp-new").await.unwrap();
        assert_eq!(balances.len(), 3);

        let vac = balances.iter().find(|b| b.leave_type == "vacation").unwrap();
        assert_eq!(vac.entitled, 20.0);
        assert_eq!(vac.remaining, 20.0);

        let sick = balances.iter().find(|b| b.leave_type == "sick").unwrap();
        assert_eq!(sick.entitled, 10.0);

        let personal = balances.iter().find(|b| b.leave_type == "personal").unwrap();
        assert_eq!(personal.entitled, 3.0);
    }

    #[tokio::test]
    async fn test_leave_request_list() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-400", "vacation", 20.0)
            .await
            .unwrap();

        repo.create_leave_request(&CreateLeaveRequestRequest {
            employee_id: "emp-400".into(),
            leave_type: "vacation".into(),
            start_date: "2025-09-01".into(),
            end_date: "2025-09-05".into(),
            reason: None,
        })
        .await
        .unwrap();
        repo.create_leave_request(&CreateLeaveRequestRequest {
            employee_id: "emp-400".into(),
            leave_type: "vacation".into(),
            start_date: "2025-10-01".into(),
            end_date: "2025-10-03".into(),
            reason: Some("Conference".into()),
        })
        .await
        .unwrap();

        let list = repo.list_leave_requests().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_leave_insufficient_balance_prevented() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-500", "personal", 3.0)
            .await
            .unwrap();

        // Request 5 days when only 3 available
        let balance = repo.get_leave_balance("emp-500", "personal").await.unwrap();
        assert_eq!(balance.remaining, 3.0);

        let days = calculate_leave_days("2025-11-01", "2025-11-05");
        assert_eq!(days, 5.0);
        assert!(days > balance.remaining, "Should detect insufficient balance");
    }

    #[tokio::test]
    async fn test_timesheet_not_found() {
        let repo = setup_repo().await;
        let result = repo.get_timesheet("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_leave_request_not_found() {
        let repo = setup_repo().await;
        let result = repo.get_leave_request("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_employee_terminated_rejects_pending_leaves() {
        let repo = setup_repo().await;
        repo.create_leave_balance("emp-term", "vacation", 20.0)
            .await
            .unwrap();

        // Create two pending leave requests
        let req1 = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-term".into(),
                leave_type: "vacation".into(),
                start_date: "2025-08-01".into(),
                end_date: "2025-08-05".into(),
                reason: None,
            })
            .await
            .unwrap();
        let req2 = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-term".into(),
                leave_type: "vacation".into(),
                start_date: "2025-09-01".into(),
                end_date: "2025-09-03".into(),
                reason: None,
            })
            .await
            .unwrap();

        // Create an already-approved request (should not be affected)
        let req3 = repo
            .create_leave_request(&CreateLeaveRequestRequest {
                employee_id: "emp-term".into(),
                leave_type: "vacation".into(),
                start_date: "2025-10-01".into(),
                end_date: "2025-10-02".into(),
                reason: None,
            })
            .await
            .unwrap();
        repo.update_leave_request_status(&req3.id, "approved").await.unwrap();

        // Simulate termination handler: reject all pending requests for this employee
        let requests = repo.list_leave_requests().await.unwrap();
        for req in requests {
            if req.employee_id == "emp-term" && req.status == "pending" {
                repo.update_leave_request_status(&req.id, "rejected").await.unwrap();
            }
        }

        // Verify pending requests are now rejected
        let r1 = repo.get_leave_request(&req1.id).await.unwrap();
        assert_eq!(r1.status, "rejected");
        let r2 = repo.get_leave_request(&req2.id).await.unwrap();
        assert_eq!(r2.status, "rejected");

        // Approved request should remain approved
        let r3 = repo.get_leave_request(&req3.id).await.unwrap();
        assert_eq!(r3.status, "approved");
    }
}

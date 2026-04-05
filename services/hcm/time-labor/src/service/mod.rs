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

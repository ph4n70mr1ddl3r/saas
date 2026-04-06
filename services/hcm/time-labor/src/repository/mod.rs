use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct TimeLaborRepo {
    pool: SqlitePool,
}

impl TimeLaborRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Timesheets ---

    pub async fn list_timesheets(&self) -> AppResult<Vec<Timesheet>> {
        let rows = sqlx::query_as::<_, Timesheet>(
            "SELECT id, employee_id, week_start, status, total_hours, submitted_at, approved_at, created_at FROM timesheets ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_timesheet(&self, id: &str) -> AppResult<Timesheet> {
        sqlx::query_as::<_, Timesheet>(
            "SELECT id, employee_id, week_start, status, total_hours, submitted_at, approved_at, created_at FROM timesheets WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Timesheet '{}' not found", id)))
    }

    pub async fn list_timesheets_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<Timesheet>> {
        let rows = sqlx::query_as::<_, Timesheet>(
            "SELECT id, employee_id, week_start, status, total_hours, submitted_at, approved_at, created_at FROM timesheets WHERE employee_id = ? ORDER BY week_start DESC"
        )
        .bind(employee_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_timesheet(&self, input: &CreateTimesheetRequest) -> AppResult<Timesheet> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO timesheets (id, employee_id, week_start) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(&input.employee_id)
            .bind(&input.week_start)
            .execute(&self.pool)
            .await?;
        self.get_timesheet(&id).await
    }

    pub async fn submit_timesheet(&self, id: &str) -> AppResult<Timesheet> {
        let ts = self.get_timesheet(id).await?;
        if ts.status != "draft" {
            return Err(AppError::Validation(format!(
                "Timesheet '{}' must be in draft status to submit (current: {})",
                id, ts.status
            )));
        }
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE timesheets SET status = 'submitted', submitted_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_timesheet(id).await
    }

    pub async fn approve_timesheet(&self, id: &str) -> AppResult<Timesheet> {
        let ts = self.get_timesheet(id).await?;
        if ts.status != "submitted" {
            return Err(AppError::Validation(format!(
                "Timesheet '{}' must be in submitted status to approve (current: {})",
                id, ts.status
            )));
        }
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE timesheets SET status = 'approved', approved_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_timesheet(id).await
    }

    // --- Time Entries ---

    pub async fn list_time_entries(&self, timesheet_id: &str) -> AppResult<Vec<TimeEntry>> {
        let rows = sqlx::query_as::<_, TimeEntry>(
            "SELECT id, timesheet_id, date, hours, project_code, description, created_at FROM time_entries WHERE timesheet_id = ? ORDER BY date"
        )
        .bind(timesheet_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_time_entry(&self, input: &CreateTimeEntryRequest) -> AppResult<TimeEntry> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO time_entries (id, timesheet_id, date, hours, project_code, description) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.timesheet_id)
        .bind(&input.date)
        .bind(input.hours)
        .bind(&input.project_code)
        .bind(&input.description)
        .execute(&self.pool)
        .await?;

        // Update total_hours on parent timesheet
        sqlx::query(
            "UPDATE timesheets SET total_hours = (SELECT COALESCE(SUM(hours), 0) FROM time_entries WHERE timesheet_id = ?) WHERE id = ?"
        )
        .bind(&input.timesheet_id)
        .bind(&input.timesheet_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, TimeEntry>(
            "SELECT id, timesheet_id, date, hours, project_code, description, created_at FROM time_entries WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to read created time entry".into()))
    }

    // --- Leave Requests ---

    pub async fn list_leave_requests(&self) -> AppResult<Vec<LeaveRequest>> {
        let rows = sqlx::query_as::<_, LeaveRequest>(
            "SELECT id, employee_id, leave_type, start_date, end_date, status, reason, created_at FROM leave_requests ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_leave_request(&self, id: &str) -> AppResult<LeaveRequest> {
        sqlx::query_as::<_, LeaveRequest>(
            "SELECT id, employee_id, leave_type, start_date, end_date, status, reason, created_at FROM leave_requests WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Leave request '{}' not found", id)))
    }

    pub async fn create_leave_request(
        &self,
        input: &CreateLeaveRequestRequest,
    ) -> AppResult<LeaveRequest> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO leave_requests (id, employee_id, leave_type, start_date, end_date, reason) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.leave_type)
        .bind(&input.start_date)
        .bind(&input.end_date)
        .bind(&input.reason)
        .execute(&self.pool)
        .await?;
        self.get_leave_request(&id).await
    }

    pub async fn update_leave_request_status(
        &self,
        id: &str,
        status: &str,
    ) -> AppResult<LeaveRequest> {
        sqlx::query("UPDATE leave_requests SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_leave_request(id).await
    }

    // --- Leave Balances ---

    pub async fn list_leave_balances_by_employee(
        &self,
        employee_id: &str,
    ) -> AppResult<Vec<LeaveBalance>> {
        let rows = sqlx::query_as::<_, LeaveBalance>(
            "SELECT id, employee_id, leave_type, entitled, used, remaining FROM leave_balances WHERE employee_id = ?"
        )
        .bind(employee_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_leave_balance(
        &self,
        employee_id: &str,
        leave_type: &str,
    ) -> AppResult<LeaveBalance> {
        sqlx::query_as::<_, LeaveBalance>(
            "SELECT id, employee_id, leave_type, entitled, used, remaining FROM leave_balances WHERE employee_id = ? AND leave_type = ?"
        )
        .bind(employee_id)
        .bind(leave_type)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Leave balance not found".into()))
    }

    pub async fn create_leave_balance(
        &self,
        employee_id: &str,
        leave_type: &str,
        entitled: f64,
    ) -> AppResult<LeaveBalance> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO leave_balances (id, employee_id, leave_type, entitled, used, remaining) VALUES (?, ?, ?, ?, 0, ?)"
        )
        .bind(&id)
        .bind(employee_id)
        .bind(leave_type)
        .bind(entitled)
        .bind(entitled)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, LeaveBalance>(
            "SELECT id, employee_id, leave_type, entitled, used, remaining FROM leave_balances WHERE id = ?"
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to read created leave balance".into()))
    }

    pub async fn deduct_leave_balance(
        &self,
        employee_id: &str,
        leave_type: &str,
        days: f64,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE leave_balances SET used = used + ?, remaining = remaining - ? WHERE employee_id = ? AND leave_type = ? AND remaining >= ?"
        )
        .bind(days)
        .bind(days)
        .bind(employee_id)
        .bind(leave_type)
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

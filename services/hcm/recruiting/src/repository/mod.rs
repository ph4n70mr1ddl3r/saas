use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct RecruitingRepo {
    pool: SqlitePool,
}

impl RecruitingRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Job Postings ---

    pub async fn list_jobs(&self) -> AppResult<Vec<JobPosting>> {
        let rows = sqlx::query_as::<_, JobPosting>(
            "SELECT id, title, department_id, description, requirements, status, posted_at, closed_at FROM job_postings ORDER BY posted_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_job(&self, id: &str) -> AppResult<JobPosting> {
        sqlx::query_as::<_, JobPosting>(
            "SELECT id, title, department_id, description, requirements, status, posted_at, closed_at FROM job_postings WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Job posting '{}' not found", id)))
    }

    pub async fn create_job(&self, input: &CreateJobRequest) -> AppResult<JobPosting> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO job_postings (id, title, department_id, description, requirements) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.title)
        .bind(&input.department_id)
        .bind(&input.description)
        .bind(&input.requirements)
        .execute(&self.pool)
        .await?;
        self.get_job(&id).await
    }

    pub async fn update_job(&self, id: &str, input: &UpdateJobRequest) -> AppResult<JobPosting> {
        let existing = self.get_job(id).await?;
        let title = input.title.as_deref().unwrap_or(&existing.title);
        let department_id = input
            .department_id
            .as_deref()
            .unwrap_or(&existing.department_id);
        let description = input.description.as_ref().or(existing.description.as_ref());
        let requirements = input
            .requirements
            .as_ref()
            .or(existing.requirements.as_ref());
        let status = input.status.as_deref().unwrap_or(&existing.status);
        let closed_at: Option<String> = if status == "closed" || status == "filled" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            existing.closed_at.clone()
        };

        sqlx::query(
            "UPDATE job_postings SET title = ?, department_id = ?, description = ?, requirements = ?, status = ?, closed_at = ? WHERE id = ?"
        )
        .bind(title)
        .bind(department_id)
        .bind(description)
        .bind(requirements)
        .bind(status)
        .bind(closed_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_job(id).await
    }

    // --- Applications ---

    pub async fn list_applications(&self) -> AppResult<Vec<Application>> {
        let rows = sqlx::query_as::<_, Application>(
            "SELECT id, job_id, candidate_first_name, candidate_last_name, candidate_email, status, applied_at, notes FROM applications ORDER BY applied_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_application(&self, id: &str) -> AppResult<Application> {
        sqlx::query_as::<_, Application>(
            "SELECT id, job_id, candidate_first_name, candidate_last_name, candidate_email, status, applied_at, notes FROM applications WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Application '{}' not found", id)))
    }

    pub async fn list_applications_by_job(&self, job_id: &str) -> AppResult<Vec<Application>> {
        let rows = sqlx::query_as::<_, Application>(
            "SELECT id, job_id, candidate_first_name, candidate_last_name, candidate_email, status, applied_at, notes FROM applications WHERE job_id = ? ORDER BY applied_at DESC"
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn create_application(
        &self,
        input: &CreateApplicationRequest,
    ) -> AppResult<Application> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO applications (id, job_id, candidate_first_name, candidate_last_name, candidate_email, notes) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.job_id)
        .bind(&input.candidate_first_name)
        .bind(&input.candidate_last_name)
        .bind(&input.candidate_email)
        .bind(&input.notes)
        .execute(&self.pool)
        .await?;
        self.get_application(&id).await
    }

    pub async fn update_application_status(
        &self,
        id: &str,
        status: &str,
        notes: Option<&str>,
    ) -> AppResult<Application> {
        sqlx::query("UPDATE applications SET status = ?, notes = COALESCE(?, notes) WHERE id = ?")
            .bind(status)
            .bind(notes)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_application(id).await
    }
}

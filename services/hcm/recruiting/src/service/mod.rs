use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use saas_proto::events::ApplicationStatusChanged;
use crate::models::*;
use crate::repository::RecruitingRepo;

#[derive(Clone)]
pub struct RecruitingService {
    repo: RecruitingRepo,
    bus: NatsBus,
}

impl RecruitingService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: RecruitingRepo::new(pool),
            bus,
        }
    }

    // --- Jobs ---

    pub async fn list_jobs(&self) -> AppResult<Vec<JobPosting>> {
        self.repo.list_jobs().await
    }

    pub async fn get_job(&self, id: &str) -> AppResult<JobPosting> {
        self.repo.get_job(id).await
    }

    pub async fn create_job(&self, input: CreateJobRequest) -> AppResult<JobPosting> {
        self.repo.create_job(&input).await
    }

    pub async fn update_job(&self, id: &str, input: UpdateJobRequest) -> AppResult<JobPosting> {
        self.repo.update_job(id, &input).await
    }

    // --- Applications ---

    pub async fn list_applications(&self) -> AppResult<Vec<Application>> {
        self.repo.list_applications().await
    }

    pub async fn create_application(&self, input: CreateApplicationRequest) -> AppResult<Application> {
        let job = self.repo.get_job(&input.job_id).await?;
        if job.status != "open" {
            return Err(AppError::Validation(
                format!("Job '{}' is not open for applications (status: {})", input.job_id, job.status)
            ));
        }
        self.repo.create_application(&input).await
    }

    pub async fn update_application_status(&self, id: &str, input: UpdateApplicationStatusRequest) -> AppResult<Application> {
        let existing = self.repo.get_application(id).await?;
        let old_status = existing.status.clone();

        let app = self.repo.update_application_status(id, &input.status, input.notes.as_deref()).await?;

        if input.status == "hired" {
            let event = ApplicationStatusChanged {
                application_id: id.to_string(),
                job_id: app.job_id.clone(),
                candidate_email: app.candidate_email.clone(),
                old_status,
                new_status: "hired".to_string(),
            };
            let _ = self.bus.publish("hcm.recruiting.application.status_changed", event).await;
        }

        Ok(app)
    }

    pub async fn list_applications_by_job(&self, job_id: &str) -> AppResult<Vec<Application>> {
        let _ = self.repo.get_job(job_id).await?;
        self.repo.list_applications_by_job(job_id).await
    }
}

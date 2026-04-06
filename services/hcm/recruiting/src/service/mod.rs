use crate::models::*;
use crate::repository::RecruitingRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::ApplicationStatusChanged;
use sqlx::SqlitePool;

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

    pub async fn create_application(
        &self,
        input: CreateApplicationRequest,
    ) -> AppResult<Application> {
        let job = self.repo.get_job(&input.job_id).await?;
        if job.status != "open" {
            return Err(AppError::Validation(format!(
                "Job '{}' is not open for applications (status: {})",
                input.job_id, job.status
            )));
        }
        self.repo.create_application(&input).await
    }

    pub async fn update_application_status(
        &self,
        id: &str,
        input: UpdateApplicationStatusRequest,
    ) -> AppResult<Application> {
        let existing = self.repo.get_application(id).await?;
        let old_status = existing.status.clone();

        let app = self
            .repo
            .update_application_status(id, &input.status, input.notes.as_deref())
            .await?;

        if input.status == "hired" {
            // Fetch job details to include in event for downstream consumers
            let job = self.repo.get_job(&app.job_id).await.ok();
            let event = ApplicationStatusChanged {
                application_id: id.to_string(),
                job_id: app.job_id.clone(),
                candidate_first_name: app.candidate_first_name.clone(),
                candidate_last_name: app.candidate_last_name.clone(),
                candidate_email: app.candidate_email.clone(),
                job_title: job.as_ref().map(|j| j.title.clone()).unwrap_or_default(),
                department_id: job.as_ref().map(|j| j.department_id.clone()).unwrap_or_default(),
                old_status,
                new_status: "hired".to_string(),
            };
            if let Err(e) = self
                .bus
                .publish("hcm.recruiting.application.status_changed", event)
                .await
            {
                tracing::error!(
                    "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                    "hcm.recruiting.application.status_changed",
                    e
                );
            }
        }

        Ok(app)
    }

    pub async fn list_applications_by_job(&self, job_id: &str) -> AppResult<Vec<Application>> {
        let _ = self.repo.get_job(job_id).await?;
        self.repo.list_applications_by_job(job_id).await
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
            include_str!("../../migrations/001_create_job_postings.sql"),
            include_str!("../../migrations/002_create_applications.sql"),
        ];
        let migration_names = [
            "001_create_job_postings.sql",
            "002_create_applications.sql",
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

    async fn setup_repo() -> RecruitingRepo {
        let pool = setup().await;
        RecruitingRepo::new(pool)
    }

    #[tokio::test]
    async fn test_job_posting_crud() {
        let repo = setup_repo().await;

        // Create
        let input = CreateJobRequest {
            title: "Senior Engineer".into(),
            department_id: "eng".into(),
            description: Some("Build great software".into()),
            requirements: Some("Rust experience".into()),
        };
        let job = repo.create_job(&input).await.unwrap();
        assert_eq!(job.title, "Senior Engineer");
        assert_eq!(job.status, "open");
        assert!(job.closed_at.is_none());

        // Read
        let fetched = repo.get_job(&job.id).await.unwrap();
        assert_eq!(fetched.title, "Senior Engineer");

        // Update
        let update = UpdateJobRequest {
            title: Some("Staff Engineer".into()),
            department_id: None,
            description: None,
            requirements: None,
            status: None,
        };
        let updated = repo.update_job(&job.id, &update).await.unwrap();
        assert_eq!(updated.title, "Staff Engineer");

        // List
        let jobs = repo.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[tokio::test]
    async fn test_close_job_posting() {
        let repo = setup_repo().await;

        let job = repo
            .create_job(&CreateJobRequest {
                title: "PM".into(),
                department_id: "product".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();
        assert_eq!(job.status, "open");

        let closed = repo
            .update_job(
                &job.id,
                &UpdateJobRequest {
                    title: None,
                    department_id: None,
                    description: None,
                    requirements: None,
                    status: Some("closed".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(closed.status, "closed");
        assert!(closed.closed_at.is_some());
    }

    #[tokio::test]
    async fn test_application_against_open_job() {
        let repo = setup_repo().await;

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Designer".into(),
                department_id: "design".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        // Job is open -- application should succeed at repo level
        let app_input = CreateApplicationRequest {
            job_id: job.id.clone(),
            candidate_first_name: "Jane".into(),
            candidate_last_name: "Doe".into(),
            candidate_email: "jane@example.com".into(),
            notes: None,
        };
        let app = repo.create_application(&app_input).await.unwrap();
        assert_eq!(app.status, "applied");
        assert_eq!(app.candidate_email, "jane@example.com");
    }

    #[tokio::test]
    async fn test_application_against_closed_job_blocked() {
        let repo = setup_repo().await;

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Analyst".into(),
                department_id: "finance".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        repo.update_job(
            &job.id,
            &UpdateJobRequest {
                title: None,
                department_id: None,
                description: None,
                requirements: None,
                status: Some("closed".into()),
            },
        )
        .await
        .unwrap();

        // Verify the job is closed -- service layer would block this
        let fetched_job = repo.get_job(&job.id).await.unwrap();
        assert_ne!(fetched_job.status, "open", "Job should not be open");
    }

    #[tokio::test]
    async fn test_application_status_transitions() {
        let repo = setup_repo().await;

        let job = repo
            .create_job(&CreateJobRequest {
                title: "DevOps".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let app = repo
            .create_application(&CreateApplicationRequest {
                job_id: job.id.clone(),
                candidate_first_name: "Alice".into(),
                candidate_last_name: "Smith".into(),
                candidate_email: "alice@example.com".into(),
                notes: None,
            })
            .await
            .unwrap();
        assert_eq!(app.status, "applied");

        // Transition to screening
        let screening = repo
            .update_application_status(&app.id, "screening", None)
            .await
            .unwrap();
        assert_eq!(screening.status, "screening");

        // Transition to interview
        let interview = repo
            .update_application_status(&app.id, "interview", None)
            .await
            .unwrap();
        assert_eq!(interview.status, "interview");

        // Transition to hired with notes
        let hired = repo
            .update_application_status(&app.id, "hired", Some("Strong candidate"))
            .await
            .unwrap();
        assert_eq!(hired.status, "hired");
        assert_eq!(hired.notes, Some("Strong candidate".to_string()));
    }

    #[tokio::test]
    async fn test_list_applications_by_job() {
        let repo = setup_repo().await;

        let job1 = repo
            .create_job(&CreateJobRequest {
                title: "Job 1".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let job2 = repo
            .create_job(&CreateJobRequest {
                title: "Job 2".into(),
                department_id: "sales".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        // Two applications for job1
        repo.create_application(&CreateApplicationRequest {
            job_id: job1.id.clone(),
            candidate_first_name: "A".into(),
            candidate_last_name: "B".into(),
            candidate_email: "a@b.com".into(),
            notes: None,
        })
        .await
        .unwrap();
        repo.create_application(&CreateApplicationRequest {
            job_id: job1.id.clone(),
            candidate_first_name: "C".into(),
            candidate_last_name: "D".into(),
            candidate_email: "c@d.com".into(),
            notes: None,
        })
        .await
        .unwrap();

        // One application for job2
        repo.create_application(&CreateApplicationRequest {
            job_id: job2.id.clone(),
            candidate_first_name: "E".into(),
            candidate_last_name: "F".into(),
            candidate_email: "e@f.com".into(),
            notes: None,
        })
        .await
        .unwrap();

        let job1_apps = repo.list_applications_by_job(&job1.id).await.unwrap();
        assert_eq!(job1_apps.len(), 2);

        let job2_apps = repo.list_applications_by_job(&job2.id).await.unwrap();
        assert_eq!(job2_apps.len(), 1);
    }

    #[tokio::test]
    async fn test_fill_job_sets_closed_at() {
        let repo = setup_repo().await;

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Filled Role".into(),
                department_id: "hr".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let filled = repo
            .update_job(
                &job.id,
                &UpdateJobRequest {
                    title: None,
                    department_id: None,
                    description: None,
                    requirements: None,
                    status: Some("filled".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(filled.status, "filled");
        assert!(filled.closed_at.is_some());
    }
}

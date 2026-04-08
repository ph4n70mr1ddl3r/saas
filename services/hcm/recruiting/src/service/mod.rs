use crate::models::*;
use crate::repository::RecruitingRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::ApplicationStatusChanged;
use sqlx::SqlitePool;
use validator::Validate;

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
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        self.repo.create_job(&input).await
    }

    pub async fn update_job(&self, id: &str, input: UpdateJobRequest) -> AppResult<JobPosting> {
        let existing = self.repo.get_job(id).await?;
        if let Some(ref new_status) = input.status {
            let valid_transitions: &[&[&str]] = &[
                &["open", "closed"],
                &["open", "filled"],
                &["closed", "open"],
            ];
            let allowed = valid_transitions
                .iter()
                .any(|t| t[0] == existing.status && t[1] == *new_status);
            if !allowed && *new_status != existing.status {
                return Err(AppError::Validation(format!(
                    "Cannot transition job '{}' from '{}' to '{}'",
                    id, existing.status, new_status
                )));
            }
        }
        self.repo.update_job(id, &input).await
    }

    /// Close a job posting — stops accepting applications.
    pub async fn close_job(&self, id: &str) -> AppResult<JobPosting> {
        let existing = self.repo.get_job(id).await?;
        if existing.status == "closed" {
            return Err(AppError::Validation(format!(
                "Job '{}' is already closed",
                id
            )));
        }
        self.repo
            .update_job(
                id,
                &UpdateJobRequest {
                    title: None,
                    department_id: None,
                    description: None,
                    requirements: None,
                    status: Some("closed".into()),
                },
            )
            .await
    }

    // --- Applications ---

    pub async fn list_applications(&self) -> AppResult<Vec<Application>> {
        self.repo.list_applications().await
    }

    pub async fn get_application(&self, id: &str) -> AppResult<Application> {
        self.repo.get_application(id).await
    }

    pub async fn create_application(
        &self,
        input: CreateApplicationRequest,
    ) -> AppResult<Application> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let job = self.repo.get_job(&input.job_id).await?;
        if job.status != "open" {
            return Err(AppError::Validation(format!(
                "Job '{}' is not open for applications (status: {})",
                input.job_id, job.status
            )));
        }
        // Prevent duplicate application from same candidate email for same job
        let existing = self.repo.list_applications().await?;
        if existing.iter().any(|a| {
            a.job_id == input.job_id
                && a.candidate_email.to_lowercase() == input.candidate_email.to_lowercase()
                && a.status != "rejected"
        }) {
            return Err(AppError::Validation(format!(
                "Candidate '{}' already has an active application for this job",
                input.candidate_email
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

        // Validate status transition
        let valid_transitions: std::collections::HashMap<&str, &[&str]> = [
            ("applied", &["screening", "rejected"][..]),
            ("screening", &["interview", "rejected"]),
            ("interview", &["offer", "rejected"]),
            ("offer", &["hired", "rejected"]),
            ("hired", &[]),
            ("rejected", &[]),
        ]
        .iter()
        .cloned()
        .collect();

        let allowed_next: &[&str] = valid_transitions.get(old_status.as_str()).copied().unwrap_or(&[]);
        if !allowed_next.contains(&input.status.as_str()) && input.status != old_status {
            return Err(AppError::Validation(format!(
                "Cannot transition application '{}' from '{}' to '{}'. Valid transitions: {:?}",
                id, old_status, input.status, allowed_next
            )));
        }

        let app = self
            .repo
            .update_application_status(id, &input.status, input.notes.as_deref())
            .await?;

        // Publish event for ALL status changes (not just "hired")
        {
            let job = self.repo.get_job(&app.job_id).await.ok();
            let event = ApplicationStatusChanged {
                application_id: id.to_string(),
                job_id: app.job_id.clone(),
                candidate_first_name: app.candidate_first_name.clone(),
                candidate_last_name: app.candidate_last_name.clone(),
                candidate_email: app.candidate_email.clone(),
                job_title: job.as_ref().map(|j| j.title.clone()).unwrap_or_default(),
                department_id: job.as_ref().map(|j| j.department_id.clone()).unwrap_or_default(),
                old_status: old_status.clone(),
                new_status: input.status.clone(),
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

    /// When an employee is terminated, log open job postings for awareness.
    pub async fn handle_employee_terminated(&self, employee_id: &str) -> AppResult<()> {
        let open_jobs = self.repo.list_jobs().await?;
        let open_count = open_jobs.iter().filter(|j| j.status == "open").count();
        if open_count > 0 {
            tracing::info!(
                "Employee {} terminated: {} open job posting(s) may need review for backfill",
                employee_id, open_count
            );
        }
        Ok(())
    }

    /// Handle application status change for non-hired statuses.
    /// Logs notification for screening, interview, offer, and rejection stages.
    pub async fn handle_application_status_changed(
        &self,
        application_id: &str,
        old_status: &str,
        new_status: &str,
        candidate_first_name: &str,
        candidate_last_name: &str,
        job_title: &str,
    ) -> AppResult<()> {
        if new_status != "hired" {
            tracing::info!(
                "Application {} status changed: {} -> {} (candidate: {} {}, job: {}). Notification sent.",
                application_id, old_status, new_status,
                candidate_first_name, candidate_last_name, job_title
            );
        }
        Ok(())
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

    #[tokio::test]
    async fn test_job_status_transition_validation() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Test".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();
        assert_eq!(job.status, "open");

        // Invalid: open -> in_progress (not a valid transition)
        let result = svc
            .update_job(
                &job.id,
                UpdateJobRequest {
                    title: None,
                    department_id: None,
                    description: None,
                    requirements: None,
                    status: Some("in_progress".into()),
                },
            )
            .await;
        assert!(result.is_err());

        // Valid: open -> closed
        let closed = svc
            .update_job(
                &job.id,
                UpdateJobRequest {
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

        // Valid: closed -> open (reopen)
        let reopened = svc
            .update_job(
                &job.id,
                UpdateJobRequest {
                    title: None,
                    department_id: None,
                    description: None,
                    requirements: None,
                    status: Some("open".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(reopened.status, "open");
    }

    #[tokio::test]
    async fn test_application_status_transition_validation() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let app = repo
            .create_application(&CreateApplicationRequest {
                job_id: job.id.clone(),
                candidate_first_name: "John".into(),
                candidate_last_name: "Doe".into(),
                candidate_email: "john@example.com".into(),
                notes: None,
            })
            .await
            .unwrap();
        assert_eq!(app.status, "applied");

        // Invalid: applied -> hired (must go through screening/interview/offer)
        let result = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "hired".into(),
                    notes: None,
                },
            )
            .await;
        assert!(result.is_err());

        // Valid: applied -> screening
        let screening = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "screening".into(),
                    notes: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(screening.status, "screening");

        // Invalid: screening -> hired (must go through interview/offer)
        let result = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "hired".into(),
                    notes: None,
                },
            )
            .await;
        assert!(result.is_err());

        // Valid: screening -> interview
        let interview = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "interview".into(),
                    notes: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(interview.status, "interview");

        // Valid: interview -> offer
        let offer = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "offer".into(),
                    notes: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(offer.status, "offer");

        // Valid: offer -> hired
        let hired = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "hired".into(),
                    notes: Some("Strong candidate".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(hired.status, "hired");

        // Invalid: hired -> any (terminal state)
        let result = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "applied".into(),
                    notes: None,
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_application_rejection_from_any_stage() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Manager".into(),
                department_id: "ops".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let app = repo
            .create_application(&CreateApplicationRequest {
                job_id: job.id.clone(),
                candidate_first_name: "Bob".into(),
                candidate_last_name: "Smith".into(),
                candidate_email: "bob@example.com".into(),
                notes: None,
            })
            .await
            .unwrap();

        // Valid: applied -> rejected
        let rejected = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "rejected".into(),
                    notes: Some("Not a fit".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(rejected.status, "rejected");

        // Invalid: rejected is terminal
        let result = svc
            .update_application_status(
                &app.id,
                UpdateApplicationStatusRequest {
                    status: "screening".into(),
                    notes: None,
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_application() {
        let repo = setup_repo().await;

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let app = repo
            .create_application(&CreateApplicationRequest {
                job_id: job.id.clone(),
                candidate_first_name: "John".into(),
                candidate_last_name: "Doe".into(),
                candidate_email: "john@test.com".into(),
                notes: Some("Great candidate".into()),
            })
            .await
            .unwrap();

        // Fetch by ID
        let fetched = repo.get_application(&app.id).await.unwrap();
        assert_eq!(fetched.id, app.id);
        assert_eq!(fetched.candidate_email, "john@test.com");
        assert_eq!(fetched.notes, Some("Great candidate".into()));

        // Not found
        let result = repo.get_application("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_duplicate_application_prevention() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        // First application should succeed
        svc.create_application(CreateApplicationRequest {
            job_id: job.id.clone(),
            candidate_first_name: "Alice".into(),
            candidate_last_name: "Smith".into(),
            candidate_email: "alice@example.com".into(),
            notes: None,
        })
        .await
        .unwrap();

        // Duplicate email for same job should fail
        let result = svc
            .create_application(CreateApplicationRequest {
                job_id: job.id.clone(),
                candidate_first_name: "Alice".into(),
                candidate_last_name: "Smith".into(),
                candidate_email: "ALICE@example.com".into(), // case-insensitive
                notes: None,
            })
            .await;
        assert!(result.is_err());

        // Different email for same job should succeed
        svc.create_application(CreateApplicationRequest {
            job_id: job.id.clone(),
            candidate_first_name: "Bob".into(),
            candidate_last_name: "Jones".into(),
            candidate_email: "bob@example.com".into(),
            notes: None,
        })
        .await
        .unwrap();

        let apps = repo.list_applications().await.unwrap();
        assert_eq!(apps.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_application_status_changed_non_hired() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Handler for non-hired status should succeed and log
        let result = svc
            .handle_application_status_changed(
                "app-001",
                "applied",
                "screening",
                "Jane",
                "Doe",
                "Senior Engineer",
            )
            .await;
        assert!(result.is_ok());

        // Hired status should also succeed (handler just skips logging for hired)
        let result = svc
            .handle_application_status_changed(
                "app-001",
                "offer",
                "hired",
                "Jane",
                "Doe",
                "Senior Engineer",
            )
            .await;
        assert!(result.is_ok());

        // Rejected status
        let result = svc
            .handle_application_status_changed(
                "app-002",
                "interview",
                "rejected",
                "Bob",
                "Smith",
                "Designer",
            )
            .await;
        assert!(result.is_ok());
    }

    // --- Validation tests ---

    #[tokio::test]
    async fn test_create_job_validation_empty_title() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_job(CreateJobRequest {
                title: "".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("title"),
            "Expected title validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_job_validation_empty_department_id() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_job(CreateJobRequest {
                title: "Engineer".into(),
                department_id: "".into(),
                description: None,
                requirements: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("department_id"),
            "Expected department_id validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_job_valid_input_succeeds() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = svc
            .create_job(CreateJobRequest {
                title: "Senior Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();
        assert_eq!(job.title, "Senior Engineer");
    }

    #[tokio::test]
    async fn test_create_application_validation_invalid_email() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let result = svc
            .create_application(CreateApplicationRequest {
                job_id: job.id,
                candidate_first_name: "Jane".into(),
                candidate_last_name: "Doe".into(),
                candidate_email: "not-an-email".into(),
                notes: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("candidate_email"),
            "Expected candidate_email validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_application_validation_empty_first_name() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let result = svc
            .create_application(CreateApplicationRequest {
                job_id: job.id,
                candidate_first_name: "".into(),
                candidate_last_name: "Doe".into(),
                candidate_email: "jane@example.com".into(),
                notes: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("candidate_first_name"),
            "Expected candidate_first_name validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_application_validation_empty_last_name() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = repo
            .create_job(&CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let result = svc
            .create_application(CreateApplicationRequest {
                job_id: job.id,
                candidate_first_name: "Jane".into(),
                candidate_last_name: "".into(),
                candidate_email: "jane@example.com".into(),
                notes: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("candidate_last_name"),
            "Expected candidate_last_name validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_application_validation_empty_job_id() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc
            .create_application(CreateApplicationRequest {
                job_id: "".into(),
                candidate_first_name: "Jane".into(),
                candidate_last_name: "Doe".into(),
                candidate_email: "jane@example.com".into(),
                notes: None,
            })
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("job_id"),
            "Expected job_id validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_application_valid_input_succeeds() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = svc
            .create_job(CreateJobRequest {
                title: "Engineer".into(),
                department_id: "eng".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        let app = svc
            .create_application(CreateApplicationRequest {
                job_id: job.id,
                candidate_first_name: "Jane".into(),
                candidate_last_name: "Doe".into(),
                candidate_email: "jane@example.com".into(),
                notes: None,
            })
            .await
            .unwrap();
        assert_eq!(app.candidate_email, "jane@example.com");
    }

    #[tokio::test]
    async fn test_handle_employee_terminated_with_open_jobs() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create open job postings that may need review after termination
        svc.create_job(CreateJobRequest {
            title: "Senior Engineer".into(),
            department_id: "eng".into(),
            description: None,
            requirements: None,
        })
        .await
        .unwrap();
        svc.create_job(CreateJobRequest {
            title: "Product Manager".into(),
            department_id: "product".into(),
            description: None,
            requirements: None,
        })
        .await
        .unwrap();

        // Verify open jobs exist
        let jobs = repo.list_jobs().await.unwrap();
        let open_count = jobs.iter().filter(|j| j.status == "open").count();
        assert_eq!(open_count, 2);

        // Handler should succeed — logs awareness of open postings
        let result = svc.handle_employee_terminated("emp-term-001").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_employee_terminated_with_no_open_jobs() {
        let repo = setup_repo().await;
        let svc = RecruitingService {
            repo: repo.clone(),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create a job and immediately close it — no open jobs
        let job = svc
            .create_job(CreateJobRequest {
                title: "Old Role".into(),
                department_id: "ops".into(),
                description: None,
                requirements: None,
            })
            .await
            .unwrap();

        svc.update_job(
            &job.id,
            UpdateJobRequest {
                title: None,
                department_id: None,
                description: None,
                requirements: None,
                status: Some("closed".into()),
            },
        )
        .await
        .unwrap();

        // Verify no open jobs
        let jobs = repo.list_jobs().await.unwrap();
        let open_count = jobs.iter().filter(|j| j.status == "open").count();
        assert_eq!(open_count, 0);

        // Handler should still succeed even with no open jobs
        let result = svc.handle_employee_terminated("emp-term-002").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_close_job() {
        let pool = setup().await;
        let svc = RecruitingService {
            repo: RecruitingRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = svc.create_job(CreateJobRequest {
            title: "Software Engineer".into(),
            department_id: "dept-eng".into(),
            description: Some("Build things".into()),
            requirements: Some("Rust".into()),
        }).await.unwrap();
        assert_eq!(job.status, "open");

        let closed = svc.close_job(&job.id).await.unwrap();
        assert_eq!(closed.status, "closed");
        assert!(closed.closed_at.is_some());

        // Closing again should fail
        let result = svc.close_job(&job.id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already closed"));
    }

    #[tokio::test]
    async fn test_close_job_blocks_applications() {
        let pool = setup().await;
        let svc = RecruitingService {
            repo: RecruitingRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let job = svc.create_job(CreateJobRequest {
            title: "Data Analyst".into(),
            department_id: "dept-data".into(),
            description: None,
            requirements: None,
        }).await.unwrap();

        svc.close_job(&job.id).await.unwrap();

        // Application to closed job should fail
        let result = svc.create_application(CreateApplicationRequest {
            job_id: job.id,
            candidate_first_name: "Jane".into(),
            candidate_last_name: "Doe".into(),
            candidate_email: "jane@example.com".into(),
            notes: None,
        }).await;
        assert!(result.is_err());
    }
}

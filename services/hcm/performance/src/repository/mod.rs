use crate::models::*;
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct PerformanceRepo {
    pool: SqlitePool,
}

impl PerformanceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Review Cycles ---

    pub async fn list_review_cycles(&self) -> AppResult<Vec<ReviewCycle>> {
        let rows = sqlx::query_as::<_, ReviewCycle>(
            "SELECT id, name, description, start_date, end_date, status, created_at FROM review_cycles ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_review_cycle(&self, id: &str) -> AppResult<ReviewCycle> {
        sqlx::query_as::<_, ReviewCycle>(
            "SELECT id, name, description, start_date, end_date, status, created_at FROM review_cycles WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Review cycle '{}' not found", id)))
    }

    pub async fn create_review_cycle(
        &self,
        input: &CreateReviewCycleRequest,
    ) -> AppResult<ReviewCycle> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO review_cycles (id, name, description, start_date, end_date, status) VALUES (?, ?, ?, ?, ?, 'draft')"
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.start_date)
        .bind(&input.end_date)
        .execute(&self.pool)
        .await?;
        self.get_review_cycle(&id).await
    }

    pub async fn update_cycle_status(&self, id: &str, status: &str) -> AppResult<ReviewCycle> {
        sqlx::query("UPDATE review_cycles SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_review_cycle(id).await
    }

    // --- Goals ---

    pub async fn list_goals(&self) -> AppResult<Vec<Goal>> {
        let rows = sqlx::query_as::<_, Goal>(
            "SELECT id, employee_id, cycle_id, title, description, weight, progress, status, due_date, created_at FROM goals ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_goals_by_cycle(&self, cycle_id: &str) -> AppResult<Vec<Goal>> {
        let rows = sqlx::query_as::<_, Goal>(
            "SELECT id, employee_id, cycle_id, title, description, weight, progress, status, due_date, created_at FROM goals WHERE cycle_id = ? ORDER BY created_at DESC"
        )
        .bind(cycle_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_goal(&self, id: &str) -> AppResult<Goal> {
        sqlx::query_as::<_, Goal>(
            "SELECT id, employee_id, cycle_id, title, description, weight, progress, status, due_date, created_at FROM goals WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Goal '{}' not found", id)))
    }

    pub async fn create_goal(&self, input: &CreateGoalRequest) -> AppResult<Goal> {
        let id = uuid::Uuid::new_v4().to_string();
        let weight = input.weight.unwrap_or(1.0);
        let progress = input.progress.unwrap_or(0.0);
        sqlx::query(
            "INSERT INTO goals (id, employee_id, cycle_id, title, description, weight, progress, status, due_date) VALUES (?, ?, ?, ?, ?, ?, ?, 'not_started', ?)"
        )
        .bind(&id)
        .bind(&input.employee_id)
        .bind(&input.cycle_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(weight)
        .bind(progress)
        .bind(&input.due_date)
        .execute(&self.pool)
        .await?;
        self.get_goal(&id).await
    }

    pub async fn update_goal(&self, id: &str, input: &UpdateGoalRequest) -> AppResult<Goal> {
        let existing = self.get_goal(id).await?;
        let title = input.title.as_deref().unwrap_or(&existing.title);
        let description = input.description.as_ref().or(existing.description.as_ref());
        let weight = input.weight.unwrap_or(existing.weight);
        let progress = input.progress.unwrap_or(existing.progress);
        let status = input.status.as_deref().unwrap_or(&existing.status);
        let due_date = input.due_date.as_ref().or(existing.due_date.as_ref());

        sqlx::query(
            "UPDATE goals SET title = ?, description = ?, weight = ?, progress = ?, status = ?, due_date = ? WHERE id = ?"
        )
        .bind(title)
        .bind(description)
        .bind(weight)
        .bind(progress)
        .bind(status)
        .bind(due_date)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_goal(id).await
    }

    // --- Review Assignments ---

    pub async fn list_review_assignments(&self) -> AppResult<Vec<ReviewAssignment>> {
        let rows = sqlx::query_as::<_, ReviewAssignment>(
            "SELECT id, cycle_id, reviewer_id, employee_id, status, rating, comments, submitted_at, created_at FROM review_assignments ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_assignments_by_cycle(&self, cycle_id: &str) -> AppResult<Vec<ReviewAssignment>> {
        let rows = sqlx::query_as::<_, ReviewAssignment>(
            "SELECT id, cycle_id, reviewer_id, employee_id, status, rating, comments, submitted_at, created_at FROM review_assignments WHERE cycle_id = ? ORDER BY created_at DESC"
        )
        .bind(cycle_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_review_assignment(&self, id: &str) -> AppResult<ReviewAssignment> {
        sqlx::query_as::<_, ReviewAssignment>(
            "SELECT id, cycle_id, reviewer_id, employee_id, status, rating, comments, submitted_at, created_at FROM review_assignments WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Review assignment '{}' not found", id)))
    }

    pub async fn create_review_assignment(
        &self,
        input: &CreateReviewAssignmentRequest,
    ) -> AppResult<ReviewAssignment> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO review_assignments (id, cycle_id, reviewer_id, employee_id, status) VALUES (?, ?, ?, ?, 'pending')"
        )
        .bind(&id)
        .bind(&input.cycle_id)
        .bind(&input.reviewer_id)
        .bind(&input.employee_id)
        .execute(&self.pool)
        .await?;
        self.get_review_assignment(&id).await
    }

    pub async fn submit_review_assignment(
        &self,
        id: &str,
        rating: i32,
        comments: Option<&str>,
    ) -> AppResult<ReviewAssignment> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE review_assignments SET status = 'completed', rating = ?, comments = ?, submitted_at = ? WHERE id = ?"
        )
        .bind(rating)
        .bind(comments)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_review_assignment(id).await
    }

    // --- Feedback ---

    pub async fn list_feedback(&self) -> AppResult<Vec<Feedback>> {
        let rows = sqlx::query_as::<_, Feedback>(
            "SELECT id, cycle_id, from_employee_id, to_employee_id, content, is_anonymous, created_at FROM feedback ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_feedback(&self, id: &str) -> AppResult<Feedback> {
        sqlx::query_as::<_, Feedback>(
            "SELECT id, cycle_id, from_employee_id, to_employee_id, content, is_anonymous, created_at FROM feedback WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Feedback '{}' not found", id)))
    }

    pub async fn create_feedback(&self, input: &CreateFeedbackRequest) -> AppResult<Feedback> {
        let id = uuid::Uuid::new_v4().to_string();
        let is_anonymous = input.is_anonymous.unwrap_or(false);
        let is_anonymous_int: i32 = if is_anonymous { 1 } else { 0 };
        sqlx::query(
            "INSERT INTO feedback (id, cycle_id, from_employee_id, to_employee_id, content, is_anonymous) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&input.cycle_id)
        .bind(&input.from_employee_id)
        .bind(&input.to_employee_id)
        .bind(&input.content)
        .bind(is_anonymous_int)
        .execute(&self.pool)
        .await?;
        self.get_feedback(&id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_review_cycles.sql"),
            include_str!("../../migrations/002_create_goals.sql"),
            include_str!("../../migrations/003_create_review_assignments.sql"),
            include_str!("../../migrations/004_create_feedback.sql"),
        ];
        let migration_names = [
            "001_create_review_cycles.sql",
            "002_create_goals.sql",
            "003_create_review_assignments.sql",
            "004_create_feedback.sql",
        ];
        // Ensure the tracking table exists
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

    #[tokio::test]
    async fn test_create_and_get_cycle() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: Some("Quarterly review".into()),
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };

        let cycle = repo.create_review_cycle(&input).await.unwrap();
        assert_eq!(cycle.name, "Q1 Review");
        assert_eq!(cycle.status, "draft");

        let fetched = repo.get_review_cycle(&cycle.id).await.unwrap();
        assert_eq!(fetched.id, cycle.id);
        assert_eq!(fetched.name, "Q1 Review");
    }

    #[tokio::test]
    async fn test_list_review_cycles() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let input1 = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let input2 = CreateReviewCycleRequest {
            name: "Q2 Review".into(),
            description: None,
            start_date: "2025-04-01".into(),
            end_date: "2025-06-30".into(),
        };

        repo.create_review_cycle(&input1).await.unwrap();
        repo.create_review_cycle(&input2).await.unwrap();

        let list = repo.list_review_cycles().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_update_cycle_status() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };

        let cycle = repo.create_review_cycle(&input).await.unwrap();
        assert_eq!(cycle.status, "draft");

        let updated = repo.update_cycle_status(&cycle.id, "active").await.unwrap();
        assert_eq!(updated.status, "active");

        let closed = repo.update_cycle_status(&cycle.id, "closed").await.unwrap();
        assert_eq!(closed.status, "closed");
    }

    #[tokio::test]
    async fn test_create_and_get_goal() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        let goal_input = CreateGoalRequest {
            employee_id: "emp-001".into(),
            cycle_id: cycle.id.clone(),
            title: "Improve code quality".into(),
            description: Some("Reduce bug count".into()),
            weight: Some(2.5),
            progress: Some(10.0),
            due_date: Some("2025-03-15".into()),
        };

        let goal = repo.create_goal(&goal_input).await.unwrap();
        assert_eq!(goal.title, "Improve code quality");
        assert_eq!(goal.weight, 2.5);
        assert_eq!(goal.status, "not_started");

        let fetched = repo.get_goal(&goal.id).await.unwrap();
        assert_eq!(fetched.id, goal.id);
    }

    #[tokio::test]
    async fn test_update_goal() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        let goal_input = CreateGoalRequest {
            employee_id: "emp-001".into(),
            cycle_id: cycle.id.clone(),
            title: "Learn Rust".into(),
            description: None,
            weight: None,
            progress: None,
            due_date: None,
        };

        let goal = repo.create_goal(&goal_input).await.unwrap();
        assert_eq!(goal.weight, 1.0);
        assert_eq!(goal.progress, 0.0);

        let update = UpdateGoalRequest {
            title: Some("Learn Rust Advanced".into()),
            description: Some("Deep dive".into()),
            weight: Some(3.0),
            progress: Some(50.0),
            status: Some("in_progress".into()),
            due_date: None,
        };

        let updated = repo.update_goal(&goal.id, &update).await.unwrap();
        assert_eq!(updated.title, "Learn Rust Advanced");
        assert_eq!(updated.weight, 3.0);
        assert_eq!(updated.progress, 50.0);
        assert_eq!(updated.status, "in_progress");
    }

    #[tokio::test]
    async fn test_create_and_submit_review_assignment() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        let assignment_input = CreateReviewAssignmentRequest {
            cycle_id: cycle.id.clone(),
            reviewer_id: "reviewer-001".into(),
            employee_id: "emp-001".into(),
        };

        let assignment = repo
            .create_review_assignment(&assignment_input)
            .await
            .unwrap();
        assert_eq!(assignment.status, "pending");
        assert!(assignment.rating.is_none());

        let submitted = repo
            .submit_review_assignment(&assignment.id, 4, Some("Great work"))
            .await
            .unwrap();
        assert_eq!(submitted.status, "completed");
        assert_eq!(submitted.rating, Some(4));
        assert_eq!(submitted.comments, Some("Great work".into()));
        assert!(submitted.submitted_at.is_some());
    }

    #[tokio::test]
    async fn test_create_feedback_named() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        let feedback_input = CreateFeedbackRequest {
            cycle_id: cycle.id.clone(),
            from_employee_id: "emp-001".into(),
            to_employee_id: "emp-002".into(),
            content: "Excellent collaboration".into(),
            is_anonymous: Some(false),
        };

        let feedback = repo.create_feedback(&feedback_input).await.unwrap();
        assert_eq!(feedback.content, "Excellent collaboration");
        assert!(!feedback.is_anonymous);
    }

    #[tokio::test]
    async fn test_create_feedback_anonymous() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        let feedback_input = CreateFeedbackRequest {
            cycle_id: cycle.id.clone(),
            from_employee_id: "emp-001".into(),
            to_employee_id: "emp-002".into(),
            content: "Could improve communication".into(),
            is_anonymous: Some(true),
        };

        let feedback = repo.create_feedback(&feedback_input).await.unwrap();
        assert!(feedback.is_anonymous);
    }

    #[tokio::test]
    async fn test_list_goals() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        for i in 0..3 {
            let goal_input = CreateGoalRequest {
                employee_id: format!("emp-00{}", i + 1),
                cycle_id: cycle.id.clone(),
                title: format!("Goal {}", i + 1),
                description: None,
                weight: None,
                progress: None,
                due_date: None,
            };
            repo.create_goal(&goal_input).await.unwrap();
        }

        let goals = repo.list_goals().await.unwrap();
        assert_eq!(goals.len(), 3);
    }

    #[tokio::test]
    async fn test_get_cycle_not_found() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let result = repo.get_review_cycle("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_review_assignments() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        for i in 0..2 {
            let assignment_input = CreateReviewAssignmentRequest {
                cycle_id: cycle.id.clone(),
                reviewer_id: format!("reviewer-00{}", i + 1),
                employee_id: "emp-001".into(),
            };
            repo.create_review_assignment(&assignment_input)
                .await
                .unwrap();
        }

        let assignments = repo.list_review_assignments().await.unwrap();
        assert_eq!(assignments.len(), 2);
    }

    #[tokio::test]
    async fn test_list_feedback() {
        let pool = setup().await;
        let repo = PerformanceRepo::new(pool);

        let cycle_input = CreateReviewCycleRequest {
            name: "Q1 Review".into(),
            description: None,
            start_date: "2025-01-01".into(),
            end_date: "2025-03-31".into(),
        };
        let cycle = repo.create_review_cycle(&cycle_input).await.unwrap();

        for i in 0..2 {
            let feedback_input = CreateFeedbackRequest {
                cycle_id: cycle.id.clone(),
                from_employee_id: format!("emp-00{}", i + 1),
                to_employee_id: "emp-003".into(),
                content: format!("Feedback {}", i + 1),
                is_anonymous: None,
            };
            repo.create_feedback(&feedback_input).await.unwrap();
        }

        let feedback_list = repo.list_feedback().await.unwrap();
        assert_eq!(feedback_list.len(), 2);
    }
}

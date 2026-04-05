use crate::models::cycle_count::{CycleCountLine, CycleCountSession};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct CycleCountRepo {
    pool: SqlitePool,
}

impl CycleCountRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_cycle_count_session(
        &self,
        warehouse_id: &str,
        count_date: &str,
        counted_by: &str,
    ) -> AppResult<CycleCountSession> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO cycle_count_sessions (id, warehouse_id, status, count_date, counted_by) VALUES (?, ?, 'draft', ?, ?)"
        )
        .bind(&id).bind(warehouse_id).bind(count_date).bind(counted_by)
        .execute(&self.pool).await?;
        self.get_cycle_count_session(&id).await
    }

    pub async fn get_cycle_count_session(&self, id: &str) -> AppResult<CycleCountSession> {
        sqlx::query_as::<_, CycleCountSession>(
            "SELECT id, warehouse_id, status, count_date, counted_by, approved_by, approved_at, created_at FROM cycle_count_sessions WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Cycle count session {} not found", id)))
    }

    pub async fn list_cycle_count_sessions(&self) -> AppResult<Vec<CycleCountSession>> {
        let rows = sqlx::query_as::<_, CycleCountSession>(
            "SELECT id, warehouse_id, status, count_date, counted_by, approved_by, approved_at, created_at FROM cycle_count_sessions ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn add_cycle_count_line(
        &self,
        session_id: &str,
        item_id: &str,
        system_quantity: i64,
        notes: Option<&str>,
    ) -> AppResult<CycleCountLine> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO cycle_count_lines (id, session_id, item_id, system_quantity, notes) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(session_id).bind(item_id).bind(system_quantity).bind(notes)
        .execute(&self.pool).await?;
        self.get_cycle_count_line_by_id(&id).await
    }

    pub async fn get_cycle_count_line_by_id(&self, id: &str) -> AppResult<CycleCountLine> {
        sqlx::query_as::<_, CycleCountLine>(
            "SELECT id, session_id, item_id, system_quantity, counted_quantity, variance, notes, created_at FROM cycle_count_lines WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Cycle count line {} not found", id)))
    }

    pub async fn update_counted_quantity(
        &self,
        line_id: &str,
        counted_quantity: i64,
        notes: Option<&str>,
    ) -> AppResult<CycleCountLine> {
        let line = self.get_cycle_count_line_by_id(line_id).await?;
        let variance = counted_quantity - line.system_quantity;
        sqlx::query(
            "UPDATE cycle_count_lines SET counted_quantity = ?, variance = ?, notes = ? WHERE id = ?"
        )
        .bind(counted_quantity).bind(variance).bind(notes).bind(line_id)
        .execute(&self.pool).await?;
        self.get_cycle_count_line_by_id(line_id).await
    }

    pub async fn get_cycle_count_lines(&self, session_id: &str) -> AppResult<Vec<CycleCountLine>> {
        let rows = sqlx::query_as::<_, CycleCountLine>(
            "SELECT id, session_id, item_id, system_quantity, counted_quantity, variance, notes, created_at FROM cycle_count_lines WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn update_session_status(
        &self,
        id: &str,
        status: &str,
    ) -> AppResult<CycleCountSession> {
        sqlx::query("UPDATE cycle_count_sessions SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get_cycle_count_session(id).await
    }

    pub async fn post_cycle_count(
        &self,
        session_id: &str,
        approved_by: &str,
    ) -> AppResult<CycleCountSession> {
        let pool = self.pool.clone();
        let mut tx = pool.begin().await?;

        // Mark session as posted with approver info
        sqlx::query(
            "UPDATE cycle_count_sessions SET status = 'posted', approved_by = ?, approved_at = datetime('now') WHERE id = ?"
        )
        .bind(approved_by).bind(session_id)
        .execute(&mut *tx).await?;

        // Get the session to find warehouse_id
        let session = sqlx::query_as::<_, CycleCountSession>(
            "SELECT id, warehouse_id, status, count_date, counted_by, approved_by, approved_at, created_at FROM cycle_count_sessions WHERE id = ?"
        )
        .bind(session_id)
        .fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::NotFound(format!("Cycle count session {} not found", session_id)))?;

        // Get all lines with non-zero variance
        let lines = sqlx::query_as::<_, CycleCountLine>(
            "SELECT id, session_id, item_id, system_quantity, counted_quantity, variance, notes, created_at FROM cycle_count_lines WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_all(&mut *tx).await?;

        for line in &lines {
            let variance = line.variance.unwrap_or(0);
            if variance != 0 {
                // Create a stock_movement for the adjustment
                let movement_id = uuid::Uuid::new_v4().to_string();
                let movement_type = if variance > 0 {
                    "adjustment"
                } else {
                    "adjustment"
                };
                sqlx::query(
                    "INSERT INTO stock_movements (id, item_id, from_warehouse_id, to_warehouse_id, quantity, movement_type, reference_type, reference_id) VALUES (?, ?, NULL, ?, ?, ?, 'cycle_count', ?)"
                )
                .bind(&movement_id)
                .bind(&line.item_id)
                .bind(&session.warehouse_id)
                .bind(variance.abs())
                .bind(movement_type)
                .bind(session_id)
                .execute(&mut *tx).await?;

                // Update stock_levels: adjust quantity_on_hand and quantity_available
                let stock_id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO stock_levels (id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available) VALUES (?, ?, ?, ?, 0, ?) ON CONFLICT(item_id, warehouse_id) DO UPDATE SET quantity_on_hand = quantity_on_hand + ?, quantity_available = quantity_on_hand + ? - quantity_reserved, updated_at = datetime('now')"
                )
                .bind(&stock_id)
                .bind(&line.item_id)
                .bind(&session.warehouse_id)
                .bind(variance)
                .bind(variance)
                .bind(variance)
                .bind(variance)
                .execute(&mut *tx).await?;
            }
        }

        tx.commit().await?;

        // Fetch the final session state
        self.get_cycle_count_session(session_id).await
    }
}

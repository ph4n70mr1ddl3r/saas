use crate::models::reservation::{CreateReservation, ReservationResponse};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ReservationRepo {
    pool: SqlitePool,
}

impl ReservationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<ReservationResponse>> {
        let rows = sqlx::query_as::<_, ReservationResponse>(
            "SELECT id, item_id, warehouse_id, quantity, reference_type, reference_id, status, created_at, fulfilled_at FROM reservations ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<ReservationResponse> {
        sqlx::query_as::<_, ReservationResponse>(
            "SELECT id, item_id, warehouse_id, quantity, reference_type, reference_id, status, created_at, fulfilled_at FROM reservations WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Reservation {} not found", id)))
    }

    pub async fn create(&self, input: &CreateReservation) -> AppResult<ReservationResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO reservations (id, item_id, warehouse_id, quantity, reference_type, reference_id) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.item_id).bind(&input.warehouse_id)
        .bind(input.quantity).bind(&input.reference_type).bind(&input.reference_id)
        .execute(&self.pool).await?;
        self.get_by_id(&id).await
    }

    pub async fn cancel(&self, id: &str) -> AppResult<ReservationResponse> {
        let reservation = self.get_by_id(id).await?;
        sqlx::query(
            "UPDATE reservations SET status = 'cancelled' WHERE id = ? AND status = 'active'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get_by_id(id).await
    }

    pub async fn get_active_by_reference(
        &self,
        reference_type: &str,
        reference_id: &str,
    ) -> AppResult<Vec<ReservationResponse>> {
        let rows = sqlx::query_as::<_, ReservationResponse>(
            "SELECT id, item_id, warehouse_id, quantity, reference_type, reference_id, status, created_at, fulfilled_at FROM reservations WHERE reference_type = ? AND reference_id = ? AND status = 'active'"
        )
        .bind(reference_type).bind(reference_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }
}

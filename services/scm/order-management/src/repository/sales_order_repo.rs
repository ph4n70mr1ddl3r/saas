use sqlx::SqlitePool;
use saas_common::error::{AppError, AppResult};
use crate::models::sales_order::{
    SalesOrderResponse, SalesOrderLineResponse, CreateSalesOrder,
};

#[derive(Clone)]
pub struct SalesOrderRepo { pool: SqlitePool }

impl SalesOrderRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn list(&self) -> AppResult<Vec<SalesOrderResponse>> {
        let rows = sqlx::query_as::<_, SalesOrderResponse>(
            "SELECT id, order_number, customer_id, order_date, status, total_cents, shipping_address, notes, created_at, updated_at FROM sales_orders ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<SalesOrderResponse> {
        sqlx::query_as::<_, SalesOrderResponse>(
            "SELECT id, order_number, customer_id, order_date, status, total_cents, shipping_address, notes, created_at, updated_at FROM sales_orders WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Sales order {} not found", id)))
    }

    pub async fn get_lines(&self, order_id: &str) -> AppResult<Vec<SalesOrderLineResponse>> {
        let rows = sqlx::query_as::<_, SalesOrderLineResponse>(
            "SELECT id, order_id, line_number, item_id, quantity, unit_price_cents, line_total_cents, status FROM order_lines WHERE order_id = ? ORDER BY line_number"
        )
        .bind(order_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn create(&self, input: &CreateSalesOrder) -> AppResult<SalesOrderResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let order_number = format!("SO-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
        let total_cents: i64 = input.lines.iter().map(|l| l.quantity * l.unit_price_cents).sum();
        let now = chrono::Utc::now().to_rfc3339();

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO sales_orders (id, order_number, customer_id, order_date, status, total_cents, shipping_address, notes, created_at, updated_at) VALUES (?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&order_number).bind(&input.customer_id).bind(&input.order_date)
        .bind(total_cents).bind(&input.shipping_address).bind(&input.notes)
        .bind(&now).bind(&now)
        .execute(&mut *tx).await?;

        for (i, line) in input.lines.iter().enumerate() {
            let line_id = uuid::Uuid::new_v4().to_string();
            let line_number = (i + 1) as i32;
            let line_total = line.quantity * line.unit_price_cents;
            sqlx::query(
                "INSERT INTO order_lines (id, order_id, line_number, item_id, quantity, unit_price_cents, line_total_cents, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'open')"
            )
            .bind(&line_id).bind(&id).bind(line_number).bind(&line.item_id)
            .bind(line.quantity).bind(line.unit_price_cents).bind(line_total)
            .execute(&mut *tx).await?;
        }

        tx.commit().await?;
        self.get_by_id(&id).await
    }

    pub async fn update_status(&self, id: &str, status: &str) -> AppResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query("UPDATE sales_orders SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status).bind(&now).bind(id)
            .execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Sales order {} not found", id)));
        }
        Ok(())
    }
}

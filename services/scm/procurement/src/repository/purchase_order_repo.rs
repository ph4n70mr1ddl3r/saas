use crate::models::purchase_order::{
    CreatePurchaseOrder, CreatePurchaseOrderLine, PurchaseOrderLineResponse, PurchaseOrderResponse,
};
use saas_common::error::{AppError, AppResult};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct PurchaseOrderRepo {
    pool: SqlitePool,
}

impl PurchaseOrderRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> AppResult<Vec<PurchaseOrderResponse>> {
        let rows = sqlx::query_as::<_, PurchaseOrderResponse>(
            "SELECT id, po_number, supplier_id, order_date, status, total_cents, created_at FROM purchase_orders ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        sqlx::query_as::<_, PurchaseOrderResponse>(
            "SELECT id, po_number, supplier_id, order_date, status, total_cents, created_at FROM purchase_orders WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Purchase order {} not found", id)))
    }

    pub async fn get_lines(&self, po_id: &str) -> AppResult<Vec<PurchaseOrderLineResponse>> {
        let rows = sqlx::query_as::<_, PurchaseOrderLineResponse>(
            "SELECT id, po_id, line_number, item_id, quantity, unit_price_cents, line_total_cents, quantity_received FROM po_lines WHERE po_id = ? ORDER BY line_number"
        )
        .bind(po_id)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn create(&self, input: &CreatePurchaseOrder) -> AppResult<PurchaseOrderResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let po_number = format!("PO-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
        let total_cents: i64 = input
            .lines
            .iter()
            .map(|l| l.quantity * l.unit_price_cents)
            .sum();

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO purchase_orders (id, po_number, supplier_id, order_date, status, total_cents) VALUES (?, ?, ?, ?, 'draft', ?)"
        )
        .bind(&id).bind(&po_number).bind(&input.supplier_id).bind(&input.order_date).bind(total_cents)
        .execute(&mut *tx).await?;

        for (i, line) in input.lines.iter().enumerate() {
            let line_id = uuid::Uuid::new_v4().to_string();
            let line_number = (i + 1) as i32;
            let line_total = line.quantity * line.unit_price_cents;
            sqlx::query(
                "INSERT INTO po_lines (id, po_id, line_number, item_id, quantity, unit_price_cents, line_total_cents) VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&line_id).bind(&id).bind(line_number).bind(&line.item_id)
            .bind(line.quantity).bind(line.unit_price_cents).bind(line_total)
            .execute(&mut *tx).await?;
        }

        tx.commit().await?;
        self.get_by_id(&id).await
    }

    pub async fn update_status(&self, id: &str, status: &str) -> AppResult<()> {
        let result = sqlx::query("UPDATE purchase_orders SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Purchase order {} not found",
                id
            )));
        }
        Ok(())
    }

    pub async fn update_line_received(&self, line_id: &str, quantity: i64) -> AppResult<()> {
        sqlx::query("UPDATE po_lines SET quantity_received = quantity_received + ? WHERE id = ?")
            .bind(quantity)
            .bind(line_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

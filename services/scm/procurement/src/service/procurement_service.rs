use crate::models::purchase_order::*;
use crate::models::supplier::*;
use crate::repository::purchase_order_repo::PurchaseOrderRepo;
use crate::repository::supplier_repo::SupplierRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct ProcurementService {
    pool: SqlitePool,
    supplier_repo: SupplierRepo,
    po_repo: PurchaseOrderRepo,
    bus: NatsBus,
}

impl ProcurementService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            pool: pool.clone(),
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool),
            bus,
        }
    }

    // Suppliers
    pub async fn list_suppliers(&self) -> AppResult<Vec<SupplierResponse>> {
        self.supplier_repo.list().await
    }

    pub async fn get_supplier(&self, id: &str) -> AppResult<SupplierResponse> {
        self.supplier_repo.get_by_id(id).await
    }

    pub async fn create_supplier(&self, input: CreateSupplier) -> AppResult<SupplierResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.supplier_repo.create(&input).await
    }

    pub async fn update_supplier(
        &self,
        id: &str,
        input: UpdateSupplier,
    ) -> AppResult<SupplierResponse> {
        self.supplier_repo.update(id, &input).await
    }

    // Purchase Orders
    pub async fn list_purchase_orders(&self) -> AppResult<Vec<PurchaseOrderResponse>> {
        self.po_repo.list().await
    }

    pub async fn get_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderDetailResponse> {
        let order = self.po_repo.get_by_id(id).await?;
        let lines = self.po_repo.get_lines(id).await?;
        Ok(PurchaseOrderDetailResponse { order, lines })
    }

    pub async fn create_purchase_order(
        &self,
        input: CreatePurchaseOrder,
    ) -> AppResult<PurchaseOrderResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.po_repo.create(&input).await
    }

    pub async fn submit_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "draft" {
            return Err(AppError::Validation(
                "Only draft orders can be submitted".into(),
            ));
        }
        self.po_repo.update_status(id, "submitted").await?;
        self.po_repo.get_by_id(id).await
    }

    pub async fn approve_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "submitted" {
            return Err(AppError::Validation(
                "Only submitted orders can be approved".into(),
            ));
        }
        self.po_repo.update_status(id, "approved").await?;
        self.po_repo.get_by_id(id).await
    }

    pub async fn receive_purchase_order(
        &self,
        id: &str,
        input: ReceivePurchaseOrder,
    ) -> AppResult<PurchaseOrderDetailResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "approved" {
            return Err(AppError::Validation(
                "Only approved orders can be received".into(),
            ));
        }
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        // Validate no over-receiving: quantity_received must not exceed ordered quantity
        let existing_lines = self.po_repo.get_lines(id).await?;
        for line in &input.lines {
            if let Some(po_line) = existing_lines.iter().find(|l| l.id == line.po_line_id) {
                let already_received = po_line.quantity_received;
                let remaining = po_line.quantity - already_received;
                if line.quantity_received > remaining {
                    return Err(AppError::Validation(format!(
                        "Over-receiving not allowed: line {} ordered {}, already received {}, attempting to receive {}. Remaining: {}",
                        line.po_line_id, po_line.quantity, already_received, line.quantity_received, remaining
                    )));
                }
            }
        }

        let mut tx = self.pool.begin().await?;

        let mut proto_lines = Vec::new();
        for line in &input.lines {
            // Create goods receipt
            let receipt_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO goods_receipts (id, po_id, po_line_id, quantity_received, received_date) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&receipt_id).bind(id).bind(&line.po_line_id).bind(line.quantity_received).bind(&today)
            .execute(&mut *tx).await?;

            // Update line received quantity
            sqlx::query(
                "UPDATE po_lines SET quantity_received = quantity_received + ? WHERE id = ?",
            )
            .bind(line.quantity_received)
            .bind(&line.po_line_id)
            .execute(&mut *tx)
            .await?;

            // Build event data from the line details we already have
            if let Some(po_line) = existing_lines.iter().find(|l| l.id == line.po_line_id) {
                proto_lines.push(saas_proto::events::PurchaseOrderLineReceived {
                    item_id: po_line.item_id.clone(),
                    warehouse_id: line.warehouse_id.clone(),
                    quantity_received: line.quantity_received,
                });
            }
        }

        // Update PO status
        sqlx::query("UPDATE purchase_orders SET status = ? WHERE id = ?")
            .bind("received")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        if let Err(e) = self
            .bus
            .publish(
                "scm.procurement.po.received",
                saas_proto::events::PurchaseOrderReceived {
                    po_id: id.to_string(),
                    supplier_id: po.supplier_id.clone(),
                    lines: proto_lines,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.procurement.po.received",
                e
            );
        }
        self.get_purchase_order(id).await
    }
}

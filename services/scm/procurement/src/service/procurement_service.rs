use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::repository::supplier_repo::SupplierRepo;
use crate::repository::purchase_order_repo::PurchaseOrderRepo;
use crate::repository::goods_receipt_repo::GoodsReceiptRepo;
use crate::models::supplier::*;
use crate::models::purchase_order::*;
use validator::Validate;

#[derive(Clone)]
pub struct ProcurementService {
    supplier_repo: SupplierRepo,
    po_repo: PurchaseOrderRepo,
    receipt_repo: GoodsReceiptRepo,
    bus: NatsBus,
}

impl ProcurementService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool.clone()),
            receipt_repo: GoodsReceiptRepo::new(pool),
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
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.supplier_repo.create(&input).await
    }

    pub async fn update_supplier(&self, id: &str, input: UpdateSupplier) -> AppResult<SupplierResponse> {
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

    pub async fn create_purchase_order(&self, input: CreatePurchaseOrder) -> AppResult<PurchaseOrderResponse> {
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.po_repo.create(&input).await
    }

    pub async fn submit_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "draft" {
            return Err(saas_common::error::AppError::Validation("Only draft orders can be submitted".into()));
        }
        self.po_repo.update_status(id, "submitted").await?;
        self.po_repo.get_by_id(id).await
    }

    pub async fn approve_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "submitted" {
            return Err(saas_common::error::AppError::Validation("Only submitted orders can be approved".into()));
        }
        self.po_repo.update_status(id, "approved").await?;
        self.po_repo.get_by_id(id).await
    }

    pub async fn receive_purchase_order(&self, id: &str, input: ReceivePurchaseOrder) -> AppResult<PurchaseOrderDetailResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "approved" {
            return Err(saas_common::error::AppError::Validation("Only approved orders can be received".into()));
        }
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let mut proto_lines = Vec::new();
        for line in &input.lines {
            self.receipt_repo.create(id, &line.po_line_id, line.quantity_received, &today).await?;
            self.po_repo.update_line_received(&line.po_line_id, line.quantity_received).await?;
            // Get line details for event
            let lines = self.po_repo.get_lines(id).await?;
            if let Some(po_line) = lines.iter().find(|l| l.id == line.po_line_id) {
                proto_lines.push(saas_proto::events::PurchaseOrderLineReceived {
                    item_id: po_line.item_id.clone(),
                    warehouse_id: line.warehouse_id.clone(),
                    quantity_received: line.quantity_received,
                });
            }
        }
        self.po_repo.update_status(id, "received").await?;
        let _ = self.bus.publish("scm.procurement.po.received", saas_proto::events::PurchaseOrderReceived {
            po_id: id.to_string(),
            supplier_id: po.supplier_id.clone(),
            lines: proto_lines,
        }).await;
        self.get_purchase_order(id).await
    }
}

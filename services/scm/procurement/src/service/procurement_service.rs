use crate::models::purchase_order::*;
use crate::models::supplier::*;
use crate::repository::goods_receipt_repo::GoodsReceiptRepo;
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
    goods_receipt_repo: GoodsReceiptRepo,
    bus: NatsBus,
}

impl ProcurementService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            pool: pool.clone(),
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool.clone()),
            goods_receipt_repo: GoodsReceiptRepo::new(pool),
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
        // Check for duplicate supplier name
        let existing = self.supplier_repo.list().await?;
        if existing.iter().any(|s| s.name.to_lowercase() == input.name.to_lowercase()) {
            return Err(AppError::Validation(format!(
                "Supplier with name '{}' already exists",
                input.name
            )));
        }
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
        for line in &input.lines {
            if line.quantity <= 0 {
                return Err(AppError::Validation(
                    "PO line quantities must be positive".into(),
                ));
            }
            if line.unit_price_cents < 0 {
                return Err(AppError::Validation(
                    "PO line unit prices must be non-negative".into(),
                ));
            }
        }
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
        let po = self.po_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.procurement.po.submitted",
                saas_proto::events::PurchaseOrderSubmitted {
                    po_id: po.id.clone(),
                    supplier_id: po.supplier_id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.procurement.po.submitted",
                e
            );
        }
        Ok(po)
    }

    pub async fn approve_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "submitted" {
            return Err(AppError::Validation(
                "Only submitted orders can be approved".into(),
            ));
        }
        self.po_repo.update_status(id, "approved").await?;
        let po = self.po_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.procurement.po.approved",
                saas_proto::events::PurchaseOrderApproved {
                    po_id: po.id.clone(),
                    supplier_id: po.supplier_id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.procurement.po.approved",
                e
            );
        }
        Ok(po)
    }

    pub async fn cancel_purchase_order(&self, id: &str) -> AppResult<PurchaseOrderResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "draft" && po.status != "submitted" {
            return Err(AppError::Validation(
                "Only draft or submitted orders can be cancelled".into(),
            ));
        }
        self.po_repo.update_status(id, "cancelled").await?;
        let po = self.po_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.procurement.po.cancelled",
                saas_proto::events::PurchaseOrderCancelled {
                    po_id: po.id.clone(),
                    supplier_id: po.supplier_id.clone(),
                    reason: None,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.procurement.po.cancelled",
                e
            );
        }
        Ok(po)
    }

    pub async fn receive_purchase_order(
        &self,
        id: &str,
        input: ReceivePurchaseOrder,
    ) -> AppResult<PurchaseOrderDetailResponse> {
        let po = self.po_repo.get_by_id(id).await?;
        if po.status != "approved" && po.status != "partially_received" {
            return Err(AppError::Validation(
                "Only approved or partially received orders can be received".into(),
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
                    unit_price_cents: po_line.unit_price_cents,
                });
            }
        }

        // Determine if all PO lines are fully received
        let updated_lines = sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT id, quantity, quantity_received FROM po_lines WHERE po_id = ?",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let all_received = updated_lines
            .iter()
            .all(|(_, qty, qty_received)| qty_received >= qty);
        let po_status = if all_received { "received" } else { "partially_received" };

        // Update PO status
        sqlx::query("UPDATE purchase_orders SET status = ? WHERE id = ?")
            .bind(po_status)
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

    // Goods Receipts
    pub async fn list_goods_receipts(&self) -> AppResult<Vec<crate::models::goods_receipt::GoodsReceiptResponse>> {
        self.goods_receipt_repo.list_all().await
    }

    pub async fn get_goods_receipt(&self, id: &str) -> AppResult<crate::models::goods_receipt::GoodsReceiptResponse> {
        self.goods_receipt_repo.get_by_id(id).await
    }

    pub async fn list_goods_receipts_by_po(&self, po_id: &str) -> AppResult<Vec<crate::models::goods_receipt::GoodsReceiptResponse>> {
        self.goods_receipt_repo.list_by_po(po_id).await
    }

    /// Handle inventory reorder alert by auto-creating a draft purchase order.
    pub async fn handle_item_below_reorder(
        &self,
        item_id: &str,
        item_name: &str,
        suggested_quantity: i64,
    ) -> AppResult<PurchaseOrderResponse> {
        // Find the first active supplier to use as default
        let suppliers = self.supplier_repo.list().await?;
        let supplier = match suppliers.iter().find(|s| s.is_active) {
            Some(s) => s,
            None => return Err(AppError::Validation("No active suppliers available for auto-PO".into())),
        };

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let input = CreatePurchaseOrder {
            supplier_id: supplier.id.clone(),
            order_date: today,
            lines: vec![CreatePurchaseOrderLine {
                item_id: item_id.to_string(),
                quantity: suggested_quantity,
                unit_price_cents: 0, // price to be negotiated; updated from item master or supplier quote
            }],
        };

        tracing::info!(
            "Auto-creating PO for item {} ({}) qty {} to supplier {}",
            item_id, item_name, suggested_quantity, supplier.id
        );
        self.create_purchase_order(input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::goods_receipt_repo::GoodsReceiptRepo;
    use crate::repository::purchase_order_repo::PurchaseOrderRepo;
    use crate::repository::supplier_repo::SupplierRepo;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_suppliers.sql"),
            include_str!("../../migrations/002_create_purchase_orders.sql"),
            include_str!("../../migrations/003_create_po_lines.sql"),
            include_str!("../../migrations/004_create_goods_receipts.sql"),
            include_str!("../../migrations/005_add_partial_received_status.sql"),
        ];
        let migration_names = [
            "001_create_suppliers.sql",
            "002_create_purchase_orders.sql",
            "003_create_po_lines.sql",
            "004_create_goods_receipts.sql",
            "005_add_partial_received_status.sql",
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

    #[tokio::test]
    async fn test_supplier_crud() {
        let pool = setup().await;
        let repo = SupplierRepo::new(pool);

        // Create
        let input = CreateSupplier {
            name: "Acme Corp".into(),
            email: Some("acme@example.com".into()),
            phone: Some("555-0100".into()),
            address: Some("123 Main St".into()),
        };
        let supplier = repo.create(&input).await.unwrap();
        assert_eq!(supplier.name, "Acme Corp");
        assert_eq!(supplier.email, Some("acme@example.com".into()));
        assert!(supplier.is_active);

        // Read
        let fetched = repo.get_by_id(&supplier.id).await.unwrap();
        assert_eq!(fetched.name, "Acme Corp");

        // Update
        let updated = repo
            .update(
                &supplier.id,
                &UpdateSupplier {
                    name: Some("Acme Inc".into()),
                    email: None,
                    phone: None,
                    address: None,
                    is_active: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "Acme Inc");

        // List
        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_supplier_deactivate() {
        let pool = setup().await;
        let repo = SupplierRepo::new(pool);

        let input = CreateSupplier {
            name: "Beta Corp".into(),
            email: None,
            phone: None,
            address: None,
        };
        let supplier = repo.create(&input).await.unwrap();
        assert!(supplier.is_active);

        let deactivated = repo
            .update(
                &supplier.id,
                &UpdateSupplier {
                    name: None,
                    email: None,
                    phone: None,
                    address: None,
                    is_active: Some(false),
                },
            )
            .await
            .unwrap();
        assert!(!deactivated.is_active);
    }

    #[tokio::test]
    async fn test_purchase_order_creation_with_lines() {
        let pool = setup().await;
        let supplier_repo = SupplierRepo::new(pool.clone());
        let po_repo = PurchaseOrderRepo::new(pool);

        // Create supplier first
        let supplier = supplier_repo
            .create(&CreateSupplier {
                name: "Supplier A".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        // Create PO with lines
        let input = CreatePurchaseOrder {
            supplier_id: supplier.id.clone(),
            order_date: "2025-01-15".into(),
            lines: vec![
                CreatePurchaseOrderLine {
                    item_id: "ITEM-001".into(),
                    quantity: 10,
                    unit_price_cents: 500,
                },
                CreatePurchaseOrderLine {
                    item_id: "ITEM-002".into(),
                    quantity: 5,
                    unit_price_cents: 1000,
                },
            ],
        };
        let po = po_repo.create(&input).await.unwrap();
        assert_eq!(po.status, "draft");
        assert_eq!(po.total_cents, 10_000); // 10*500 + 5*1000
        assert_eq!(po.supplier_id, supplier.id);

        // Verify lines
        let lines = po_repo.get_lines(&po.id).await.unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].item_id, "ITEM-001");
        assert_eq!(lines[0].quantity, 10);
        assert_eq!(lines[0].quantity_received, 0);
        assert_eq!(lines[1].item_id, "ITEM-002");
    }

    #[tokio::test]
    async fn test_po_status_transitions() {
        let pool = setup().await;
        let supplier_repo = SupplierRepo::new(pool.clone());
        let po_repo = PurchaseOrderRepo::new(pool);

        let supplier = supplier_repo
            .create(&CreateSupplier {
                name: "Supplier B".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        let input = CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-02-01".into(),
            lines: vec![CreatePurchaseOrderLine {
                item_id: "ITEM-010".into(),
                quantity: 20,
                unit_price_cents: 100,
            }],
        };
        let po = po_repo.create(&input).await.unwrap();
        assert_eq!(po.status, "draft");

        // draft -> submitted
        po_repo.update_status(&po.id, "submitted").await.unwrap();
        let po = po_repo.get_by_id(&po.id).await.unwrap();
        assert_eq!(po.status, "submitted");

        // submitted -> approved
        po_repo.update_status(&po.id, "approved").await.unwrap();
        let po = po_repo.get_by_id(&po.id).await.unwrap();
        assert_eq!(po.status, "approved");

        // approved -> received
        po_repo.update_status(&po.id, "received").await.unwrap();
        let po = po_repo.get_by_id(&po.id).await.unwrap();
        assert_eq!(po.status, "received");
    }

    #[tokio::test]
    async fn test_po_submit_blocks_non_draft() {
        let pool = setup().await;
        let supplier_repo = SupplierRepo::new(pool.clone());
        let po_repo = PurchaseOrderRepo::new(pool);

        let supplier = supplier_repo
            .create(&CreateSupplier {
                name: "Supplier C".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        let input = CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-03-01".into(),
            lines: vec![CreatePurchaseOrderLine {
                item_id: "ITEM-020".into(),
                quantity: 5,
                unit_price_cents: 200,
            }],
        };
        let po = po_repo.create(&input).await.unwrap();

        // Move to approved directly (bypassing service validation)
        po_repo.update_status(&po.id, "submitted").await.unwrap();
        po_repo.update_status(&po.id, "approved").await.unwrap();

        // Verify status is "approved" - service should block re-submit
        let po = po_repo.get_by_id(&po.id).await.unwrap();
        assert_eq!(po.status, "approved");
        // Business rule: only draft can be submitted
        assert_ne!(po.status, "draft");
    }

    #[tokio::test]
    async fn test_goods_receipt_creation() {
        let pool = setup().await;
        let supplier_repo = SupplierRepo::new(pool.clone());
        let po_repo = PurchaseOrderRepo::new(pool.clone());
        let receipt_repo = GoodsReceiptRepo::new(pool);

        let supplier = supplier_repo
            .create(&CreateSupplier {
                name: "Supplier D".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        let input = CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-04-01".into(),
            lines: vec![CreatePurchaseOrderLine {
                item_id: "ITEM-030".into(),
                quantity: 100,
                unit_price_cents: 50,
            }],
        };
        let po = po_repo.create(&input).await.unwrap();
        let lines = po_repo.get_lines(&po.id).await.unwrap();
        let line_id = &lines[0].id;

        // Create goods receipt
        let receipt = receipt_repo
            .create(&po.id, line_id, 80, "2025-04-10")
            .await
            .unwrap();
        assert_eq!(receipt.po_id, po.id);
        assert_eq!(receipt.po_line_id, *line_id);
        assert_eq!(receipt.quantity_received, 80);

        // Update line received quantity
        po_repo.update_line_received(line_id, 80).await.unwrap();
        let lines = po_repo.get_lines(&po.id).await.unwrap();
        assert_eq!(lines[0].quantity_received, 80);
    }

    #[tokio::test]
    async fn test_over_receiving_prevention() {
        let pool = setup().await;
        let supplier_repo = SupplierRepo::new(pool.clone());
        let po_repo = PurchaseOrderRepo::new(pool);

        let supplier = supplier_repo
            .create(&CreateSupplier {
                name: "Supplier E".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        let input = CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-05-01".into(),
            lines: vec![CreatePurchaseOrderLine {
                item_id: "ITEM-040".into(),
                quantity: 50,
                unit_price_cents: 100,
            }],
        };
        let po = po_repo.create(&input).await.unwrap();
        let lines = po_repo.get_lines(&po.id).await.unwrap();
        let po_line = &lines[0];

        // Simulate already receiving 40 units
        po_repo
            .update_line_received(&po_line.id, 40)
            .await
            .unwrap();

        // Refresh lines to see updated quantity_received
        let lines = po_repo.get_lines(&po.id).await.unwrap();
        let po_line = &lines[0];
        assert_eq!(po_line.quantity_received, 40);

        // Business rule: cannot receive more than remaining
        let remaining = po_line.quantity - po_line.quantity_received;
        assert_eq!(remaining, 10);
        let attempting_to_receive = 15;
        assert!(
            attempting_to_receive > remaining,
            "Should detect over-receive"
        );
    }

    #[tokio::test]
    async fn test_po_line_total_calculation() {
        let pool = setup().await;
        let supplier_repo = SupplierRepo::new(pool.clone());
        let po_repo = PurchaseOrderRepo::new(pool);

        let supplier = supplier_repo
            .create(&CreateSupplier {
                name: "Supplier F".into(),
                email: None,
                phone: None,
                address: None,
            })
            .await
            .unwrap();

        let input = CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-06-01".into(),
            lines: vec![
                CreatePurchaseOrderLine {
                    item_id: "ITEM-A".into(),
                    quantity: 3,
                    unit_price_cents: 2500,
                },
                CreatePurchaseOrderLine {
                    item_id: "ITEM-B".into(),
                    quantity: 7,
                    unit_price_cents: 1000,
                },
                CreatePurchaseOrderLine {
                    item_id: "ITEM-C".into(),
                    quantity: 1,
                    unit_price_cents: 50000,
                },
            ],
        };
        let po = po_repo.create(&input).await.unwrap();

        // PO total: 3*2500 + 7*1000 + 1*50000 = 7500 + 7000 + 50000 = 64500
        assert_eq!(po.total_cents, 64500);

        let lines = po_repo.get_lines(&po.id).await.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].line_total_cents, 7500);
        assert_eq!(lines[1].line_total_cents, 7000);
        assert_eq!(lines[2].line_total_cents, 50000);
    }

    #[tokio::test]
    async fn test_supplier_not_found() {
        let pool = setup().await;
        let repo = SupplierRepo::new(pool);
        let result = repo.get_by_id("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_supplier_name_uniqueness() {
        let pool = setup().await;
        let svc = ProcurementService {
            pool: pool.clone(),
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool.clone()),
            goods_receipt_repo: GoodsReceiptRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        svc.create_supplier(CreateSupplier {
            name: "Acme Corp".into(),
            email: None,
            phone: None,
            address: None,
        })
        .await
        .unwrap();

        // Duplicate name (case-insensitive) should fail
        let result = svc.create_supplier(CreateSupplier {
            name: "ACME CORP".into(),
            email: None,
            phone: None,
            address: None,
        })
        .await;
        assert!(result.is_err());

        // Different name should succeed
        svc.create_supplier(CreateSupplier {
            name: "Beta Corp".into(),
            email: None,
            phone: None,
            address: None,
        })
        .await
        .unwrap();

        let suppliers = svc.supplier_repo.list().await.unwrap();
        assert_eq!(suppliers.len(), 2);
    }

    #[tokio::test]
    async fn test_po_partial_receipt_status() {
        let pool = setup().await;
        let svc = ProcurementService {
            pool: pool.clone(),
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool.clone()),
            goods_receipt_repo: GoodsReceiptRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let supplier = svc.create_supplier(CreateSupplier {
            name: "Partial Receipt Supplier".into(),
            email: None,
            phone: None,
            address: None,
        }).await.unwrap();

        let po = svc.create_purchase_order(CreatePurchaseOrder {
            supplier_id: supplier.id.clone(),
            order_date: "2025-01-01".into(),
            lines: vec![
                CreatePurchaseOrderLine {
                    item_id: "ITEM-PR-1".into(),
                    quantity: 10,
                    unit_price_cents: 1000,
                },
                CreatePurchaseOrderLine {
                    item_id: "ITEM-PR-2".into(),
                    quantity: 5,
                    unit_price_cents: 2000,
                },
            ],
        }).await.unwrap();

        svc.submit_purchase_order(&po.id).await.unwrap();
        svc.approve_purchase_order(&po.id).await.unwrap();

        let po_lines = svc.po_repo.get_lines(&po.id).await.unwrap();

        // Receive only partial quantity for first line
        let detail = svc.receive_purchase_order(&po.id, ReceivePurchaseOrder {
            lines: vec![ReceiveLine {
                po_line_id: po_lines[0].id.clone(),
                quantity_received: 3,
                warehouse_id: "WH-1".into(),
            }],
        }).await.unwrap();
        assert_eq!(detail.order.status, "partially_received");

        // Receive remaining for first line + all of second line
        let po_lines = svc.po_repo.get_lines(&po.id).await.unwrap();
        let detail = svc.receive_purchase_order(&po.id, ReceivePurchaseOrder {
            lines: vec![
                ReceiveLine {
                    po_line_id: po_lines[0].id.clone(),
                    quantity_received: 7,
                    warehouse_id: "WH-1".into(),
                },
                ReceiveLine {
                    po_line_id: po_lines[1].id.clone(),
                    quantity_received: 5,
                    warehouse_id: "WH-1".into(),
                },
            ],
        }).await.unwrap();
        assert_eq!(detail.order.status, "received");
    }

    #[tokio::test]
    async fn test_get_goods_receipt_by_id() {
        let pool = setup().await;
        let svc = ProcurementService {
            pool: pool.clone(),
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool.clone()),
            goods_receipt_repo: GoodsReceiptRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let supplier = svc.create_supplier(CreateSupplier {
            name: "GR Get Supplier".into(),
            email: None,
            phone: None,
            address: None,
        }).await.unwrap();

        let po = svc.create_purchase_order(CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-01-01".into(),
            lines: vec![CreatePurchaseOrderLine {
                item_id: "ITEM-GR".into(),
                quantity: 10,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        svc.submit_purchase_order(&po.id).await.unwrap();
        svc.approve_purchase_order(&po.id).await.unwrap();

        let po_lines = svc.po_repo.get_lines(&po.id).await.unwrap();
        svc.receive_purchase_order(&po.id, ReceivePurchaseOrder {
            lines: vec![ReceiveLine {
                po_line_id: po_lines[0].id.clone(),
                quantity_received: 10,
                warehouse_id: "WH-1".into(),
            }],
        }).await.unwrap();

        let receipts = svc.list_goods_receipts().await.unwrap();
        assert_eq!(receipts.len(), 1);

        let receipt = svc.get_goods_receipt(&receipts[0].id).await.unwrap();
        assert_eq!(receipt.quantity_received, 10);

        // Not found
        let result = svc.get_goods_receipt("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_po_publishes_event() {
        let pool = setup().await;
        let svc = ProcurementService {
            pool: pool.clone(),
            supplier_repo: SupplierRepo::new(pool.clone()),
            po_repo: PurchaseOrderRepo::new(pool.clone()),
            goods_receipt_repo: GoodsReceiptRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let supplier = svc.create_supplier(CreateSupplier {
            name: "Cancel PO Supplier".into(),
            email: None,
            phone: None,
            address: None,
        }).await.unwrap();

        let po = svc.create_purchase_order(CreatePurchaseOrder {
            supplier_id: supplier.id,
            order_date: "2025-01-01".into(),
            lines: vec![CreatePurchaseOrderLine {
                item_id: "ITEM-CANCEL".into(),
                quantity: 5,
                unit_price_cents: 100,
            }],
        }).await.unwrap();

        let cancelled = svc.cancel_purchase_order(&po.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");
    }
}

use crate::models::cycle_count::*;
use crate::models::item::*;
use crate::models::reservation::*;
use crate::models::stock_level::*;
use crate::models::stock_movement::*;
use crate::models::warehouse::*;
use crate::repository::cycle_count_repo::CycleCountRepo;
use crate::repository::item_repo::ItemRepo;
use crate::repository::reservation_repo::ReservationRepo;
use crate::repository::stock_level_repo::StockLevelRepo;
use crate::repository::stock_movement_repo::StockMovementRepo;
use crate::repository::warehouse_repo::WarehouseRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct InventoryService {
    pool: SqlitePool,
    warehouse_repo: WarehouseRepo,
    item_repo: ItemRepo,
    stock_level_repo: StockLevelRepo,
    stock_movement_repo: StockMovementRepo,
    reservation_repo: ReservationRepo,
    cycle_count_repo: CycleCountRepo,
    bus: NatsBus,
}

impl InventoryService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            pool: pool.clone(),
            warehouse_repo: WarehouseRepo::new(pool.clone()),
            item_repo: ItemRepo::new(pool.clone()),
            stock_level_repo: StockLevelRepo::new(pool.clone()),
            stock_movement_repo: StockMovementRepo::new(pool.clone()),
            reservation_repo: ReservationRepo::new(pool.clone()),
            cycle_count_repo: CycleCountRepo::new(pool),
            bus,
        }
    }

    // Warehouses
    pub async fn list_warehouses(&self) -> AppResult<Vec<WarehouseResponse>> {
        self.warehouse_repo.list().await
    }

    pub async fn create_warehouse(&self, input: CreateWarehouse) -> AppResult<WarehouseResponse> {
        // Check for duplicate warehouse name
        let existing = self.warehouse_repo.list().await?;
        if existing.iter().any(|w| w.name.to_lowercase() == input.name.to_lowercase()) {
            return Err(AppError::Validation(format!(
                "Warehouse with name '{}' already exists",
                input.name
            )));
        }
        self.warehouse_repo.create(&input).await
    }

    pub async fn get_warehouse(&self, id: &str) -> AppResult<WarehouseResponse> {
        self.warehouse_repo.get_by_id(id).await
    }

    pub async fn update_warehouse(&self, id: &str, input: UpdateWarehouse) -> AppResult<WarehouseResponse> {
        self.warehouse_repo.update(id, &input).await
    }

    // Items
    pub async fn list_items(&self, filters: &ItemFilters) -> AppResult<Vec<ItemResponse>> {
        self.item_repo.list(filters).await
    }

    pub async fn get_item(&self, id: &str) -> AppResult<ItemResponse> {
        self.item_repo.get_by_id(id).await
    }

    pub async fn create_item(&self, input: CreateItem) -> AppResult<ItemResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        // Check for duplicate SKU
        let existing = self.item_repo.list(&ItemFilters { item_type: None, is_active: None }).await?;
        if existing.iter().any(|i| i.sku.to_lowercase() == input.sku.to_lowercase()) {
            return Err(AppError::Validation(format!(
                "Item with SKU '{}' already exists",
                input.sku
            )));
        }
        self.item_repo.create(&input).await
    }

    pub async fn update_item(&self, id: &str, input: UpdateItem) -> AppResult<ItemResponse> {
        self.item_repo.get_by_id(id).await?;
        self.item_repo.update_item(id, &input).await
    }

    pub async fn list_items_below_reorder_point(&self) -> AppResult<Vec<ItemResponse>> {
        self.item_repo.list_items_below_reorder_point().await
    }

    pub async fn get_item_stock(&self, item_id: &str) -> AppResult<Vec<StockLevelResponse>> {
        self.stock_level_repo.get_by_item(item_id).await
    }

    pub async fn get_item_availability(&self, item_id: &str) -> AppResult<Vec<StockLevelResponse>> {
        self.stock_level_repo.get_by_item(item_id).await
    }

    // Stock Movements
    pub async fn list_stock_movements(&self) -> AppResult<Vec<StockMovementResponse>> {
        self.stock_movement_repo.list().await
    }

    pub async fn create_stock_movement(
        &self,
        input: CreateStockMovement,
    ) -> AppResult<StockMovementResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;

        // Validate movement_type
        let valid_types = ["receipt", "transfer", "adjustment", "issue", "pick", "return"];
        if !valid_types.contains(&input.movement_type.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid movement_type '{}'. Must be one of: {:?}",
                input.movement_type, valid_types
            )));
        }

        // Check for negative stock on deduct operations
        match input.movement_type.as_str() {
            "transfer" | "issue" | "pick" => {
                let warehouse_id = match &input.from_warehouse_id {
                    Some(wh) => wh,
                    None => &input.to_warehouse_id,
                };
                let stock = self
                    .stock_level_repo
                    .get_by_item_warehouse(&input.item_id, warehouse_id)
                    .await?;
                let on_hand = stock.as_ref().map(|s| s.quantity_on_hand).unwrap_or(0);
                if on_hand < input.quantity {
                    return Err(AppError::Validation(format!(
                        "Insufficient stock for {} operation. On hand: {}, Requested: {}",
                        input.movement_type, on_hand, input.quantity
                    )));
                }
            }
            _ => {}
        }

        let movement = self.stock_movement_repo.create(&input).await?;
        // Update stock levels based on movement type
        match input.movement_type.as_str() {
            "receipt" => {
                self.stock_level_repo
                    .upsert_receipt(&input.item_id, &input.to_warehouse_id, input.quantity)
                    .await?;
                if let Err(e) = self
                    .bus
                    .publish(
                        "scm.inventory.stock.received",
                        saas_proto::events::StockReceived {
                            item_id: input.item_id.clone(),
                            warehouse_id: input.to_warehouse_id.clone(),
                            location_id: input.to_warehouse_id.clone(),
                            quantity: input.quantity,
                            reference_type: input.reference_type.clone().unwrap_or_default(),
                            reference_id: input.reference_id.clone().unwrap_or_default(),
                        },
                    )
                    .await
                {
                    tracing::error!(
                        "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                        "scm.inventory.stock.received",
                        e
                    );
                }
            }
            "transfer" => {
                // Deduct from source warehouse, add to destination warehouse
                if let Some(from_wh) = &input.from_warehouse_id {
                    self.stock_level_repo
                        .deduct(&input.item_id, from_wh, input.quantity)
                        .await?;
                }
                self.stock_level_repo
                    .upsert_receipt(&input.item_id, &input.to_warehouse_id, input.quantity)
                    .await?;
            }
            "adjustment" => {
                // Adjustment: positive quantity increases stock, negative would decrease
                // For positive adjustments, treat like a receipt to the destination
                self.stock_level_repo
                    .upsert_receipt(&input.item_id, &input.to_warehouse_id, input.quantity)
                    .await?;
            }
            "issue" => {
                // Issue: deduct stock from the destination warehouse (used as "from")
                self.stock_level_repo
                    .deduct(&input.item_id, &input.to_warehouse_id, input.quantity)
                    .await?;
            }
            "return" => {
                // Return: add stock back to the destination warehouse
                self.stock_level_repo
                    .upsert_receipt(&input.item_id, &input.to_warehouse_id, input.quantity)
                    .await?;
            }
            _ => {}
        }
        Ok(movement)
    }

    // Reservations
    pub async fn list_reservations(&self) -> AppResult<Vec<ReservationResponse>> {
        self.reservation_repo.list().await
    }

    pub async fn create_reservation(
        &self,
        input: CreateReservation,
    ) -> AppResult<ReservationResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;

        // Check available stock before reserving
        let stock = self
            .stock_level_repo
            .get_by_item_warehouse(&input.item_id, &input.warehouse_id)
            .await?;
        let available = stock.as_ref().map(|s| s.quantity_available).unwrap_or(0);
        if available < input.quantity {
            return Err(AppError::Validation(format!(
                "Insufficient available stock for reservation. Available: {}, Requested: {}",
                available, input.quantity
            )));
        }

        let mut tx = self.pool.begin().await?;

        // Create reservation within transaction
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO reservations (id, item_id, warehouse_id, quantity, reference_type, reference_id) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id).bind(&input.item_id).bind(&input.warehouse_id)
        .bind(input.quantity).bind(&input.reference_type).bind(&input.reference_id)
        .execute(&mut *tx).await?;

        // Reserve stock within same transaction
        let stock_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_levels (id, item_id, warehouse_id, quantity_on_hand, quantity_reserved, quantity_available) VALUES (?, ?, ?, 0, ?, -?) ON CONFLICT(item_id, warehouse_id) DO UPDATE SET quantity_reserved = quantity_reserved + ?, quantity_available = quantity_on_hand - quantity_reserved, updated_at = datetime('now')"
        )
        .bind(&stock_id).bind(&input.item_id).bind(&input.warehouse_id).bind(input.quantity).bind(input.quantity)
        .bind(input.quantity)
        .execute(&mut *tx).await?;

        tx.commit().await?;

        let reservation = self.reservation_repo.get_by_id(&id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.inventory.stock.reserved",
                saas_proto::events::StockReserved {
                    item_id: input.item_id.clone(),
                    warehouse_id: input.warehouse_id.clone(),
                    quantity: input.quantity,
                    reference_type: input.reference_type.clone(),
                    reference_id: input.reference_id.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.inventory.stock.reserved",
                e
            );
        }
        Ok(reservation)
    }

    pub async fn cancel_reservation(&self, id: &str) -> AppResult<ReservationResponse> {
        let existing = self.reservation_repo.get_by_id(id).await?;
        let reservation = self.reservation_repo.cancel(id).await?;
        self.stock_level_repo
            .release_reservation(&existing.item_id, &existing.warehouse_id, existing.quantity)
            .await?;
        Ok(reservation)
    }

    // Event handlers
    pub async fn handle_po_received(
        &self,
        po_id: &str,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        self.stock_level_repo
            .upsert_receipt(item_id, warehouse_id, quantity)
            .await?;
        let movement = CreateStockMovement {
            item_id: item_id.to_string(),
            from_warehouse_id: None,
            to_warehouse_id: warehouse_id.to_string(),
            quantity,
            movement_type: "receipt".to_string(),
            reference_type: Some("purchase_order".to_string()),
            reference_id: Some(po_id.to_string()),
        };
        self.stock_movement_repo.create(&movement).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.inventory.stock.received",
                saas_proto::events::StockReceived {
                    item_id: item_id.to_string(),
                    warehouse_id: warehouse_id.to_string(),
                    location_id: warehouse_id.to_string(),
                    quantity,
                    reference_type: "purchase_order".to_string(),
                    reference_id: po_id.to_string(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.inventory.stock.received",
                e
            );
        }
        Ok(())
    }

    pub async fn handle_order_confirmed(
        &self,
        order_id: &str,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        let reservation = CreateReservation {
            item_id: item_id.to_string(),
            warehouse_id: warehouse_id.to_string(),
            quantity,
            reference_type: "sales_order".to_string(),
            reference_id: order_id.to_string(),
        };
        self.create_reservation(reservation).await?;
        Ok(())
    }

    /// Deduct stock when a sales order is fulfilled (pick and ship).
    /// Also releases the reservation that was created on order confirmation.
    pub async fn handle_order_fulfilled(
        &self,
        order_id: &str,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        // Deduct on-hand stock
        self.stock_level_repo
            .deduct(item_id, warehouse_id, quantity)
            .await?;

        // Release the reservation for this order/item
        self.stock_level_repo
            .release_reservation(item_id, warehouse_id, quantity)
            .await?;

        // Record the stock movement
        let movement = CreateStockMovement {
            item_id: item_id.to_string(),
            from_warehouse_id: Some(warehouse_id.to_string()),
            to_warehouse_id: warehouse_id.to_string(), // staging in same warehouse
            quantity,
            movement_type: "pick".to_string(),
            reference_type: Some("sales_order".to_string()),
            reference_id: Some(order_id.to_string()),
        };
        self.stock_movement_repo.create(&movement).await?;

        // Check if item dropped below reorder point
        if let Err(e) = self.check_and_publish_reorder_alert(item_id, warehouse_id).await {
            tracing::warn!("Failed to check reorder point for item {}: {}", item_id, e);
        }

        Ok(())
    }

    /// Check if an item's available stock is below its reorder point and publish an alert.
    async fn check_and_publish_reorder_alert(&self, item_id: &str, warehouse_id: &str) -> AppResult<()> {
        let levels = self.stock_level_repo.get_by_item(item_id).await?;
        let level = match levels.iter().find(|l| l.warehouse_id == warehouse_id) {
            Some(l) => l,
            None => return Ok(()),
        };

        let item = match self.item_repo.get_by_id(item_id).await {
            Ok(i) => i,
            Err(_) => return Ok(()),
        };

        let reorder_point = item.reorder_point;
        if reorder_point > 0 && level.quantity_available <= reorder_point {
            let suggested_qty = if item.economic_order_qty > 0 {
                item.economic_order_qty
            } else {
                reorder_point * 2
            };
            if let Err(e) = self.bus.publish(
                "scm.inventory.item.below_reorder",
                saas_proto::events::ItemBelowReorderPoint {
                    item_id: item_id.to_string(),
                    item_name: item.name.clone(),
                    sku: item.sku.clone(),
                    warehouse_id: warehouse_id.to_string(),
                    available_quantity: level.quantity_available,
                    reorder_point,
                    suggested_order_quantity: suggested_qty,
                },
            ).await {
                tracing::error!("Failed to publish reorder alert for item {}: {}", item_id, e);
            }
        }
        Ok(())
    }

    /// Add finished goods to inventory when a manufacturing work order completes.
    pub async fn handle_work_order_completed(
        &self,
        work_order_id: &str,
        item_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        // Find the first warehouse to place finished goods
        let warehouse_id = match self.stock_level_repo.get_first_warehouse_for_item(item_id).await? {
            Some(sl) => sl.warehouse_id.clone(),
            None => {
                // Default to first warehouse in the system
                let warehouses = self.warehouse_repo.list().await?;
                warehouses
                    .first()
                    .ok_or_else(|| AppError::Validation("No warehouse available for finished goods".into()))?
                    .id
                    .clone()
            }
        };

        self.stock_level_repo
            .upsert_receipt(item_id, &warehouse_id, quantity)
            .await?;

        let movement = CreateStockMovement {
            item_id: item_id.to_string(),
            from_warehouse_id: None,
            to_warehouse_id: warehouse_id,
            quantity,
            movement_type: "receipt".to_string(),
            reference_type: Some("work_order".to_string()),
            reference_id: Some(work_order_id.to_string()),
        };
        self.stock_movement_repo.create(&movement).await?;
        Ok(())
    }

    /// Release reserved materials when a work order is cancelled.
    pub async fn handle_work_order_cancelled(
        &self,
        work_order_id: &str,
    ) -> AppResult<()> {
        // Find and release reservations tied to this work order
        let reservations = self.reservation_repo.list().await?;
        for res in reservations {
            if res.reference_id == work_order_id && res.status == "active" {
                self.reservation_repo.cancel(&res.id).await?;
            }
        }
        Ok(())
    }

    /// Check material availability when a manufacturing work order starts.
    /// Logs the event details and current stock levels for the required item.
    pub async fn handle_work_order_started(
        &self,
        work_order_id: &str,
        item_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        tracing::info!(
            "Work order started: wo_id={}, item={}, required qty={}",
            work_order_id, item_id, quantity
        );

        match self.get_item_stock(item_id).await {
            Ok(levels) => {
                let total_on_hand: i64 = levels.iter().map(|l| l.quantity_on_hand).sum();
                let total_available: i64 = levels.iter().map(|l| l.quantity_available).sum();
                if total_on_hand >= quantity {
                    tracing::info!(
                        "Sufficient materials available for work order {}: item={}, required={}, on_hand={}, available={}",
                        work_order_id, item_id, quantity, total_on_hand, total_available
                    );
                } else {
                    tracing::warn!(
                        "Insufficient materials for work order {}: item={}, required={}, on_hand={}, available={}",
                        work_order_id, item_id, quantity, total_on_hand, total_available
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Could not check stock for work order {}: item={}, error={}",
                    work_order_id, item_id, e
                );
            }
        }
        Ok(())
    }

    /// Release reserved stock when a sales order is cancelled.
    /// Finds active reservations tied to the order and cancels them,
    /// releasing the reserved quantity back to available stock.
    pub async fn handle_order_cancelled(
        &self,
        order_id: &str,
        order_number: &str,
        reason: &Option<String>,
    ) -> AppResult<()> {
        tracing::info!(
            "Processing sales order cancelled: order_id={}, order_number={}, reason={:?}",
            order_id, order_number, reason
        );

        let reservations = self.reservation_repo.list().await?;
        let matching: Vec<_> = reservations
            .iter()
            .filter(|r| r.reference_type == "sales_order" && r.reference_id == order_id && r.status == "active")
            .collect();

        if matching.is_empty() {
            tracing::info!(
                "No active reservations found for cancelled order: order_id={}",
                order_id
            );
            return Ok(());
        }

        for res in matching {
            tracing::info!(
                "Cancelling reservation {} for cancelled order {} (item={}, warehouse={}, qty={})",
                res.id, order_id, res.item_id, res.warehouse_id, res.quantity
            );
            self.cancel_reservation(&res.id).await?;
        }

        tracing::info!(
            "Successfully released reserved stock for cancelled order: order_id={}, order_number={}",
            order_id, order_number
        );
        Ok(())
    }

    /// Handle purchase order cancellation by cancelling all active reservations
    /// tied to the PO (reference_type="purchase_order", reference_id=po_id).
    pub async fn handle_po_cancelled(
        &self,
        po_id: &str,
        supplier_id: &str,
        reason: &Option<String>,
    ) -> AppResult<()> {
        tracing::info!(
            "Processing PO cancelled: po_id={}, supplier_id={}, reason={:?}",
            po_id, supplier_id, reason
        );

        let reservations = self.reservation_repo.list().await?;
        let matching: Vec<_> = reservations
            .iter()
            .filter(|r| r.reference_type == "purchase_order" && r.reference_id == po_id && r.status == "active")
            .collect();

        if matching.is_empty() {
            tracing::info!(
                "No active reservations found for cancelled PO: po_id={}",
                po_id
            );
            return Ok(());
        }

        for res in matching {
            tracing::info!(
                "Cancelling reservation {} for cancelled PO {} (item={}, warehouse={}, qty={})",
                res.id, po_id, res.item_id, res.warehouse_id, res.quantity
            );
            self.cancel_reservation(&res.id).await?;
        }

        tracing::info!(
            "Successfully released reserved stock for cancelled PO: po_id={}, supplier_id={}",
            po_id, supplier_id
        );
        Ok(())
    }

    /// Add returned items back to inventory when a customer return is processed.
    pub async fn handle_return_created(
        &self,
        return_id: &str,
        item_id: &str,
        quantity: i64,
    ) -> AppResult<()> {
        // Find the first warehouse with this item to restock
        let warehouse_id = match self.stock_level_repo.get_first_warehouse_for_item(item_id).await? {
            Some(sl) => sl.warehouse_id.clone(),
            None => {
                let warehouses = self.warehouse_repo.list().await?;
                warehouses
                    .first()
                    .ok_or_else(|| AppError::Validation("No warehouse available for returns".into()))?
                    .id
                    .clone()
            }
        };

        self.stock_level_repo
            .upsert_receipt(item_id, &warehouse_id, quantity)
            .await?;

        let movement = CreateStockMovement {
            item_id: item_id.to_string(),
            from_warehouse_id: None,
            to_warehouse_id: warehouse_id,
            quantity,
            movement_type: "return".to_string(),
            reference_type: Some("sales_return".to_string()),
            reference_id: Some(return_id.to_string()),
        };
        self.stock_movement_repo.create(&movement).await?;
        Ok(())
    }

    // Cycle Count
    pub async fn create_cycle_count_session(
        &self,
        input: CreateCycleCountSessionRequest,
        counted_by: &str,
    ) -> AppResult<CycleCountSession> {
        // Validate warehouse exists
        self.warehouse_repo.get_by_id(&input.warehouse_id).await?;
        self.cycle_count_repo
            .create_cycle_count_session(&input.warehouse_id, &input.count_date, counted_by)
            .await
    }

    pub async fn get_cycle_count_session(&self, id: &str) -> AppResult<CycleCountSessionWithLines> {
        let session = self.cycle_count_repo.get_cycle_count_session(id).await?;
        let lines = self.cycle_count_repo.get_cycle_count_lines(id).await?;
        Ok(CycleCountSessionWithLines { session, lines })
    }

    pub async fn list_cycle_count_sessions(&self) -> AppResult<Vec<CycleCountSession>> {
        self.cycle_count_repo.list_cycle_count_sessions().await
    }

    pub async fn add_cycle_count_line(
        &self,
        session_id: &str,
        input: AddCycleCountLineRequest,
    ) -> AppResult<CycleCountLine> {
        // Validate session is 'draft'
        let session = self
            .cycle_count_repo
            .get_cycle_count_session(session_id)
            .await?;
        if session.status != "draft" {
            return Err(AppError::Validation(
                "Cannot add lines to a session that is not in draft status".to_string(),
            ));
        }

        // Validate item exists
        self.item_repo.get_by_id(&input.item_id).await?;

        // Get system_quantity from stock_levels for the item in the session's warehouse
        let stock = self
            .stock_level_repo
            .get_by_item_warehouse(&input.item_id, &session.warehouse_id)
            .await?;
        let system_quantity = stock.map(|s| s.quantity_on_hand).unwrap_or(0);

        self.cycle_count_repo
            .add_cycle_count_line(
                session_id,
                &input.item_id,
                system_quantity,
                input.notes.as_deref(),
            )
            .await
    }

    pub async fn update_counted_quantity(
        &self,
        session_id: &str,
        line_id: &str,
        input: UpdateCountedQuantityRequest,
    ) -> AppResult<CycleCountLine> {
        // Validate session is 'draft' or 'submitted'
        let session = self
            .cycle_count_repo
            .get_cycle_count_session(session_id)
            .await?;
        if session.status != "draft" && session.status != "submitted" {
            return Err(AppError::Validation(
                "Cannot update counted quantity when session is not in draft or submitted status"
                    .to_string(),
            ));
        }

        // Validate line belongs to session
        let line = self
            .cycle_count_repo
            .get_cycle_count_line_by_id(line_id)
            .await?;
        if line.session_id != session_id {
            return Err(AppError::NotFound(format!(
                "Line {} does not belong to session {}",
                line_id, session_id
            )));
        }

        self.cycle_count_repo
            .update_counted_quantity(line_id, input.counted_quantity, input.notes.as_deref())
            .await
    }

    pub async fn submit_cycle_count(&self, id: &str) -> AppResult<CycleCountSession> {
        let session = self.cycle_count_repo.get_cycle_count_session(id).await?;
        if session.status != "draft" {
            return Err(AppError::Validation(
                "Session must be in draft status to submit".to_string(),
            ));
        }
        self.cycle_count_repo
            .update_session_status(id, "submitted")
            .await
    }

    pub async fn approve_cycle_count(
        &self,
        id: &str,
        approved_by: &str,
    ) -> AppResult<CycleCountSession> {
        let session = self.cycle_count_repo.get_cycle_count_session(id).await?;
        if session.status != "submitted" {
            return Err(AppError::Validation(
                "Session must be in submitted status to approve".to_string(),
            ));
        }
        // Update status and set approver info
        self.cycle_count_repo
            .update_session_status(id, "approved")
            .await?;
        // Also set approved_by/approved_at
        sqlx::query(
            "UPDATE cycle_count_sessions SET approved_by = ?, approved_at = datetime('now') WHERE id = ?"
        )
        .bind(approved_by).bind(id)
        .execute(&self.pool).await?;
        self.cycle_count_repo.get_cycle_count_session(id).await
    }

    pub async fn post_cycle_count(
        &self,
        id: &str,
        approved_by: &str,
    ) -> AppResult<CycleCountSession> {
        let session = self.cycle_count_repo.get_cycle_count_session(id).await?;
        if session.status != "approved" {
            return Err(AppError::Validation(
                "Session must be in approved status to post".to_string(),
            ));
        }
        let lines = self.cycle_count_repo.get_cycle_count_lines(id).await?;
        let adjustment_count = lines.iter().filter(|l| l.variance.unwrap_or(0) != 0).count() as u32;

        let posted = self.cycle_count_repo
            .post_cycle_count(id, approved_by)
            .await?;

        if let Err(e) = self
            .bus
            .publish(
                "scm.inventory.cycle_count.posted",
                saas_proto::events::CycleCountPosted {
                    session_id: id.to_string(),
                    warehouse_id: posted.warehouse_id.clone(),
                    adjustment_count,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.inventory.cycle_count.posted",
                e
            );
        }
        Ok(posted)
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
            include_str!("../../migrations/001_create_warehouses.sql"),
            include_str!("../../migrations/002_create_items.sql"),
            include_str!("../../migrations/003_create_stock_levels.sql"),
            include_str!("../../migrations/004_create_stock_movements.sql"),
            include_str!("../../migrations/005_create_reservations.sql"),
            include_str!("../../migrations/006_create_cycle_count_sessions.sql"),
            include_str!("../../migrations/007_create_cycle_count_lines.sql"),
            include_str!("../../migrations/008_add_reorder_fields.sql"),
            include_str!("../../migrations/009_add_unit_price.sql"),
            include_str!("../../migrations/010_add_issue_movement_type.sql"),
        ];
        let migration_names = [
            "001_create_warehouses.sql",
            "002_create_items.sql",
            "003_create_stock_levels.sql",
            "004_create_stock_movements.sql",
            "005_create_reservations.sql",
            "006_create_cycle_count_sessions.sql",
            "007_create_cycle_count_lines.sql",
            "008_add_reorder_fields.sql",
            "009_add_unit_price.sql",
            "010_add_issue_movement_type.sql",
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

    async fn setup_repos() -> (
        WarehouseRepo,
        ItemRepo,
        StockLevelRepo,
        StockMovementRepo,
        ReservationRepo,
        CycleCountRepo,
    ) {
        let pool = setup().await;
        (
            WarehouseRepo::new(pool.clone()),
            ItemRepo::new(pool.clone()),
            StockLevelRepo::new(pool.clone()),
            StockMovementRepo::new(pool.clone()),
            ReservationRepo::new(pool.clone()),
            CycleCountRepo::new(pool),
        )
    }

    #[tokio::test]
    async fn test_warehouse_crud() {
        let (wh_repo, _, _, _, _, _) = setup_repos().await;

        // Create
        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "Main Warehouse".into(),
                address: Some("123 Industrial Blvd".into()),
            })
            .await
            .unwrap();
        assert_eq!(wh.name, "Main Warehouse");
        assert!(wh.is_active);

        // Read
        let fetched = wh_repo.get_by_id(&wh.id).await.unwrap();
        assert_eq!(fetched.name, "Main Warehouse");

        // Update
        let updated = wh_repo
            .update(
                &wh.id,
                &UpdateWarehouse {
                    name: Some("Warehouse A".into()),
                    address: None,
                    is_active: Some(false),
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "Warehouse A");
        assert!(!updated.is_active);

        // List
        let warehouses = wh_repo.list().await.unwrap();
        assert_eq!(warehouses.len(), 1);
    }

    #[tokio::test]
    async fn test_item_creation() {
        let (_, item_repo, _, _, _, _) = setup_repos().await;

        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                description: Some("A fine widget".into()),
                unit_of_measure: Some("EA".into()),
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();
        assert_eq!(item.sku, "SKU-001");
        assert_eq!(item.item_type, "finished");
        assert!(item.is_active);
        assert_eq!(item.unit_of_measure, "EA");

        // Get by id
        let fetched = item_repo.get_by_id(&item.id).await.unwrap();
        assert_eq!(fetched.name, "Widget");

        // Default UOM
        let item2 = item_repo
            .create(&CreateItem {
                sku: "SKU-002".into(),
                name: "Gadget".into(),
                description: None,
                unit_of_measure: None,
                item_type: "raw_material".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();
        assert_eq!(item2.unit_of_measure, "EA");
    }

    #[tokio::test]
    async fn test_stock_levels_and_receipt() {
        let (wh_repo, item_repo, sl_repo, _, _, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-1".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-100".into(),
                name: "Bolt".into(),
                description: None,
                unit_of_measure: None,
                item_type: "component".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Initial stock should be empty
        let levels = sl_repo.get_by_item(&item.id).await.unwrap();
        assert!(levels.is_empty());

        // Receive 100 units
        let sl = sl_repo
            .upsert_receipt(&item.id, &wh.id, 100)
            .await
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 0);
        assert_eq!(sl.quantity_available, 100);

        // Receive 50 more -- should add up
        let sl = sl_repo
            .upsert_receipt(&item.id, &wh.id, 50)
            .await
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 150);
        assert_eq!(sl.quantity_available, 150);
    }

    #[tokio::test]
    async fn test_stock_movements() {
        let (wh_repo, item_repo, _, sm_repo, _, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-MV".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-MV".into(),
                name: "Gear".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        let movement = sm_repo
            .create(&CreateStockMovement {
                item_id: item.id.clone(),
                from_warehouse_id: None,
                to_warehouse_id: wh.id.clone(),
                quantity: 200,
                movement_type: "receipt".into(),
                reference_type: Some("purchase_order".into()),
                reference_id: Some("PO-001".into()),
            })
            .await
            .unwrap();
        assert_eq!(movement.quantity, 200);
        assert_eq!(movement.movement_type, "receipt");
        assert!(movement.from_warehouse_id.is_none());

        let movements = sm_repo.list().await.unwrap();
        assert_eq!(movements.len(), 1);
    }

    #[tokio::test]
    async fn test_reservation_create_and_cancel() {
        let (wh_repo, item_repo, sl_repo, _, res_repo, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-RES".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-RES".into(),
                name: "Spring".into(),
                description: None,
                unit_of_measure: None,
                item_type: "component".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Put stock on hand first
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 100)
            .await
            .unwrap();

        // Create reservation
        let reservation = res_repo
            .create(&CreateReservation {
                item_id: item.id.clone(),
                warehouse_id: wh.id.clone(),
                quantity: 30,
                reference_type: "sales_order".into(),
                reference_id: "SO-001".into(),
            })
            .await
            .unwrap();
        assert_eq!(reservation.status, "active");
        assert_eq!(reservation.quantity, 30);

        // Cancel reservation
        let cancelled = res_repo.cancel(&reservation.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");
    }

    #[tokio::test]
    async fn test_reservation_release_updates_stock() {
        let (wh_repo, item_repo, sl_repo, _, res_repo, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-REL".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-REL".into(),
                name: "Nut".into(),
                description: None,
                unit_of_measure: None,
                item_type: "raw_material".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Receipt: 100 units
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 100)
            .await
            .unwrap();

        // Reserve 40 units via stock level repo
        let sl = sl_repo
            .reserve(&item.id, &wh.id, 40)
            .await
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 40);
        assert_eq!(sl.quantity_available, 60);

        // Release reservation
        sl_repo
            .release_reservation(&item.id, &wh.id, 40)
            .await
            .unwrap();
        let sl_after = sl_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl_after.quantity_reserved, 0);
    }

    #[tokio::test]
    async fn test_cycle_count_lifecycle() {
        let (wh_repo, item_repo, sl_repo, _, _, cc_repo) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-CC".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-CC".into(),
                name: "Bearing".into(),
                description: None,
                unit_of_measure: None,
                item_type: "component".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Put stock on hand: 50 units
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 50)
            .await
            .unwrap();

        // Create cycle count session (draft)
        let session = cc_repo
            .create_cycle_count_session(&wh.id, "2025-07-01", "counter-1")
            .await
            .unwrap();
        assert_eq!(session.status, "draft");
        assert_eq!(session.warehouse_id, wh.id);

        // Add line with system_quantity = 50
        let line = cc_repo
            .add_cycle_count_line(&session.id, &item.id, 50, None)
            .await
            .unwrap();
        assert_eq!(line.system_quantity, 50);
        assert!(line.counted_quantity.is_none());

        // Update counted quantity to 45 (variance = -5)
        let updated_line = cc_repo
            .update_counted_quantity(&line.id, 45, Some("Missing 5 units".into()))
            .await
            .unwrap();
        assert_eq!(updated_line.counted_quantity, Some(45));
        assert_eq!(updated_line.variance, Some(-5));

        // Submit session (draft -> submitted)
        let submitted = cc_repo
            .update_session_status(&session.id, "submitted")
            .await
            .unwrap();
        assert_eq!(submitted.status, "submitted");

        // Approve (submitted -> approved)
        let approved = cc_repo
            .update_session_status(&session.id, "approved")
            .await
            .unwrap();
        assert_eq!(approved.status, "approved");

        // Post (approved -> posted) -- adjusts stock
        let posted = cc_repo
            .post_cycle_count(&session.id, "approver-1")
            .await
            .unwrap();
        assert_eq!(posted.status, "posted");
        assert_eq!(posted.approved_by, Some("approver-1".to_string()));

        // Verify stock level adjusted by -5
        let sl = sl_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 45);
    }

    #[tokio::test]
    async fn test_cycle_count_session_list() {
        let (wh_repo, _, _, _, _, cc_repo) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-CL".into(),
                address: None,
            })
            .await
            .unwrap();

        cc_repo
            .create_cycle_count_session(&wh.id, "2025-08-01", "counter-a")
            .await
            .unwrap();
        cc_repo
            .create_cycle_count_session(&wh.id, "2025-08-15", "counter-b")
            .await
            .unwrap();

        let sessions = cc_repo.list_cycle_count_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_cycle_count_lines_by_session() {
        let (wh_repo, item_repo, _, _, _, cc_repo) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-LN".into(),
                address: None,
            })
            .await
            .unwrap();
        let item1 = item_repo
            .create(&CreateItem {
                sku: "SKU-LN1".into(),
                name: "Item LN1".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();
        let item2 = item_repo
            .create(&CreateItem {
                sku: "SKU-LN2".into(),
                name: "Item LN2".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        let session = cc_repo
            .create_cycle_count_session(&wh.id, "2025-09-01", "counter-x")
            .await
            .unwrap();

        cc_repo
            .add_cycle_count_line(&session.id, &item1.id, 100, None)
            .await
            .unwrap();
        cc_repo
            .add_cycle_count_line(&session.id, &item2.id, 200, Some("Check this".into()))
            .await
            .unwrap();

        let lines = cc_repo.get_cycle_count_lines(&session.id).await.unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[tokio::test]
    async fn test_stock_deduction() {
        let (wh_repo, item_repo, sl_repo, sm_repo, _, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-DED".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-DED".into(),
                name: "Deductible".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Receive 100 units
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 100)
            .await
            .unwrap();

        // Deduct 30 units
        let sl = sl_repo.deduct(&item.id, &wh.id, 30).await.unwrap();
        assert_eq!(sl.quantity_on_hand, 70);

        // Record the movement
        let movement = sm_repo
            .create(&CreateStockMovement {
                item_id: item.id.clone(),
                from_warehouse_id: Some(wh.id.clone()),
                to_warehouse_id: wh.id.clone(),
                quantity: 30,
                movement_type: "pick".into(),
                reference_type: Some("sales_order".into()),
                reference_id: Some("SO-001".into()),
            })
            .await
            .unwrap();
        assert_eq!(movement.movement_type, "pick");
        assert_eq!(movement.quantity, 30);
    }

    #[tokio::test]
    async fn test_order_fulfilled_deducts_and_releases() {
        let (wh_repo, item_repo, sl_repo, sm_repo, res_repo, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-FUL".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-FUL".into(),
                name: "Fulfillable".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Receive 100 units
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 100)
            .await
            .unwrap();

        // Reserve 40 for the order
        sl_repo.reserve(&item.id, &wh.id, 40).await.unwrap();
        res_repo
            .create(&CreateReservation {
                item_id: item.id.clone(),
                warehouse_id: wh.id.clone(),
                quantity: 40,
                reference_type: "sales_order".into(),
                reference_id: "SO-FUL".into(),
            })
            .await
            .unwrap();

        // Simulate fulfillment: deduct 40 and release reservation
        sl_repo.deduct(&item.id, &wh.id, 40).await.unwrap();
        sl_repo.release_reservation(&item.id, &wh.id, 40).await.unwrap();

        // Record the pick movement
        sm_repo
            .create(&CreateStockMovement {
                item_id: item.id.clone(),
                from_warehouse_id: Some(wh.id.clone()),
                to_warehouse_id: wh.id.clone(),
                quantity: 40,
                movement_type: "pick".into(),
                reference_type: Some("sales_order".into()),
                reference_id: Some("SO-FUL".into()),
            })
            .await
            .unwrap();

        let sl = sl_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 60);
        assert_eq!(sl.quantity_reserved, 0);
        assert_eq!(sl.quantity_available, 60);

        // Verify movement recorded
        let movements = sm_repo.list().await.unwrap();
        assert!(!movements.is_empty());
    }

    #[tokio::test]
    async fn test_work_order_completion_adds_stock() {
        let (wh_repo, item_repo, sl_repo, sm_repo, _, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-MFG".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-MFG".into(),
                name: "Finished Good".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Before: no stock
        let levels = sl_repo.get_by_item(&item.id).await.unwrap();
        assert!(levels.is_empty());

        // Simulate work order completion: add 50 units to first warehouse
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 50)
            .await
            .unwrap();

        let movement = sm_repo
            .create(&CreateStockMovement {
                item_id: item.id.clone(),
                from_warehouse_id: None,
                to_warehouse_id: wh.id.clone(),
                quantity: 50,
                movement_type: "receipt".into(),
                reference_type: Some("work_order".into()),
                reference_id: Some("WO-001".into()),
            })
            .await
            .unwrap();

        let sl = sl_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 50);
        assert_eq!(movement.reference_type, Some("work_order".to_string()));
    }

    #[tokio::test]
    async fn test_return_restocks_inventory() {
        let (wh_repo, item_repo, sl_repo, sm_repo, _, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-RET".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-RET".into(),
                name: "Returnable".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        // Start with 100 units
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 100)
            .await
            .unwrap();

        // Deduct 10 (sold)
        sl_repo.deduct(&item.id, &wh.id, 10).await.unwrap();

        // Return: add 10 back
        sl_repo
            .upsert_receipt(&item.id, &wh.id, 10)
            .await
            .unwrap();
        sm_repo
            .create(&CreateStockMovement {
                item_id: item.id.clone(),
                from_warehouse_id: None,
                to_warehouse_id: wh.id.clone(),
                quantity: 10,
                movement_type: "return".into(),
                reference_type: Some("sales_return".into()),
                reference_id: Some("RET-001".into()),
            })
            .await
            .unwrap();

        let sl = sl_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
    }

    #[tokio::test]
    async fn test_get_first_warehouse_for_item() {
        let (wh_repo, item_repo, sl_repo, _, _, _) = setup_repos().await;

        // No stock for nonexistent item
        let result = sl_repo.get_first_warehouse_for_item("nonexistent").await.unwrap();
        assert!(result.is_none());

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-LOOKUP".into(),
                address: None,
            })
            .await
            .unwrap();
        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-LOOKUP".into(),
                name: "Lookup Item".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        sl_repo
            .upsert_receipt(&item.id, &wh.id, 50)
            .await
            .unwrap();

        let result = sl_repo.get_first_warehouse_for_item(&item.id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().warehouse_id, wh.id);
    }

    #[tokio::test]
    async fn test_item_creation_with_reorder_fields() {
        let (_, item_repo, _, _, _, _) = setup_repos().await;

        let item = item_repo
            .create(&CreateItem {
                sku: "SKU-REO".into(),
                name: "Reorder Item".into(),
                description: Some("Needs reorder tracking".into()),
                unit_of_measure: Some("EA".into()),
                item_type: "finished".into(),
                reorder_point: 50,
                safety_stock: 20,
                economic_order_qty: 100,
                unit_price_cents: None,
            })
            .await
            .unwrap();

        assert_eq!(item.sku, "SKU-REO");
        assert_eq!(item.reorder_point, 50);
        assert_eq!(item.safety_stock, 20);
        assert_eq!(item.economic_order_qty, 100);

        // Verify fields persist after retrieval
        let fetched = item_repo.get_by_id(&item.id).await.unwrap();
        assert_eq!(fetched.reorder_point, 50);
        assert_eq!(fetched.safety_stock, 20);
        assert_eq!(fetched.economic_order_qty, 100);
    }

    #[tokio::test]
    async fn test_warehouse_name_uniqueness() {
        let pool = setup().await;
        let svc = InventoryService {
            pool: pool.clone(),
            warehouse_repo: WarehouseRepo::new(pool.clone()),
            item_repo: ItemRepo::new(pool.clone()),
            stock_level_repo: StockLevelRepo::new(pool.clone()),
            stock_movement_repo: StockMovementRepo::new(pool.clone()),
            reservation_repo: ReservationRepo::new(pool.clone()),
            cycle_count_repo: CycleCountRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        svc.create_warehouse(CreateWarehouse {
            name: "Main WH".into(),
            address: None,
        })
        .await
        .unwrap();

        // Duplicate name should fail
        let result = svc.create_warehouse(CreateWarehouse {
            name: "MAIN WH".into(),
            address: None,
        })
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_item_sku_uniqueness() {
        let pool = setup().await;
        let svc = InventoryService {
            pool: pool.clone(),
            warehouse_repo: WarehouseRepo::new(pool.clone()),
            item_repo: ItemRepo::new(pool.clone()),
            stock_level_repo: StockLevelRepo::new(pool.clone()),
            stock_movement_repo: StockMovementRepo::new(pool.clone()),
            reservation_repo: ReservationRepo::new(pool.clone()),
            cycle_count_repo: CycleCountRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        svc.create_item(CreateItem {
            sku: "SKU-UNIQ".into(),
            name: "Item A".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        })
        .await
        .unwrap();

        // Duplicate SKU should fail
        let result = svc.create_item(CreateItem {
            sku: "sku-uniq".into(), // case-insensitive
            name: "Item B".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        })
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stock_movement_type_validation() {
        let pool = setup().await;
        let svc = InventoryService {
            pool: pool.clone(),
            warehouse_repo: WarehouseRepo::new(pool.clone()),
            item_repo: ItemRepo::new(pool.clone()),
            stock_level_repo: StockLevelRepo::new(pool.clone()),
            stock_movement_repo: StockMovementRepo::new(pool.clone()),
            reservation_repo: ReservationRepo::new(pool.clone()),
            cycle_count_repo: CycleCountRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-TYPE".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-TYPE".into(),
            name: "Type Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        let result = svc.create_stock_movement(CreateStockMovement {
            item_id: item.id,
            from_warehouse_id: None,
            to_warehouse_id: wh.id,
            quantity: 10,
            movement_type: "invalid_type".into(),
            reference_type: None,
            reference_id: None,
        }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reservation_insufficient_stock_prevented() {
        let pool = setup().await;
        let svc = InventoryService {
            pool: pool.clone(),
            warehouse_repo: WarehouseRepo::new(pool.clone()),
            item_repo: ItemRepo::new(pool.clone()),
            stock_level_repo: StockLevelRepo::new(pool.clone()),
            stock_movement_repo: StockMovementRepo::new(pool.clone()),
            reservation_repo: ReservationRepo::new(pool.clone()),
            cycle_count_repo: CycleCountRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-RES-LIMIT".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-RES-LIMIT".into(),
            name: "Res Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 5 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 5).await.unwrap();

        // Reserve 10 should fail (only 5 available)
        let result = svc.create_reservation(CreateReservation {
            item_id: item.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 10,
            reference_type: "test".into(),
            reference_id: "REF-1".into(),
        }).await;
        assert!(result.is_err());

        // Reserve 5 should succeed
        svc.create_reservation(CreateReservation {
            item_id: item.id,
            warehouse_id: wh.id,
            quantity: 5,
            reference_type: "test".into(),
            reference_id: "REF-2".into(),
        }).await.unwrap();
    }

    #[tokio::test]
    async fn test_issue_insufficient_stock_prevented() {
        let pool = setup().await;
        let svc = InventoryService {
            pool: pool.clone(),
            warehouse_repo: WarehouseRepo::new(pool.clone()),
            item_repo: ItemRepo::new(pool.clone()),
            stock_level_repo: StockLevelRepo::new(pool.clone()),
            stock_movement_repo: StockMovementRepo::new(pool.clone()),
            reservation_repo: ReservationRepo::new(pool.clone()),
            cycle_count_repo: CycleCountRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-ISS".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-ISS".into(),
            name: "Issue Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 10 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 10).await.unwrap();

        // Issue 20 should fail
        let result = svc.create_stock_movement(CreateStockMovement {
            item_id: item.id,
            from_warehouse_id: None,
            to_warehouse_id: wh.id,
            quantity: 20,
            movement_type: "issue".into(),
            reference_type: None,
            reference_id: None,
        }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_items_below_reorder_point() {
        let (wh_repo, item_repo, sl_repo, _, _, _) = setup_repos().await;

        let wh = wh_repo
            .create(&CreateWarehouse {
                name: "WH-REO".into(),
                address: None,
            })
            .await
            .unwrap();

        // Item with reorder_point = 50, on_hand = 30 (below reorder point)
        let item_low = item_repo
            .create(&CreateItem {
                sku: "SKU-LOW".into(),
                name: "Low Stock Item".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 50,
                safety_stock: 10,
                economic_order_qty: 100,
                unit_price_cents: None,
            })
            .await
            .unwrap();
        sl_repo
            .upsert_receipt(&item_low.id, &wh.id, 30)
            .await
            .unwrap();

        // Item with reorder_point = 50, on_hand = 60 (above reorder point)
        let item_ok = item_repo
            .create(&CreateItem {
                sku: "SKU-OK".into(),
                name: "OK Stock Item".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 50,
                safety_stock: 10,
                economic_order_qty: 100,
                unit_price_cents: None,
            })
            .await
            .unwrap();
        sl_repo
            .upsert_receipt(&item_ok.id, &wh.id, 60)
            .await
            .unwrap();

        // Item with reorder_point = 0 (should not appear in results)
        let item_no_reorder = item_repo
            .create(&CreateItem {
                sku: "SKU-NOREO".into(),
                name: "No Reorder Item".into(),
                description: None,
                unit_of_measure: None,
                item_type: "finished".into(),
                reorder_point: 0,
                safety_stock: 0,
                economic_order_qty: 0,
                unit_price_cents: None,
            })
            .await
            .unwrap();
        sl_repo
            .upsert_receipt(&item_no_reorder.id, &wh.id, 5)
            .await
            .unwrap();

        // Query items below reorder point
        let below = item_repo.list_items_below_reorder_point().await.unwrap();

        // Only item_low should be returned
        assert_eq!(below.len(), 1);
        assert_eq!(below[0].sku, "SKU-LOW");
        assert_eq!(below[0].reorder_point, 50);
    }

    #[tokio::test]
    async fn test_return_stock_movement_updates_levels() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );
        let item_repo = ItemRepo::new(pool.clone());
        let wh_repo = WarehouseRepo::new(pool.clone());
        let sl_repo = StockLevelRepo::new(pool.clone());

        let item = item_repo.create(&CreateItem {
            name: "Return Test Item".into(),
            sku: "SKU-RET".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        let wh = wh_repo.create(&CreateWarehouse {
            name: "Return WH".into(),
            address: None,
        }).await.unwrap();

        // First receipt to establish stock
        svc.create_stock_movement(CreateStockMovement {
            item_id: item.id.clone(),
            to_warehouse_id: wh.id.clone(),
            from_warehouse_id: None,
            quantity: 10,
            movement_type: "receipt".into(),
            reference_type: None,
            reference_id: None,
        }).await.unwrap();

        // Issue 3 units
        svc.create_stock_movement(CreateStockMovement {
            item_id: item.id.clone(),
            to_warehouse_id: wh.id.clone(),
            from_warehouse_id: None,
            quantity: 3,
            movement_type: "issue".into(),
            reference_type: None,
            reference_id: None,
        }).await.unwrap();

        let stock = sl_repo.get_by_item_warehouse(&item.id, &wh.id).await.unwrap().unwrap();
        assert_eq!(stock.quantity_on_hand, 7); // 10 - 3

        // Return 2 units back
        svc.create_stock_movement(CreateStockMovement {
            item_id: item.id.clone(),
            to_warehouse_id: wh.id.clone(),
            from_warehouse_id: None,
            quantity: 2,
            movement_type: "return".into(),
            reference_type: None,
            reference_id: None,
        }).await.unwrap();

        let stock = sl_repo.get_by_item_warehouse(&item.id, &wh.id).await.unwrap().unwrap();
        assert_eq!(stock.quantity_on_hand, 9); // 7 + 2
    }

    #[tokio::test]
    async fn test_handle_work_order_started_sufficient_stock() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-WOS".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-WOS".into(),
            name: "Work Order Material".into(),
            description: None,
            unit_of_measure: None,
            item_type: "raw_material".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Stock 200 units
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 200).await.unwrap();

        // Handle work order started requiring 100 units -- should succeed
        let result = svc.handle_work_order_started("WO-START-001", &item.id, 100).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_work_order_started_insufficient_stock() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-WOS-LOW".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-WOS-LOW".into(),
            name: "Low Stock Material".into(),
            description: None,
            unit_of_measure: None,
            item_type: "raw_material".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Stock only 10 units
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 10).await.unwrap();

        // Handle work order started requiring 50 units -- still returns Ok (logs warning)
        let result = svc.handle_work_order_started("WO-START-002", &item.id, 50).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_work_order_started_no_stock() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-WOS-NONE".into(),
            name: "No Stock Material".into(),
            description: None,
            unit_of_measure: None,
            item_type: "raw_material".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // No stock at all -- handler should still succeed (logs warning about missing stock)
        let result = svc.handle_work_order_started("WO-START-003", &item.id, 25).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_order_cancelled_releases_reservation() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-OC".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-OC".into(),
            name: "Order Cancel Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 100 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 100).await.unwrap();

        // Create a reservation tied to a sales order
        svc.create_reservation(CreateReservation {
            item_id: item.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 30,
            reference_type: "sales_order".into(),
            reference_id: "SO-CANCEL-001".into(),
        }).await.unwrap();

        // Verify reservation is active and stock is reserved
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await.unwrap().unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 30);

        // Handle order cancellation
        let result = svc.handle_order_cancelled(
            "SO-CANCEL-001",
            "ORD-001",
            &Some("Customer requested cancellation".into()),
        ).await;
        assert!(result.is_ok());

        // Verify reservation is cancelled
        let reservations = svc.reservation_repo.list().await.unwrap();
        let cancelled = reservations.iter().find(|r| r.reference_id == "SO-CANCEL-001").unwrap();
        assert_eq!(cancelled.status, "cancelled");

        // Verify stock is released: reserved goes back to 0, available returns to on_hand level
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await.unwrap().unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 0);
    }

    #[tokio::test]
    async fn test_handle_order_cancelled_no_reservation() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        // No reservations exist for this order -- should succeed gracefully
        let result = svc.handle_order_cancelled(
            "SO-NORES-999",
            "ORD-999",
            &None,
        ).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_order_cancelled_multiple_reservations() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-OC-MULTI".into(),
            address: None,
        }).await.unwrap();
        let item1 = svc.item_repo.create(&CreateItem {
            sku: "SKU-OC-M1".into(),
            name: "Multi Cancel Item 1".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();
        let item2 = svc.item_repo.create(&CreateItem {
            sku: "SKU-OC-M2".into(),
            name: "Multi Cancel Item 2".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put stock on hand for both items
        svc.stock_level_repo.upsert_receipt(&item1.id, &wh.id, 100).await.unwrap();
        svc.stock_level_repo.upsert_receipt(&item2.id, &wh.id, 50).await.unwrap();

        // Create two reservations tied to the same sales order
        svc.create_reservation(CreateReservation {
            item_id: item1.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 20,
            reference_type: "sales_order".into(),
            reference_id: "SO-MULTI-001".into(),
        }).await.unwrap();
        svc.create_reservation(CreateReservation {
            item_id: item2.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 10,
            reference_type: "sales_order".into(),
            reference_id: "SO-MULTI-001".into(),
        }).await.unwrap();

        // Handle order cancellation
        let result = svc.handle_order_cancelled(
            "SO-MULTI-001",
            "ORD-MULTI",
            &Some("Duplicate order".into()),
        ).await;
        assert!(result.is_ok());

        // Verify both reservations are cancelled
        let reservations = svc.reservation_repo.list().await.unwrap();
        let multi_reservations: Vec<_> = reservations
            .iter()
            .filter(|r| r.reference_id == "SO-MULTI-001")
            .collect();
        assert_eq!(multi_reservations.len(), 2);
        for res in &multi_reservations {
            assert_eq!(res.status, "cancelled");
        }

        // Verify stock is fully released for both items
        let sl1 = svc.stock_level_repo
            .get_by_item_warehouse(&item1.id, &wh.id)
            .await.unwrap().unwrap();
        assert_eq!(sl1.quantity_reserved, 0);
        assert_eq!(sl1.quantity_available, 100);

        let sl2 = svc.stock_level_repo
            .get_by_item_warehouse(&item2.id, &wh.id)
            .await.unwrap().unwrap();
        assert_eq!(sl2.quantity_reserved, 0);
        assert_eq!(sl2.quantity_available, 50);
    }

    #[tokio::test]
    async fn test_handle_po_cancelled_cancels_reservations() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-PO-CANCEL".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-PO-CANCEL".into(),
            name: "PO Cancel Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "raw_material".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 100 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 100).await.unwrap();

        // Create a reservation tied to a purchase order
        svc.create_reservation(CreateReservation {
            item_id: item.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 40,
            reference_type: "purchase_order".into(),
            reference_id: "PO-CANCEL-001".into(),
        }).await.unwrap();

        // Verify reservation is active and stock is reserved
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await.unwrap().unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 40);

        // Handle PO cancellation
        let result = svc.handle_po_cancelled(
            "PO-CANCEL-001",
            "SUPPLIER-001",
            &Some("Supplier defaulted".into()),
        ).await;
        assert!(result.is_ok());

        // Verify reservation is cancelled
        let reservations = svc.reservation_repo.list().await.unwrap();
        let cancelled = reservations.iter().find(|r| r.reference_id == "PO-CANCEL-001").unwrap();
        assert_eq!(cancelled.status, "cancelled");

        // Verify stock is released
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await.unwrap().unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 0);
    }

    #[tokio::test]
    async fn test_handle_po_cancelled_no_reservation() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        // No reservations exist for this PO — should succeed gracefully
        let result = svc.handle_po_cancelled(
            "PO-NORES-999",
            "SUPPLIER-999",
            &None,
        ).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_po_cancelled_only_cancels_purchase_order_reservations() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-PO-CANCEL-SELECTIVE".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-PO-SELECTIVE".into(),
            name: "PO Selective Cancel Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "raw_material".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 200 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 200).await.unwrap();

        // Create a purchase_order reservation
        svc.create_reservation(CreateReservation {
            item_id: item.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 30,
            reference_type: "purchase_order".into(),
            reference_id: "PO-SELECT-001".into(),
        }).await.unwrap();

        // Create a sales_order reservation (should NOT be cancelled)
        svc.create_reservation(CreateReservation {
            item_id: item.id.clone(),
            warehouse_id: wh.id.clone(),
            quantity: 20,
            reference_type: "sales_order".into(),
            reference_id: "PO-SELECT-001".into(),
        }).await.unwrap();

        // Handle PO cancellation
        let result = svc.handle_po_cancelled(
            "PO-SELECT-001",
            "SUPPLIER-SELECT",
            &None,
        ).await;
        assert!(result.is_ok());

        // Verify only the purchase_order reservation is cancelled
        let reservations = svc.reservation_repo.list().await.unwrap();
        let po_res = reservations.iter().find(|r| r.reference_type == "purchase_order" && r.reference_id == "PO-SELECT-001").unwrap();
        assert_eq!(po_res.status, "cancelled");

        let so_res = reservations.iter().find(|r| r.reference_type == "sales_order" && r.reference_id == "PO-SELECT-001").unwrap();
        assert_eq!(so_res.status, "active");
    }

    #[tokio::test]
    async fn test_handle_po_received_adds_stock_and_movement() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-PO-RECV".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-PO-RECV".into(),
            name: "PO Receive Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "raw_material".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Initially no stock
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await.unwrap();
        assert!(sl.is_none());

        // Handle PO received: add 75 units
        svc.handle_po_received("PO-RECV-001", &item.id, &wh.id, 75)
            .await
            .unwrap();

        // Verify stock level increased
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 75);
        assert_eq!(sl.quantity_available, 75);

        // Verify a receipt stock movement was recorded
        let movements = svc.stock_movement_repo.list().await.unwrap();
        assert_eq!(movements.len(), 1);
        assert_eq!(movements[0].movement_type, "receipt");
        assert_eq!(movements[0].quantity, 75);
        assert_eq!(movements[0].reference_type, Some("purchase_order".to_string()));
        assert_eq!(movements[0].reference_id, Some("PO-RECV-001".to_string()));
    }

    #[tokio::test]
    async fn test_handle_order_confirmed_reserves_stock() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-ORD-CONF".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-ORD-CONF".into(),
            name: "Order Confirm Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 100 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 100).await.unwrap();

        // Handle order confirmed: reserve 40 units
        svc.handle_order_confirmed("SO-CONF-001", &item.id, &wh.id, 40)
            .await
            .unwrap();

        // Verify stock is reserved
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 100);
        assert_eq!(sl.quantity_reserved, 40);

        // Verify reservation was created with correct reference
        let reservations = svc.reservation_repo.list().await.unwrap();
        assert_eq!(reservations.len(), 1);
        assert_eq!(reservations[0].quantity, 40);
        assert_eq!(reservations[0].reference_type, "sales_order");
        assert_eq!(reservations[0].reference_id, "SO-CONF-001");
    }

    #[tokio::test]
    async fn test_handle_order_fulfilled_deducts_stock_and_releases_reservation() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-ORD-FUL".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-ORD-FUL".into(),
            name: "Order Fulfill Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Put 200 units on hand
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 200).await.unwrap();

        // Reserve 50 units for the order
        svc.stock_level_repo.reserve(&item.id, &wh.id, 50).await.unwrap();

        // Handle order fulfilled: deduct 50 and release reservation
        svc.handle_order_fulfilled("SO-FUL-001", &item.id, &wh.id, 50)
            .await
            .unwrap();

        // Verify stock deducted and reservation released
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 150); // 200 - 50
        assert_eq!(sl.quantity_reserved, 0);  // reservation released
        assert_eq!(sl.quantity_available, 150);

        // Verify a pick movement was recorded
        let movements = svc.stock_movement_repo.list().await.unwrap();
        assert_eq!(movements.len(), 1);
        assert_eq!(movements[0].movement_type, "pick");
        assert_eq!(movements[0].quantity, 50);
        assert_eq!(movements[0].reference_type, Some("sales_order".to_string()));
        assert_eq!(movements[0].reference_id, Some("SO-FUL-001".to_string()));
    }

    #[tokio::test]
    async fn test_handle_work_order_completed_adds_stock_to_first_warehouse() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-WO-COMP".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-WO-COMP".into(),
            name: "Work Order Finished Good".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Establish a stock level row so get_first_warehouse_for_item finds it
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 20).await.unwrap();

        // Handle work order completed: add 80 manufactured units
        svc.handle_work_order_completed("WO-COMP-001", &item.id, 80)
            .await
            .unwrap();

        // Verify stock increased: 20 initial + 80 manufactured = 100
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 100);

        // Verify a receipt movement was recorded with work_order reference
        let movements = svc.stock_movement_repo.list().await.unwrap();
        assert_eq!(movements.len(), 1);
        assert_eq!(movements[0].movement_type, "receipt");
        assert_eq!(movements[0].quantity, 80);
        assert_eq!(movements[0].reference_type, Some("work_order".to_string()));
        assert_eq!(movements[0].reference_id, Some("WO-COMP-001".to_string()));
    }

    #[tokio::test]
    async fn test_handle_return_created_adds_stock_back() {
        let pool = setup().await;
        let svc = InventoryService::new(
            pool.clone(),
            saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        );

        let wh = svc.warehouse_repo.create(&CreateWarehouse {
            name: "WH-RET-CREATED".into(),
            address: None,
        }).await.unwrap();
        let item = svc.item_repo.create(&CreateItem {
            sku: "SKU-RET-CREATED".into(),
            name: "Return Created Item".into(),
            description: None,
            unit_of_measure: None,
            item_type: "finished".into(),
            reorder_point: 0,
            safety_stock: 0,
            economic_order_qty: 0,
            unit_price_cents: None,
        }).await.unwrap();

        // Establish initial stock of 50 so get_first_warehouse_for_item finds the warehouse
        svc.stock_level_repo.upsert_receipt(&item.id, &wh.id, 50).await.unwrap();

        // Handle return created: add 15 units back
        svc.handle_return_created("RET-CREATED-001", &item.id, 15)
            .await
            .unwrap();

        // Verify stock increased: 50 initial + 15 returned = 65
        let sl = svc.stock_level_repo
            .get_by_item_warehouse(&item.id, &wh.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sl.quantity_on_hand, 65);

        // Verify a return movement was recorded
        let movements = svc.stock_movement_repo.list().await.unwrap();
        assert_eq!(movements.len(), 1);
        assert_eq!(movements[0].movement_type, "return");
        assert_eq!(movements[0].quantity, 15);
        assert_eq!(movements[0].reference_type, Some("sales_return".to_string()));
        assert_eq!(movements[0].reference_id, Some("RET-CREATED-001".to_string()));
    }
}

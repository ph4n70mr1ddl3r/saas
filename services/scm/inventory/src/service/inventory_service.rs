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
        self.warehouse_repo.create(&input).await
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
        self.item_repo.create(&input).await
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
        let movement = self.stock_movement_repo.create(&input).await?;
        // Update stock levels based on movement type
        if input.movement_type == "receipt" {
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
        self.cycle_count_repo
            .post_cycle_count(id, approved_by)
            .await
    }
}

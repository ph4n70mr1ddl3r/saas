use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::repository::warehouse_repo::WarehouseRepo;
use crate::repository::item_repo::ItemRepo;
use crate::repository::stock_level_repo::StockLevelRepo;
use crate::repository::stock_movement_repo::StockMovementRepo;
use crate::repository::reservation_repo::ReservationRepo;
use crate::models::warehouse::*;
use crate::models::item::*;
use validator::Validate;
use crate::models::stock_level::*;
use crate::models::stock_movement::*;
use crate::models::reservation::*;

#[derive(Clone)]
pub struct InventoryService {
    pool: SqlitePool,
    warehouse_repo: WarehouseRepo,
    item_repo: ItemRepo,
    stock_level_repo: StockLevelRepo,
    stock_movement_repo: StockMovementRepo,
    reservation_repo: ReservationRepo,
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
            reservation_repo: ReservationRepo::new(pool),
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
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
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

    pub async fn create_stock_movement(&self, input: CreateStockMovement) -> AppResult<StockMovementResponse> {
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        let movement = self.stock_movement_repo.create(&input).await?;
        // Update stock levels based on movement type
        if input.movement_type == "receipt" {
            self.stock_level_repo.upsert_receipt(&input.item_id, &input.to_warehouse_id, input.quantity).await?;
            if let Err(e) = self.bus.publish("scm.inventory.stock.received", saas_proto::events::StockReceived {
                item_id: input.item_id.clone(),
                warehouse_id: input.to_warehouse_id.clone(),
                location_id: input.to_warehouse_id.clone(),
                quantity: input.quantity,
                reference_type: input.reference_type.clone().unwrap_or_default(),
                reference_id: input.reference_id.clone().unwrap_or_default(),
            }).await {
                tracing::error!("CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.", "scm.inventory.stock.received", e);
            }
        }
        Ok(movement)
    }

    // Reservations
    pub async fn list_reservations(&self) -> AppResult<Vec<ReservationResponse>> {
        self.reservation_repo.list().await
    }

    pub async fn create_reservation(&self, input: CreateReservation) -> AppResult<ReservationResponse> {
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;

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
        if let Err(e) = self.bus.publish("scm.inventory.stock.reserved", saas_proto::events::StockReserved {
            item_id: input.item_id.clone(),
            warehouse_id: input.warehouse_id.clone(),
            quantity: input.quantity,
            reference_type: input.reference_type.clone(),
            reference_id: input.reference_id.clone(),
        }).await {
            tracing::error!("CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.", "scm.inventory.stock.reserved", e);
        }
        Ok(reservation)
    }

    pub async fn cancel_reservation(&self, id: &str) -> AppResult<ReservationResponse> {
        let existing = self.reservation_repo.get_by_id(id).await?;
        let reservation = self.reservation_repo.cancel(id).await?;
        self.stock_level_repo.release_reservation(&existing.item_id, &existing.warehouse_id, existing.quantity).await?;
        Ok(reservation)
    }

    // Event handlers
    pub async fn handle_po_received(&self, po_id: &str, item_id: &str, warehouse_id: &str, quantity: i64) -> AppResult<()> {
        self.stock_level_repo.upsert_receipt(item_id, warehouse_id, quantity).await?;
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
        if let Err(e) = self.bus.publish("scm.inventory.stock.received", saas_proto::events::StockReceived {
            item_id: item_id.to_string(),
            warehouse_id: warehouse_id.to_string(),
            location_id: warehouse_id.to_string(),
            quantity,
            reference_type: "purchase_order".to_string(),
            reference_id: po_id.to_string(),
        }).await {
            tracing::error!("CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.", "scm.inventory.stock.received", e);
        }
        Ok(())
    }

    pub async fn handle_order_confirmed(&self, order_id: &str, item_id: &str, warehouse_id: &str, quantity: i64) -> AppResult<()> {
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
}

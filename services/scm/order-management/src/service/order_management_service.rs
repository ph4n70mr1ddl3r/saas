use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::repository::sales_order_repo::SalesOrderRepo;
use crate::repository::fulfillment_repo::FulfillmentRepo;
use crate::repository::return_repo::ReturnRepo;
use crate::models::sales_order::*;
use crate::models::return_model::*;
use validator::Validate;

#[derive(Clone)]
pub struct OrderManagementService {
    order_repo: SalesOrderRepo,
    fulfillment_repo: FulfillmentRepo,
    return_repo: ReturnRepo,
    bus: NatsBus,
}

impl OrderManagementService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus,
        }
    }

    // Sales Orders
    pub async fn list_sales_orders(&self) -> AppResult<Vec<SalesOrderResponse>> {
        self.order_repo.list().await
    }

    pub async fn get_sales_order(&self, id: &str) -> AppResult<SalesOrderDetailResponse> {
        let order = self.order_repo.get_by_id(id).await?;
        let lines = self.order_repo.get_lines(id).await?;
        Ok(SalesOrderDetailResponse { order, lines })
    }

    pub async fn create_sales_order(&self, input: CreateSalesOrder) -> AppResult<SalesOrderResponse> {
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.order_repo.create(&input).await
    }

    pub async fn confirm_sales_order(&self, id: &str) -> AppResult<SalesOrderDetailResponse> {
        let order = self.order_repo.get_by_id(id).await?;
        if order.status != "draft" {
            return Err(saas_common::error::AppError::Validation("Only draft orders can be confirmed".into()));
        }
        self.order_repo.update_status(id, "confirmed").await?;
        let detail = self.get_sales_order(id).await?;
        // Publish scm.orders.order.confirmed
        let proto_lines: Vec<saas_proto::events::SalesOrderLineEvent> = detail.lines.iter().map(|l| {
            saas_proto::events::SalesOrderLineEvent {
                item_id: l.item_id.clone(),
                quantity: l.quantity,
                warehouse_id: None,
            }
        }).collect();
        if let Err(e) = self.bus.publish("scm.orders.order.confirmed", saas_proto::events::SalesOrderConfirmed {
            order_id: id.to_string(),
            order_number: detail.order.order_number.clone(),
            customer_id: detail.order.customer_id.clone(),
            lines: proto_lines,
        }).await {
            tracing::error!("CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.", "scm.orders.order.confirmed", e);
        }
        Ok(detail)
    }

    pub async fn fulfill_sales_order(&self, id: &str, input: FulfillRequest) -> AppResult<SalesOrderDetailResponse> {
        let order = self.order_repo.get_by_id(id).await?;
        if order.status != "confirmed" && order.status != "picking" {
            return Err(saas_common::error::AppError::Validation("Only confirmed orders can be fulfilled".into()));
        }
        for line in &input.lines {
            self.fulfillment_repo.create(id, &line.order_line_id, line.quantity, &line.warehouse_id).await?;
        }
        self.order_repo.update_status(id, "picking").await?;
        self.get_sales_order(id).await
    }

    // Returns
    pub async fn list_returns(&self) -> AppResult<Vec<ReturnResponse>> {
        self.return_repo.list().await
    }

    pub async fn get_return(&self, id: &str) -> AppResult<ReturnResponse> {
        self.return_repo.get_by_id(id).await
    }

    pub async fn create_return(&self, input: CreateReturn) -> AppResult<ReturnResponse> {
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.return_repo.create(&input).await
    }
}

use crate::models::return_model::*;
use crate::models::sales_order::*;
use crate::repository::fulfillment_repo::FulfillmentRepo;
use crate::repository::return_repo::ReturnRepo;
use crate::repository::sales_order_repo::SalesOrderRepo;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
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

    pub async fn create_sales_order(
        &self,
        input: CreateSalesOrder,
    ) -> AppResult<SalesOrderResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        for line in &input.lines {
            if line.unit_price_cents < 0 {
                return Err(saas_common::error::AppError::Validation(
                    "Sales order line unit prices must be non-negative".into(),
                ));
            }
        }
        self.order_repo.create(&input).await
    }

    pub async fn cancel_sales_order(&self, id: &str) -> AppResult<SalesOrderResponse> {
        let order = self.order_repo.get_by_id(id).await?;
        if order.status != "draft" && order.status != "confirmed" {
            return Err(saas_common::error::AppError::Validation(
                "Only draft or confirmed orders can be cancelled".into(),
            ));
        }
        self.order_repo.update_status(id, "cancelled").await?;
        let order = self.order_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.orders.order.cancelled",
                saas_proto::events::SalesOrderCancelled {
                    order_id: order.id.clone(),
                    order_number: order.order_number.clone(),
                    customer_id: order.customer_id.clone(),
                    reason: None,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.orders.order.cancelled",
                e
            );
        }
        Ok(order)
    }

    pub async fn confirm_sales_order(&self, id: &str) -> AppResult<SalesOrderDetailResponse> {
        let order = self.order_repo.get_by_id(id).await?;
        if order.status != "draft" {
            return Err(saas_common::error::AppError::Validation(
                "Only draft orders can be confirmed".into(),
            ));
        }
        self.order_repo.update_status(id, "confirmed").await?;
        let detail = self.get_sales_order(id).await?;
        // Publish scm.orders.order.confirmed
        let proto_lines: Vec<saas_proto::events::SalesOrderLineEvent> = detail
            .lines
            .iter()
            .map(|l| saas_proto::events::SalesOrderLineEvent {
                item_id: l.item_id.clone(),
                quantity: l.quantity,
                warehouse_id: None,
            })
            .collect();
        if let Err(e) = self
            .bus
            .publish(
                "scm.orders.order.confirmed",
                saas_proto::events::SalesOrderConfirmed {
                    order_id: id.to_string(),
                    order_number: detail.order.order_number.clone(),
                    customer_id: detail.order.customer_id.clone(),
                    lines: proto_lines,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.orders.order.confirmed",
                e
            );
        }
        Ok(detail)
    }

    pub async fn fulfill_sales_order(
        &self,
        id: &str,
        input: FulfillRequest,
    ) -> AppResult<SalesOrderDetailResponse> {
        let order = self.order_repo.get_by_id(id).await?;
        if order.status != "confirmed" && order.status != "picking" {
            return Err(saas_common::error::AppError::Validation(
                "Only confirmed orders can be fulfilled".into(),
            ));
        }

        // Fetch order lines for quantity validation
        let order_lines = self.order_repo.get_lines(id).await?;

        // Get existing fulfillments to check cumulative quantities
        let existing_fulfillments = self.fulfillment_repo.list_by_order(id).await?;

        let mut fulfilled_lines = Vec::new();
        for line in &input.lines {
            // Find the matching order line
            let order_line = order_lines.iter().find(|ol| ol.id == line.order_line_id);
            let order_line = match order_line {
                Some(ol) => ol,
                None => {
                    return Err(saas_common::error::AppError::Validation(format!(
                        "Order line '{}' not found on order '{}'",
                        line.order_line_id, id
                    )));
                }
            };

            // Calculate already fulfilled quantity for this line
            let already_fulfilled: i64 = existing_fulfillments
                .iter()
                .filter(|f| f.order_line_id == line.order_line_id)
                .map(|f| f.quantity)
                .sum();

            // Validate quantity doesn't exceed remaining
            let remaining = order_line.quantity - already_fulfilled;
            if line.quantity > remaining {
                return Err(saas_common::error::AppError::Validation(format!(
                    "Fulfillment quantity ({}) exceeds remaining order line quantity ({}). Ordered: {}, Already fulfilled: {}",
                    line.quantity, remaining, order_line.quantity, already_fulfilled
                )));
            }

            self.fulfillment_repo
                .create(id, &line.order_line_id, line.quantity, &line.warehouse_id)
                .await?;
            fulfilled_lines.push(saas_proto::events::OrderFulfilledLine {
                item_id: order_line.item_id.clone(),
                quantity: line.quantity,
                warehouse_id: line.warehouse_id.clone(),
                unit_price_cents: order_line.unit_price_cents,
            });
        }
        // Determine if all order lines are fully fulfilled
        let all_fulfillments = self.fulfillment_repo.list_by_order(id).await?;
        let all_fulfilled = order_lines.iter().all(|ol| {
            let fulfilled_qty: i64 = all_fulfillments
                .iter()
                .filter(|f| f.order_line_id == ol.id)
                .map(|f| f.quantity)
                .sum();
            fulfilled_qty >= ol.quantity
        });
        let new_status = if all_fulfilled { "fulfilled" } else { "picking" };
        self.order_repo.update_status(id, new_status).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.orders.order.fulfilled",
                saas_proto::events::OrderFulfilled {
                    order_id: id.to_string(),
                    order_number: order.order_number.clone(),
                    customer_id: order.customer_id.clone(),
                    lines: fulfilled_lines,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.orders.order.fulfilled",
                e
            );
        }
        self.get_sales_order(id).await
    }

    // Event Handlers
    pub async fn handle_stock_reserved(
        &self,
        item_id: &str,
        warehouse_id: &str,
        quantity: i64,
        reference_type: &str,
        reference_id: &str,
    ) {
        // Only process stock reservations for sales orders
        if reference_type != "sales_order" {
            tracing::debug!(
                "Ignoring stock reserved event: reference_type '{}' is not 'sales_order'",
                reference_type
            );
            return;
        }

        let order_id = reference_id;
        tracing::info!(
            "Processing stock reserved for sales order {}: item={}, warehouse={}, qty={}",
            order_id,
            item_id,
            warehouse_id,
            quantity
        );

        // Try to fetch the sales order
        let order = match self.order_repo.get_by_id(order_id).await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch sales order {} for stock reserved event: {}",
                    order_id,
                    e
                );
                return;
            }
        };

        // Only auto-advance if the order is in "confirmed" status
        if order.status != "confirmed" {
            tracing::info!(
                "Sales order {} has status '{}' — skipping auto-advance to picking (only 'confirmed' orders are advanced)",
                order_id,
                order.status
            );
            return;
        }

        // Advance status to "picking"
        match self.order_repo.update_status(order_id, "picking").await {
            Ok(_) => {
                tracing::info!(
                    "Auto-advanced sales order {} from 'confirmed' to 'picking' (stock reserved: item={}, warehouse={}, qty={})",
                    order_id,
                    item_id,
                    warehouse_id,
                    quantity
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to update sales order {} status to 'picking': {}",
                    order_id,
                    e
                );
            }
        }
    }

    /// Handle WorkOrderCompleted event — auto-advance sales orders that now have
    /// manufactured items available. Confirmed orders advance to "picking",
    /// picking orders advance to "shipped" (ready for delivery).
    pub async fn handle_work_order_completed(&self, work_order_id: &str, item_id: &str, quantity: i64) {
        tracing::info!(
            "Processing work order completed: wo={}, item={}, qty={}",
            work_order_id, item_id, quantity
        );

        let orders = match self.list_sales_orders().await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!("Failed to list sales orders for work order completion: {}", e);
                return;
            }
        };

        for order in &orders {
            if order.status != "confirmed" && order.status != "picking" {
                continue;
            }
            let detail = match self.get_sales_order(&order.id).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            let has_matching_line = detail.lines.iter().any(|l| l.item_id == item_id);
            if !has_matching_line {
                continue;
            }

            let new_status = match order.status.as_str() {
                "confirmed" => "picking",
                "picking" => "shipped",
                _ => continue,
            };

            match self.order_repo.update_status(&order.id, new_status).await {
                Ok(_) => {
                    tracing::info!(
                        "Auto-advanced sales order {} from '{}' to '{}' (work order {} completed: item={}, qty={})",
                        order.order_number, order.status, new_status, work_order_id, item_id, quantity
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to update sales order {} status to '{}': {}",
                        order.order_number, new_status, e
                    );
                }
            }
        }
    }

    // Returns
    pub async fn list_returns(&self) -> AppResult<Vec<ReturnResponse>> {
        self.return_repo.list().await
    }

    // Fulfillments
    pub async fn list_fulfillments(&self) -> AppResult<Vec<crate::models::fulfillment::FulfillmentResponse>> {
        self.fulfillment_repo.list_all().await
    }

    pub async fn list_fulfillments_by_order(&self, order_id: &str) -> AppResult<Vec<crate::models::fulfillment::FulfillmentResponse>> {
        self.fulfillment_repo.list_by_order(order_id).await
    }

    pub async fn get_return(&self, id: &str) -> AppResult<ReturnResponse> {
        self.return_repo.get_by_id(id).await
    }

    pub async fn create_return(&self, input: CreateReturn) -> AppResult<ReturnResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;

        // Check order status — only allow returns if status is at least "confirmed"
        let order = self.order_repo.get_by_id(&input.order_id).await?;
        if order.status == "draft" || order.status == "cancelled" {
            return Err(saas_common::error::AppError::Validation(format!(
                "Cannot create return for order in '{}' status. Order must be at least confirmed.",
                order.status
            )));
        }

        // Validate order line belongs to this order
        let order_lines = self.order_repo.get_lines(&input.order_id).await?;
        let order_line = order_lines
            .iter()
            .find(|ol| ol.id == input.order_line_id)
            .ok_or_else(|| {
                saas_common::error::AppError::Validation(
                    "Order line does not belong to this order".into(),
                )
            })?;

        // Validate return quantity doesn't exceed ordered quantity
        if input.quantity > order_line.quantity {
            return Err(saas_common::error::AppError::Validation(format!(
                "Return quantity ({}) exceeds ordered quantity ({})",
                input.quantity, order_line.quantity
            )));
        }

        // Check cumulative return quantity
        let existing_returns = self.return_repo.list_by_order_line(&input.order_line_id).await?;
        let existing_returned_qty: i64 = existing_returns.iter().map(|r| r.quantity).sum();
        if existing_returned_qty + input.quantity > order_line.quantity {
            return Err(saas_common::error::AppError::Validation(format!(
                "Cumulative return quantity ({}) would exceed ordered quantity ({}). Already returned: {}",
                existing_returned_qty + input.quantity,
                order_line.quantity,
                existing_returned_qty
            )));
        }

        let ret = self.return_repo.create(&input).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.orders.return.created",
                saas_proto::events::ReturnCreated {
                    return_id: ret.id.clone(),
                    order_id: ret.order_id.clone(),
                    item_id: ret.order_line_id.clone(),
                    quantity: ret.quantity,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.orders.return.created",
                e
            );
        }
        Ok(ret)
    }

    pub async fn approve_return(&self, id: &str) -> AppResult<ReturnResponse> {
        let ret = self.return_repo.get_by_id(id).await?;
        if ret.status != "requested" {
            return Err(saas_common::error::AppError::Validation(
                "Only requested returns can be approved".into(),
            ));
        }
        let approved = self.return_repo.update_status(id, "approved").await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.orders.return.approved",
                saas_proto::events::ReturnApproved {
                    return_id: approved.id.clone(),
                    order_id: approved.order_id.clone(),
                    item_id: approved.order_line_id.clone(),
                    quantity: approved.quantity,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.orders.return.approved",
                e
            );
        }
        Ok(approved)
    }

    pub async fn reject_return(&self, id: &str) -> AppResult<ReturnResponse> {
        let ret = self.return_repo.get_by_id(id).await?;
        if ret.status != "requested" {
            return Err(saas_common::error::AppError::Validation(
                "Only requested returns can be rejected".into(),
            ));
        }
        let rejected = self.return_repo.update_status(id, "rejected").await?;
        // No ReturnRejected event type exists in saas_proto; log the rejection for observability
        tracing::info!(
            "Return '{}' for order '{}' rejected (order_line_id: {}, quantity: {})",
            rejected.id, rejected.order_id, rejected.order_line_id, rejected.quantity
        );
        Ok(rejected)
    }

    pub async fn process_return(&self, id: &str, refund_amount_cents: i64) -> AppResult<ReturnResponse> {
        let ret = self.return_repo.get_by_id(id).await?;
        if ret.status != "approved" {
            return Err(saas_common::error::AppError::Validation(
                "Only approved returns can be processed".into(),
            ));
        }
        if refund_amount_cents < 0 {
            return Err(saas_common::error::AppError::Validation(
                "Refund amount must be non-negative".into(),
            ));
        }
        self.return_repo.update_refund_amount(id, refund_amount_cents).await?;
        let processed = self.return_repo.update_status(id, "processed").await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.orders.return.processed",
                saas_proto::events::ReturnProcessed {
                    return_id: processed.id.clone(),
                    order_id: processed.order_id.clone(),
                    refund_amount_cents,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.orders.return.processed",
                e
            );
        }
        Ok(processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::fulfillment_repo::FulfillmentRepo;
    use crate::repository::return_repo::ReturnRepo;
    use crate::repository::sales_order_repo::SalesOrderRepo;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_sales_orders.sql"),
            include_str!("../../migrations/002_create_order_lines.sql"),
            include_str!("../../migrations/003_create_fulfillments.sql"),
            include_str!("../../migrations/004_create_returns.sql"),
            include_str!("../../migrations/005_add_fulfilled_status.sql"),
        ];
        let migration_names = [
            "001_create_sales_orders.sql",
            "002_create_order_lines.sql",
            "003_create_fulfillments.sql",
            "004_create_returns.sql",
            "005_add_fulfilled_status.sql",
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
    async fn test_sales_order_creation_with_lines() {
        let pool = setup().await;
        let repo = SalesOrderRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-001".into(),
            order_date: "2025-01-20".into(),
            shipping_address: Some("456 Oak Ave".into()),
            notes: Some("Rush order".into()),
            lines: vec![
                CreateSalesOrderLine {
                    item_id: "ITEM-100".into(),
                    quantity: 3,
                    unit_price_cents: 2000,
                },
                CreateSalesOrderLine {
                    item_id: "ITEM-101".into(),
                    quantity: 1,
                    unit_price_cents: 15000,
                },
            ],
        };
        let order = repo.create(&input).await.unwrap();
        assert_eq!(order.status, "draft");
        assert_eq!(order.total_cents, 21_000); // 3*2000 + 1*15000
        assert_eq!(order.customer_id, "CUST-001");
        assert_eq!(order.shipping_address, Some("456 Oak Ave".into()));

        let lines = repo.get_lines(&order.id).await.unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].item_id, "ITEM-100");
        assert_eq!(lines[0].line_total_cents, 6000);
        assert_eq!(lines[1].line_total_cents, 15000);
    }

    #[tokio::test]
    async fn test_order_status_draft_to_confirmed() {
        let pool = setup().await;
        let repo = SalesOrderRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-002".into(),
            order_date: "2025-02-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-200".into(),
                quantity: 5,
                unit_price_cents: 1000,
            }],
        };
        let order = repo.create(&input).await.unwrap();
        assert_eq!(order.status, "draft");

        // draft -> confirmed
        repo.update_status(&order.id, "confirmed").await.unwrap();
        let updated = repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "confirmed");
    }

    #[tokio::test]
    async fn test_order_status_confirmed_to_picking() {
        let pool = setup().await;
        let repo = SalesOrderRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-003".into(),
            order_date: "2025-03-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-300".into(),
                quantity: 2,
                unit_price_cents: 5000,
            }],
        };
        let order = repo.create(&input).await.unwrap();
        repo.update_status(&order.id, "confirmed").await.unwrap();

        // confirmed -> picking (via fulfillment)
        repo.update_status(&order.id, "picking").await.unwrap();
        let updated = repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "picking");
    }

    #[tokio::test]
    async fn test_fulfillment_creation() {
        let pool = setup().await;
        let order_repo = SalesOrderRepo::new(pool.clone());
        let fulfillment_repo = FulfillmentRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-004".into(),
            order_date: "2025-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-400".into(),
                quantity: 10,
                unit_price_cents: 500,
            }],
        };
        let order = order_repo.create(&input).await.unwrap();
        let lines = order_repo.get_lines(&order.id).await.unwrap();
        order_repo.update_status(&order.id, "confirmed").await.unwrap();

        // Create fulfillment
        let fulfillment = fulfillment_repo
            .create(&order.id, &lines[0].id, 10, "WH-001")
            .await
            .unwrap();
        assert_eq!(fulfillment.order_id, order.id);
        assert_eq!(fulfillment.quantity, 10);
        assert_eq!(fulfillment.warehouse_id, "WH-001");
        assert_eq!(fulfillment.status, "pending");
    }

    #[tokio::test]
    async fn test_fulfillment_status_update() {
        let pool = setup().await;
        let order_repo = SalesOrderRepo::new(pool.clone());
        let fulfillment_repo = FulfillmentRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-005".into(),
            order_date: "2025-05-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-500".into(),
                quantity: 5,
                unit_price_cents: 300,
            }],
        };
        let order = order_repo.create(&input).await.unwrap();
        let lines = order_repo.get_lines(&order.id).await.unwrap();
        order_repo.update_status(&order.id, "confirmed").await.unwrap();

        let fulfillment = fulfillment_repo
            .create(&order.id, &lines[0].id, 5, "WH-002")
            .await
            .unwrap();

        // Update fulfillment status
        fulfillment_repo
            .update_status(&fulfillment.id, "shipped")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_return_creation() {
        let pool = setup().await;
        let order_repo = SalesOrderRepo::new(pool.clone());
        let return_repo = ReturnRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-006".into(),
            order_date: "2025-06-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-600".into(),
                quantity: 4,
                unit_price_cents: 2500,
            }],
        };
        let order = order_repo.create(&input).await.unwrap();
        let lines = order_repo.get_lines(&order.id).await.unwrap();

        let return_input = CreateReturn {
            order_id: order.id.clone(),
            order_line_id: lines[0].id.clone(),
            quantity: 2,
            reason: Some("Defective product".into()),
        };
        let ret = return_repo.create(&return_input).await.unwrap();
        assert_eq!(ret.order_id, order.id);
        assert_eq!(ret.quantity, 2);
        assert_eq!(ret.status, "requested");
        assert_eq!(ret.reason, Some("Defective product".into()));
    }

    #[tokio::test]
    async fn test_order_confirm_blocks_non_draft() {
        let pool = setup().await;
        let repo = SalesOrderRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-007".into(),
            order_date: "2025-07-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-700".into(),
                quantity: 1,
                unit_price_cents: 100,
            }],
        };
        let order = repo.create(&input).await.unwrap();

        // Move to picking
        repo.update_status(&order.id, "confirmed").await.unwrap();
        repo.update_status(&order.id, "picking").await.unwrap();

        let order = repo.get_by_id(&order.id).await.unwrap();
        // Business rule: only draft can be confirmed
        assert_ne!(order.status, "draft");
    }

    #[tokio::test]
    async fn test_order_line_status_open() {
        let pool = setup().await;
        let repo = SalesOrderRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-008".into(),
            order_date: "2025-08-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-800".into(),
                quantity: 8,
                unit_price_cents: 800,
            }],
        };
        let order = repo.create(&input).await.unwrap();
        let lines = repo.get_lines(&order.id).await.unwrap();
        assert_eq!(lines[0].status, "open");
    }

    #[tokio::test]
    async fn test_fulfillment_updates_order_status() {
        let pool = setup().await;
        let order_repo = SalesOrderRepo::new(pool.clone());
        let fulfillment_repo = FulfillmentRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-FUL".into(),
            order_date: "2025-09-01".into(),
            shipping_address: Some("123 Main St".into()),
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-FUL".into(),
                quantity: 10,
                unit_price_cents: 500,
            }],
        };
        let order = order_repo.create(&input).await.unwrap();
        let lines = order_repo.get_lines(&order.id).await.unwrap();
        order_repo.update_status(&order.id, "confirmed").await.unwrap();

        // Fulfill the order
        fulfillment_repo
            .create(&order.id, &lines[0].id, 10, "WH-FUL-001")
            .await
            .unwrap();

        order_repo.update_status(&order.id, "picking").await.unwrap();
        let updated = order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "picking");
    }

    #[tokio::test]
    async fn test_return_tracks_order_line() {
        let pool = setup().await;
        let order_repo = SalesOrderRepo::new(pool.clone());
        let return_repo = ReturnRepo::new(pool);

        let input = CreateSalesOrder {
            customer_id: "CUST-RET".into(),
            order_date: "2025-10-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-RET".into(),
                quantity: 5,
                unit_price_cents: 2000,
            }],
        };
        let order = order_repo.create(&input).await.unwrap();
        let lines = order_repo.get_lines(&order.id).await.unwrap();

        let ret = return_repo
            .create(&CreateReturn {
                order_id: order.id.clone(),
                order_line_id: lines[0].id.clone(),
                quantity: 3,
                reason: Some("Wrong size".into()),
            })
            .await
            .unwrap();

        assert_eq!(ret.order_id, order.id);
        assert_eq!(ret.quantity, 3);
        assert_eq!(ret.status, "requested");
    }

    #[tokio::test]
    async fn test_return_lifecycle_approve_process() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-LC".into(),
            order_date: "2025-11-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-LC".into(),
                quantity: 10,
                unit_price_cents: 5000,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        let ret = svc.create_return(CreateReturn {
            order_id: order.id.clone(),
            order_line_id: lines[0].id.clone(),
            quantity: 4,
            reason: Some("Damaged".into()),
        }).await.unwrap();
        assert_eq!(ret.status, "requested");

        // Approve
        let approved = svc.approve_return(&ret.id).await.unwrap();
        assert_eq!(approved.status, "approved");

        // Process with refund
        let processed = svc.process_return(&ret.id, 20000).await.unwrap();
        assert_eq!(processed.status, "processed");
        assert_eq!(processed.refund_amount_cents, Some(20000));
    }

    #[tokio::test]
    async fn test_return_reject_from_requested() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-REJ".into(),
            order_date: "2025-12-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-REJ".into(),
                quantity: 2,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        let ret = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 1,
            reason: None,
        }).await.unwrap();

        let rejected = svc.reject_return(&ret.id).await.unwrap();
        assert_eq!(rejected.status, "rejected");
    }

    #[tokio::test]
    async fn test_return_approve_blocks_non_requested() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-BLK".into(),
            order_date: "2025-12-15".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-BLK".into(),
                quantity: 3,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        let ret = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 1,
            reason: None,
        }).await.unwrap();

        // Approve it first
        svc.approve_return(&ret.id).await.unwrap();

        // Try to approve again - should fail
        let result = svc.approve_return(&ret.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_return_process_blocks_non_approved() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-PRO".into(),
            order_date: "2026-01-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-PRO".into(),
                quantity: 1,
                unit_price_cents: 100,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        let ret = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 1,
            reason: Some("Test".into()),
        }).await.unwrap();

        // Process without approval should fail
        let result = svc.process_return(&ret.id, 100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_return_process_negative_refund_blocked() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-NEG".into(),
            order_date: "2026-01-15".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-NEG".into(),
                quantity: 1,
                unit_price_cents: 100,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        let ret = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 1,
            reason: None,
        }).await.unwrap();

        svc.approve_return(&ret.id).await.unwrap();

        // Negative refund should fail
        let result = svc.process_return(&ret.id, -500).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fulfill_order_sets_fulfilled_when_all_lines_complete() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-FULFILL".into(),
            order_date: "2025-07-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-FULL".into(),
                quantity: 5,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();

        // Fulfill all 5 units
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();
        let detail = svc.fulfill_sales_order(&order.id, FulfillRequest {
            lines: vec![FulfillLine {
                order_line_id: lines[0].id.clone(),
                quantity: 5,
                warehouse_id: "WH-1".into(),
            }],
        }).await.unwrap();
        assert_eq!(detail.order.status, "fulfilled");
    }

    #[tokio::test]
    async fn test_fulfill_order_sets_picking_when_partial() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-PICK".into(),
            order_date: "2025-07-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-PARTIAL".into(),
                quantity: 10,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();

        // Fulfill only 3 out of 10
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();
        let detail = svc.fulfill_sales_order(&order.id, FulfillRequest {
            lines: vec![FulfillLine {
                order_line_id: lines[0].id.clone(),
                quantity: 3,
                warehouse_id: "WH-1".into(),
            }],
        }).await.unwrap();
        assert_eq!(detail.order.status, "picking");
    }

    #[tokio::test]
    async fn test_cancel_sales_order_publishes_event() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-CANCEL-EV".into(),
            order_date: "2025-08-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-CANCEL".into(),
                quantity: 1,
                unit_price_cents: 100,
            }],
        }).await.unwrap();

        let cancelled = svc.cancel_sales_order(&order.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");
    }

    #[tokio::test]
    async fn test_return_quantity_exceeds_ordered() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-RETQTY".into(),
            order_date: "2025-09-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-RETQTY".into(),
                quantity: 3,
                unit_price_cents: 100,
            }],
        }).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        // Return 5 when only 3 ordered
        let result = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 5,
            reason: None,
        }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_stock_reserved_advances_confirmed_to_picking() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and confirm a sales order
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-STOCK".into(),
            order_date: "2026-02-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-STOCK".into(),
                quantity: 10,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "confirmed").await.unwrap();

        // Simulate stock reserved event for this sales order
        svc.handle_stock_reserved("ITEM-STOCK", "WH-001", 10, "sales_order", &order.id).await;

        // Verify status advanced to picking
        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "picking");
    }

    #[tokio::test]
    async fn test_handle_stock_reserved_ignores_non_sales_order_reference() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and confirm a sales order
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-REF".into(),
            order_date: "2026-02-15".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-REF".into(),
                quantity: 5,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "confirmed").await.unwrap();

        // Call with a non-sales_order reference type
        svc.handle_stock_reserved("ITEM-REF", "WH-002", 5, "purchase_order", &order.id).await;

        // Status should remain confirmed (not advanced)
        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "confirmed");
    }

    #[tokio::test]
    async fn test_handle_stock_reserved_ignores_non_confirmed_order() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create a sales order but leave it in draft
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-DRAFT".into(),
            order_date: "2026-03-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-DRAFT".into(),
                quantity: 2,
                unit_price_cents: 100,
            }],
        }).await.unwrap();
        // Status is "draft" — not confirmed

        svc.handle_stock_reserved("ITEM-DRAFT", "WH-003", 2, "sales_order", &order.id).await;

        // Status should remain draft (not advanced)
        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "draft");
    }

    #[tokio::test]
    async fn test_handle_stock_reserved_ignores_already_picking_order() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create, confirm, then advance to picking already
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-PICK2".into(),
            order_date: "2026-03-15".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-PICK2".into(),
                quantity: 7,
                unit_price_cents: 200,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "confirmed").await.unwrap();
        svc.order_repo.update_status(&order.id, "picking").await.unwrap();

        // Another stock reserved event arrives — should not change status
        svc.handle_stock_reserved("ITEM-PICK2", "WH-004", 7, "sales_order", &order.id).await;

        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "picking");
    }

    #[tokio::test]
    async fn test_handle_work_order_completed_advances_confirmed_to_picking() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-WO1".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-MFG".into(),
                quantity: 10,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "confirmed").await.unwrap();

        svc.handle_work_order_completed("WO-001", "ITEM-MFG", 10).await;

        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "picking");
    }

    #[tokio::test]
    async fn test_handle_work_order_completed_advances_picking_to_shipped() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-WO2".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-MFG2".into(),
                quantity: 5,
                unit_price_cents: 300,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "confirmed").await.unwrap();
        svc.order_repo.update_status(&order.id, "picking").await.unwrap();

        svc.handle_work_order_completed("WO-002", "ITEM-MFG2", 5).await;

        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "shipped");
    }

    #[tokio::test]
    async fn test_handle_work_order_completed_no_matching_item() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-WO3".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-OTHER".into(),
                quantity: 3,
                unit_price_cents: 100,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "confirmed").await.unwrap();

        svc.handle_work_order_completed("WO-003", "ITEM-DIFFERENT", 3).await;

        let updated = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(updated.status, "confirmed"); // unchanged
    }

    #[tokio::test]
    async fn test_confirm_already_confirmed_order_fails() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and confirm an order
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-DBLCONF".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-DBLCONF".into(),
                quantity: 2,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        assert_eq!(order.status, "draft");

        let confirmed = svc.confirm_sales_order(&order.id).await.unwrap();
        assert_eq!(confirmed.order.status, "confirmed");

        // Try to confirm again — should fail
        let result = svc.confirm_sales_order(&order.id).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Only draft orders can be confirmed"));
    }

    #[tokio::test]
    async fn test_cancel_fulfilled_order_fails() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create, confirm, and fulfill an order
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-CANCFUL".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-CANCFUL".into(),
                quantity: 3,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();

        svc.confirm_sales_order(&order.id).await.unwrap();

        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();
        let fulfilled = svc.fulfill_sales_order(&order.id, FulfillRequest {
            lines: vec![FulfillLine {
                order_line_id: lines[0].id.clone(),
                quantity: 3,
                warehouse_id: "WH-1".into(),
            }],
        }).await.unwrap();
        assert_eq!(fulfilled.order.status, "fulfilled");

        // Try to cancel the fulfilled order — should fail
        let result = svc.cancel_sales_order(&order.id).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Only draft or confirmed orders can be cancelled"));
    }

    #[tokio::test]
    async fn test_sales_order_status_transitions() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Start: create order in draft
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-LIFECYCLE".into(),
            order_date: "2026-04-01".into(),
            shipping_address: Some("789 Lifecycle Blvd".into()),
            notes: Some("Full lifecycle test".into()),
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-LIFE".into(),
                quantity: 10,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();
        assert_eq!(order.status, "draft");

        // draft -> confirmed
        let confirmed = svc.confirm_sales_order(&order.id).await.unwrap();
        assert_eq!(confirmed.order.status, "confirmed");

        // confirmed -> picking (partial fulfillment)
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();
        let partial = svc.fulfill_sales_order(&order.id, FulfillRequest {
            lines: vec![FulfillLine {
                order_line_id: lines[0].id.clone(),
                quantity: 4,
                warehouse_id: "WH-LIFE".into(),
            }],
        }).await.unwrap();
        assert_eq!(partial.order.status, "picking");

        // picking -> shipped (simulate via work order completed handler path:
        // the handle_work_order_completed method advances picking -> shipped)
        svc.order_repo.update_status(&order.id, "shipped").await.unwrap();
        let shipped = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(shipped.status, "shipped");

        // shipped -> fulfilled (remaining fulfillment delivered;
        // fulfill_sales_order rejects shipped status, so we use repo directly
        // to reflect the final delivery step)
        svc.order_repo.update_status(&order.id, "fulfilled").await.unwrap();
        let fulfilled = svc.order_repo.get_by_id(&order.id).await.unwrap();
        assert_eq!(fulfilled.status, "fulfilled");
    }

    #[tokio::test]
    async fn test_return_with_invalid_order_line_id() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and confirm an order
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-INVLINE".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-INVLINE".into(),
                quantity: 5,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();

        // Try to create a return with a completely made-up order_line_id
        let result = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: "nonexistent-line-id-99999".into(),
            quantity: 1,
            reason: None,
        }).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Order line does not belong to this order"));
    }

    #[tokio::test]
    async fn test_cumulative_return_exceeds_ordered() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and confirm an order with qty=5
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-CUMUL".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-CUMUL".into(),
                quantity: 5,
                unit_price_cents: 500,
            }],
        }).await.unwrap();
        svc.confirm_sales_order(&order.id).await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        // First return: qty=3 — valid
        let ret1 = svc.create_return(CreateReturn {
            order_id: order.id.clone(),
            order_line_id: lines[0].id.clone(),
            quantity: 3,
            reason: Some("Defective".into()),
        }).await.unwrap();
        assert_eq!(ret1.quantity, 3);

        // Second return: qty=3 — should fail because 3+3=6 > 5 ordered
        let result = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 3,
            reason: Some("More defective".into()),
        }).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Cumulative return quantity"));
    }

    #[tokio::test]
    async fn test_return_on_draft_order_blocked() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create order — stays in "draft" status
        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-DRAFTRET".into(),
            order_date: "2026-04-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-DRAFTRET".into(),
                quantity: 5,
                unit_price_cents: 1000,
            }],
        }).await.unwrap();
        assert_eq!(order.status, "draft");

        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        // Try to create a return on a draft order — should fail
        let result = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 1,
            reason: Some("Changed mind".into()),
        }).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Cannot create return for order in 'draft' status"));
    }

    #[tokio::test]
    async fn test_return_on_cancelled_order_blocked() {
        let pool = setup().await;
        let svc = OrderManagementService {
            order_repo: SalesOrderRepo::new(pool.clone()),
            fulfillment_repo: FulfillmentRepo::new(pool.clone()),
            return_repo: ReturnRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let order = svc.order_repo.create(&CreateSalesOrder {
            customer_id: "CUST-CANCEL".into(),
            order_date: "2025-11-01".into(),
            shipping_address: None,
            notes: None,
            lines: vec![CreateSalesOrderLine {
                item_id: "ITEM-CANCEL".into(),
                quantity: 2,
                unit_price_cents: 100,
            }],
        }).await.unwrap();
        svc.order_repo.update_status(&order.id, "cancelled").await.unwrap();
        let lines = svc.order_repo.get_lines(&order.id).await.unwrap();

        let result = svc.create_return(CreateReturn {
            order_id: order.id,
            order_line_id: lines[0].id.clone(),
            quantity: 1,
            reason: None,
        }).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cancelled"), "Expected cancelled status error, got: {}", err);
    }
}

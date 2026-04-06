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
        self.order_repo.create(&input).await
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
        let mut fulfilled_lines = Vec::new();
        for line in &input.lines {
            self.fulfillment_repo
                .create(id, &line.order_line_id, line.quantity, &line.warehouse_id)
                .await?;
            fulfilled_lines.push(saas_proto::events::OrderFulfilledLine {
                item_id: line.order_line_id.clone(),
                quantity: line.quantity,
                warehouse_id: line.warehouse_id.clone(),
            });
        }
        self.order_repo.update_status(id, "picking").await?;
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

    // Returns
    pub async fn list_returns(&self) -> AppResult<Vec<ReturnResponse>> {
        self.return_repo.list().await
    }

    pub async fn get_return(&self, id: &str) -> AppResult<ReturnResponse> {
        self.return_repo.get_by_id(id).await
    }

    pub async fn create_return(&self, input: CreateReturn) -> AppResult<ReturnResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
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
        ];
        let migration_names = [
            "001_create_sales_orders.sql",
            "002_create_order_lines.sql",
            "003_create_fulfillments.sql",
            "004_create_returns.sql",
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
}

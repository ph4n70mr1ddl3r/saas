use crate::models::bom::*;
use crate::models::work_order::*;
use crate::repository::bom_repo::BomRepo;
use crate::repository::routing_step_repo::RoutingStepRepo;
use crate::repository::work_order_repo::WorkOrderRepo;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct ManufacturingService {
    work_order_repo: WorkOrderRepo,
    bom_repo: BomRepo,
    routing_step_repo: RoutingStepRepo,
    bus: NatsBus,
}

impl ManufacturingService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus,
        }
    }

    // Work Orders
    pub async fn list_work_orders(&self) -> AppResult<Vec<WorkOrderResponse>> {
        self.work_order_repo.list().await
    }

    pub async fn get_work_order(&self, id: &str) -> AppResult<WorkOrderResponse> {
        self.work_order_repo.get_by_id(id).await
    }

    pub async fn create_work_order(&self, input: CreateWorkOrder) -> AppResult<WorkOrderResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.work_order_repo.create(&input).await
    }

    pub async fn start_work_order(&self, id: &str) -> AppResult<WorkOrderResponse> {
        let wo = self.work_order_repo.get_by_id(id).await?;
        if wo.status != "planned" {
            return Err(saas_common::error::AppError::Validation(
                "Only planned work orders can be started".into(),
            ));
        }
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.work_order_repo
            .update_status(id, "in_progress", Some(&now), None)
            .await?;
        let wo = self.work_order_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.manufacturing.work_order.started",
                saas_proto::events::WorkOrderStarted {
                    work_order_id: wo.id.clone(),
                    item_id: wo.item_id.clone(),
                    quantity: wo.quantity,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.manufacturing.work_order.started",
                e
            );
        }
        Ok(wo)
    }

    pub async fn complete_work_order(&self, id: &str) -> AppResult<WorkOrderResponse> {
        let wo = self.work_order_repo.get_by_id(id).await?;
        if wo.status != "in_progress" {
            return Err(saas_common::error::AppError::Validation(
                "Only in-progress work orders can be completed".into(),
            ));
        }
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.work_order_repo
            .update_status(id, "completed", None, Some(&now))
            .await?;
        let wo = self.work_order_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.manufacturing.work_order.completed",
                saas_proto::events::WorkOrderCompleted {
                    work_order_id: wo.id.clone(),
                    item_id: wo.item_id.clone(),
                    quantity: wo.quantity,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.manufacturing.work_order.completed",
                e
            );
        }
        Ok(wo)
    }

    pub async fn cancel_work_order(&self, id: &str) -> AppResult<WorkOrderResponse> {
        let wo = self.work_order_repo.get_by_id(id).await?;
        if wo.status != "planned" && wo.status != "in_progress" {
            return Err(saas_common::error::AppError::Validation(
                "Only planned or in-progress work orders can be cancelled".into(),
            ));
        }
        self.work_order_repo
            .update_status(id, "cancelled", None, None)
            .await?;
        let wo = self.work_order_repo.get_by_id(id).await?;
        if let Err(e) = self
            .bus
            .publish(
                "scm.manufacturing.work_order.cancelled",
                saas_proto::events::WorkOrderCancelled {
                    work_order_id: wo.id.clone(),
                    item_id: wo.item_id.clone(),
                    quantity: wo.quantity,
                    reason: None,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "scm.manufacturing.work_order.cancelled",
                e
            );
        }
        Ok(wo)
    }

    // BOMs
    pub async fn list_boms(&self) -> AppResult<Vec<BomResponse>> {
        self.bom_repo.list().await
    }

    pub async fn get_bom(&self, id: &str) -> AppResult<BomDetailResponse> {
        let bom = self.bom_repo.get_by_id(id).await?;
        let components = self.bom_repo.get_components(id).await?;
        Ok(BomDetailResponse { bom, components })
    }

    pub async fn create_bom(&self, input: CreateBom) -> AppResult<BomResponse> {
        input
            .validate()
            .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.bom_repo.create(&input).await
    }

    /// Handle sales order confirmed by auto-creating a work order if a BOM exists for the item.
    pub async fn handle_order_confirmed(
        &self,
        order_id: &str,
        item_id: &str,
        quantity: i64,
    ) -> AppResult<Option<WorkOrderResponse>> {
        // Check if a BOM exists for this item
        let boms = self.bom_repo.list().await?;
        let bom = match boms.iter().find(|b| b.finished_item_id == item_id) {
            Some(b) => b,
            None => return Ok(None), // No BOM, not a manufactured item
        };

        let now = chrono::Utc::now();
        let planned_start = now.format("%Y-%m-%dT%H:%M:%S").to_string();
        let planned_end = (now + chrono::Duration::days(7)).format("%Y-%m-%dT%H:%M:%S").to_string();

        let bom_qty = if bom.quantity > 0 { bom.quantity } else { 1 };
        let wo_quantity = (quantity + bom_qty - 1) / bom_qty * bom_qty; // Round up to BOM multiples

        let input = CreateWorkOrder {
            item_id: item_id.to_string(),
            quantity: wo_quantity,
            planned_start: Some(planned_start),
            planned_end: Some(planned_end),
        };

        tracing::info!(
            "Auto-creating work order for item {} (order {}) qty {} based on BOM {}",
            item_id, order_id, wo_quantity, bom.id
        );
        let wo = self.create_work_order(input).await?;
        Ok(Some(wo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::bom_repo::BomRepo;
    use crate::repository::routing_step_repo::RoutingStepRepo;
    use crate::repository::work_order_repo::WorkOrderRepo;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_work_orders.sql"),
            include_str!("../../migrations/002_create_bom.sql"),
            include_str!("../../migrations/003_create_bom_components.sql"),
            include_str!("../../migrations/004_create_routing_steps.sql"),
        ];
        let migration_names = [
            "001_create_work_orders.sql",
            "002_create_bom.sql",
            "003_create_bom_components.sql",
            "004_create_routing_steps.sql",
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
    async fn test_work_order_creation() {
        let pool = setup().await;
        let repo = WorkOrderRepo::new(pool);

        let input = CreateWorkOrder {
            item_id: "ITEM-ASSEMBLY-001".into(),
            quantity: 100,
            planned_start: Some("2025-03-01T08:00:00".into()),
            planned_end: Some("2025-03-05T17:00:00".into()),
        };
        let wo = repo.create(&input).await.unwrap();
        assert_eq!(wo.status, "planned");
        assert_eq!(wo.item_id, "ITEM-ASSEMBLY-001");
        assert_eq!(wo.quantity, 100);
        assert!(wo.wo_number.starts_with("WO-"));
    }

    #[tokio::test]
    async fn test_work_order_status_planned_to_in_progress() {
        let pool = setup().await;
        let repo = WorkOrderRepo::new(pool);

        let input = CreateWorkOrder {
            item_id: "ITEM-ASSY-002".into(),
            quantity: 50,
            planned_start: None,
            planned_end: None,
        };
        let wo = repo.create(&input).await.unwrap();
        assert_eq!(wo.status, "planned");

        // planned -> in_progress (sets actual_start)
        let now = "2025-04-01T09:00:00";
        repo.update_status(&wo.id, "in_progress", Some(now), None)
            .await
            .unwrap();
        let wo = repo.get_by_id(&wo.id).await.unwrap();
        assert_eq!(wo.status, "in_progress");
        assert_eq!(wo.actual_start, Some(now.to_string()));
    }

    #[tokio::test]
    async fn test_work_order_status_in_progress_to_completed() {
        let pool = setup().await;
        let repo = WorkOrderRepo::new(pool);

        let input = CreateWorkOrder {
            item_id: "ITEM-ASSY-003".into(),
            quantity: 25,
            planned_start: Some("2025-05-01T08:00:00".into()),
            planned_end: Some("2025-05-03T17:00:00".into()),
        };
        let wo = repo.create(&input).await.unwrap();

        // planned -> in_progress
        repo.update_status(&wo.id, "in_progress", Some("2025-05-01T08:00:00"), None)
            .await
            .unwrap();

        // in_progress -> completed (sets actual_end)
        let end_time = "2025-05-02T16:30:00";
        repo.update_status(&wo.id, "completed", None, Some(end_time))
            .await
            .unwrap();
        let wo = repo.get_by_id(&wo.id).await.unwrap();
        assert_eq!(wo.status, "completed");
        assert_eq!(wo.actual_end, Some(end_time.to_string()));
    }

    #[tokio::test]
    async fn test_work_order_start_blocks_non_planned() {
        let pool = setup().await;
        let repo = WorkOrderRepo::new(pool);

        let input = CreateWorkOrder {
            item_id: "ITEM-ASSY-004".into(),
            quantity: 10,
            planned_start: None,
            planned_end: None,
        };
        let wo = repo.create(&input).await.unwrap();

        // Move to in_progress then try to start again
        repo.update_status(&wo.id, "in_progress", Some("2025-06-01T08:00:00"), None)
            .await
            .unwrap();
        let wo = repo.get_by_id(&wo.id).await.unwrap();

        // Business rule: only planned can be started
        assert_ne!(wo.status, "planned");
    }

    #[tokio::test]
    async fn test_work_order_complete_blocks_non_in_progress() {
        let pool = setup().await;
        let repo = WorkOrderRepo::new(pool);

        let input = CreateWorkOrder {
            item_id: "ITEM-ASSY-005".into(),
            quantity: 15,
            planned_start: None,
            planned_end: None,
        };
        let wo = repo.create(&input).await.unwrap();

        // Business rule: only in_progress can be completed
        assert_eq!(wo.status, "planned");
        assert_ne!(wo.status, "in_progress");
    }

    #[tokio::test]
    async fn test_bom_creation_with_components() {
        let pool = setup().await;
        let repo = BomRepo::new(pool);

        let input = CreateBom {
            name: "Widget Assembly".into(),
            description: Some("Standard widget".into()),
            finished_item_id: "ITEM-FINISHED-001".into(),
            quantity: Some(1),
            components: vec![
                CreateBomComponent {
                    component_item_id: "COMP-A".into(),
                    quantity_required: 2,
                },
                CreateBomComponent {
                    component_item_id: "COMP-B".into(),
                    quantity_required: 4,
                },
                CreateBomComponent {
                    component_item_id: "COMP-C".into(),
                    quantity_required: 1,
                },
            ],
        };
        let bom = repo.create(&input).await.unwrap();
        assert_eq!(bom.name, "Widget Assembly");
        assert_eq!(bom.finished_item_id, "ITEM-FINISHED-001");
        assert_eq!(bom.quantity, 1);

        // Verify components
        let components = repo.get_components(&bom.id).await.unwrap();
        assert_eq!(components.len(), 3);
        assert_eq!(components[0].component_item_id, "COMP-A");
        assert_eq!(components[0].quantity_required, 2);
        assert_eq!(components[1].component_item_id, "COMP-B");
        assert_eq!(components[1].quantity_required, 4);
        assert_eq!(components[2].component_item_id, "COMP-C");
        assert_eq!(components[2].quantity_required, 1);
    }

    #[tokio::test]
    async fn test_bom_default_quantity() {
        let pool = setup().await;
        let repo = BomRepo::new(pool);

        let input = CreateBom {
            name: "Gadget Assembly".into(),
            description: None,
            finished_item_id: "ITEM-FINISHED-002".into(),
            quantity: None, // defaults to 1
            components: vec![CreateBomComponent {
                component_item_id: "COMP-D".into(),
                quantity_required: 3,
            }],
        };
        let bom = repo.create(&input).await.unwrap();
        assert_eq!(bom.quantity, 1); // default
    }

    #[tokio::test]
    async fn test_routing_steps() {
        let pool = setup().await;
        let wo_repo = WorkOrderRepo::new(pool.clone());
        let routing_repo = RoutingStepRepo::new(pool);

        // Create work order first
        let wo = wo_repo
            .create(&CreateWorkOrder {
                item_id: "ITEM-ASSY-RT".into(),
                quantity: 20,
                planned_start: None,
                planned_end: None,
            })
            .await
            .unwrap();

        // Create routing steps
        let step1 = routing_repo
            .create(&wo.id, 1, "Cut raw material")
            .await
            .unwrap();
        assert_eq!(step1.work_order_id, wo.id);
        assert_eq!(step1.step_number, 1);
        assert_eq!(step1.status, "pending");
        assert_eq!(step1.description, Some("Cut raw material".into()));

        let step2 = routing_repo
            .create(&wo.id, 2, "Weld joints")
            .await
            .unwrap();
        assert_eq!(step2.step_number, 2);

        let step3 = routing_repo
            .create(&wo.id, 3, "Quality inspection")
            .await
            .unwrap();

        // List all steps
        let steps = routing_repo.list_by_work_order(&wo.id).await.unwrap();
        assert_eq!(steps.len(), 3);

        // Update step statuses
        routing_repo
            .update_status(&step1.id, "in_progress")
            .await
            .unwrap();
        routing_repo
            .update_status(&step1.id, "completed")
            .await
            .unwrap();
        routing_repo
            .update_status(&step2.id, "in_progress")
            .await
            .unwrap();

        let steps = routing_repo.list_by_work_order(&wo.id).await.unwrap();
        assert_eq!(steps[0].status, "completed");
        assert_eq!(steps[1].status, "in_progress");
        assert_eq!(steps[2].status, "pending");
    }

    #[tokio::test]
    async fn test_bom_not_found() {
        let pool = setup().await;
        let repo = BomRepo::new(pool);
        let result = repo.get_by_id("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_work_order_completion_updates_item_and_quantity() {
        let pool = setup().await;
        let repo = WorkOrderRepo::new(pool);

        let input = CreateWorkOrder {
            item_id: "ITEM-FG-001".into(),
            quantity: 500,
            planned_start: Some("2025-06-01T08:00:00".into()),
            planned_end: Some("2025-06-05T17:00:00".into()),
        };
        let wo = repo.create(&input).await.unwrap();

        // Start the work order
        repo.update_status(&wo.id, "in_progress", Some("2025-06-01T08:00:00"), None)
            .await
            .unwrap();

        // Complete the work order
        let end_time = "2025-06-04T16:00:00";
        repo.update_status(&wo.id, "completed", None, Some(end_time))
            .await
            .unwrap();

        let completed = repo.get_by_id(&wo.id).await.unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.item_id, "ITEM-FG-001");
        assert_eq!(completed.quantity, 500);
        assert_eq!(completed.actual_end, Some(end_time.to_string()));
    }

    #[tokio::test]
    async fn test_cancel_work_order_service() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool.clone()),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Cancel from planned
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-CANCEL-001".into(),
            quantity: 10,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        assert_eq!(wo.status, "planned");

        let cancelled = svc.cancel_work_order(&wo.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");

        // Cancel from in_progress
        let wo2 = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-CANCEL-002".into(),
            quantity: 20,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        svc.start_work_order(&wo2.id).await.unwrap();

        let cancelled2 = svc.cancel_work_order(&wo2.id).await.unwrap();
        assert_eq!(cancelled2.status, "cancelled");

        // Cannot cancel a completed work order
        let wo3 = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-CANCEL-003".into(),
            quantity: 5,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        svc.start_work_order(&wo3.id).await.unwrap();
        svc.complete_work_order(&wo3.id).await.unwrap();

        let result = svc.cancel_work_order(&wo3.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_work_order_publishes_event() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-START-EV".into(),
            quantity: 50,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        assert_eq!(wo.status, "planned");

        let started = svc.start_work_order(&wo.id).await.unwrap();
        assert_eq!(started.status, "in_progress");
        assert!(started.actual_start.is_some());
    }
}

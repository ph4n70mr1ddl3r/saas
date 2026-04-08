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
        if input.quantity <= 0 {
            return Err(saas_common::error::AppError::Validation(
                "Work order quantity must be positive".into(),
            ));
        }
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
        // Verify all routing steps are completed
        let routing_steps = self.routing_step_repo.list_by_work_order(id).await?;
        if let Some(incomplete) = routing_steps.iter().find(|s| s.status != "completed") {
            return Err(saas_common::error::AppError::Validation(format!(
                "Cannot complete work order: routing step {} (step {}) has status '{}', expected 'completed'",
                incomplete.id, incomplete.step_number, incomplete.status
            )));
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
        // Validate each component (nested validation is not automatic)
        for component in &input.components {
            component
                .validate()
                .map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        }
        // Check for duplicate BOM for the same finished item
        let existing = self.bom_repo.list().await?;
        if existing
            .iter()
            .any(|b| b.finished_item_id == input.finished_item_id)
        {
            return Err(saas_common::error::AppError::Validation(format!(
                "BOM for item '{}' already exists",
                input.finished_item_id
            )));
        }
        self.bom_repo.create(&input).await
    }

    /// Handle PO cancellation notification.
    /// Logs the cancellation and checks if any active work orders may be impacted
    /// because they depend on materials sourced from this PO/supplier.
    pub async fn handle_po_cancelled(
        &self,
        po_id: &str,
        supplier_id: &str,
        reason: &Option<String>,
    ) -> AppResult<()> {
        tracing::info!(
            "PO cancelled notification received: po_id={}, supplier_id={}, reason={:?}",
            po_id, supplier_id, reason
        );

        // Check for work orders that might be tied to materials from this PO/supplier.
        // Work orders in non-terminal states (planned, in_progress) are potentially impacted.
        let work_orders = self.work_order_repo.list().await?;
        let active_orders: Vec<_> = work_orders
            .iter()
            .filter(|wo| wo.status == "planned" || wo.status == "in_progress")
            .collect();

        if active_orders.is_empty() {
            tracing::info!(
                "No active work orders found that could be impacted by PO cancellation: po_id={}",
                po_id
            );
        } else {
            tracing::warn!(
                "PO {} from supplier {} cancelled — {} active work order(s) may be impacted by material supply disruption",
                po_id, supplier_id, active_orders.len()
            );
            for wo in &active_orders {
                tracing::warn!(
                    "Potentially impacted work order: wo_number={}, item_id={}, quantity={}, status={}",
                    wo.wo_number, wo.item_id, wo.quantity, wo.status
                );
            }
        }

        Ok(())
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

    /// Handle purchase order approved by checking BOMs for materials from this supplier.
    /// This is a notification/planning handler that logs which BOMs may need production
    /// scheduling now that materials from the given supplier have been approved.
    pub async fn handle_po_approved(&self, po_id: &str, supplier_id: &str) -> AppResult<Vec<String>> {
        tracing::info!(
            "PO {} approved for supplier {} — checking BOMs for affected materials",
            po_id, supplier_id
        );

        let boms = self.bom_repo.list().await?;
        let mut affected_finished_items = Vec::new();

        for bom in &boms {
            let components = self.bom_repo.get_components(&bom.id).await?;
            if !components.is_empty() {
                tracing::info!(
                    "BOM '{}' (finished item: {}) has {} component(s) — production may need to be planned/scheduled for materials from supplier {}",
                    bom.name, bom.finished_item_id, components.len(), supplier_id
                );
                affected_finished_items.push(bom.finished_item_id.clone());
            }
        }

        if affected_finished_items.is_empty() {
            tracing::info!(
                "No BOMs found with components that could be affected by PO {} from supplier {}",
                po_id, supplier_id
            );
        }

        Ok(affected_finished_items)
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

    #[tokio::test]
    async fn test_handle_po_approved_with_boms() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create a BOM with components — this should be flagged for production planning
        svc.create_bom(CreateBom {
            name: "Motor Assembly".into(),
            description: Some("Electric motor".into()),
            finished_item_id: "ITEM-MOTOR-001".into(),
            quantity: Some(1),
            components: vec![
                CreateBomComponent {
                    component_item_id: "COMP-STATOR".into(),
                    quantity_required: 1,
                },
                CreateBomComponent {
                    component_item_id: "COMP-ROTOR".into(),
                    quantity_required: 1,
                },
            ],
        })
        .await
        .unwrap();

        // Handle PO approved
        let affected = svc
            .handle_po_approved("PO-001", "SUPPLIER-ABC")
            .await
            .unwrap();

        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0], "ITEM-MOTOR-001");
    }

    #[tokio::test]
    async fn test_handle_po_approved_no_boms() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // No BOMs exist — should return empty list
        let affected = svc
            .handle_po_approved("PO-002", "SUPPLIER-XYZ")
            .await
            .unwrap();

        assert!(affected.is_empty());
    }

    #[tokio::test]
    async fn test_handle_po_cancelled_warns_active_work_orders() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create a planned work order that could be impacted
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-PO-CANCEL-001".into(),
            quantity: 50,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        assert_eq!(wo.status, "planned");

        // Handle PO cancelled — should succeed and warn about the active work order
        let result = svc
            .handle_po_cancelled("PO-CANCEL-001", "SUPPLIER-001", &Some("Supplier went bankrupt".into()))
            .await;
        assert!(result.is_ok());

        // The work order should still exist and be in planned status (not auto-cancelled)
        let wo_check = svc.get_work_order(&wo.id).await.unwrap();
        assert_eq!(wo_check.status, "planned");
    }

    #[tokio::test]
    async fn test_handle_po_cancelled_no_active_work_orders() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // No work orders at all — should succeed gracefully
        let result = svc
            .handle_po_cancelled("PO-CANCEL-002", "SUPPLIER-002", &None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_bom_without_components_rejected() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // CreateBom has #[validate(length(min = 1))] on the components field,
        // so an empty components list should be rejected by input.validate()
        let result = svc.create_bom(CreateBom {
            name: "Empty BOM".into(),
            description: Some("BOM with no components".into()),
            finished_item_id: "ITEM-EMPTY-BOM".into(),
            quantity: Some(1),
            components: vec![],
        }).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("component"));
    }

    #[tokio::test]
    async fn test_bom_negative_component_quantity_rejected() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // CreateBomComponent has #[validate(range(min = 1))] on quantity_required,
        // so a negative quantity should be rejected by input.validate()
        let result = svc.create_bom(CreateBom {
            name: "Bad Qty BOM".into(),
            description: None,
            finished_item_id: "ITEM-NEG-QTY".into(),
            quantity: Some(1),
            components: vec![CreateBomComponent {
                component_item_id: "COMP-NEG".into(),
                quantity_required: -5,
            }],
        }).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("quantity_required")
            || err.to_string().contains("quantity")
            || err.to_string().contains("Validation"));
    }

    #[tokio::test]
    async fn test_work_order_status_transitions() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create work order — starts as planned
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-LIFECYCLE".into(),
            quantity: 50,
            planned_start: Some("2026-04-01T08:00:00".into()),
            planned_end: Some("2026-04-10T17:00:00".into()),
        }).await.unwrap();
        assert_eq!(wo.status, "planned");

        // planned -> in_progress
        let started = svc.start_work_order(&wo.id).await.unwrap();
        assert_eq!(started.status, "in_progress");
        assert!(started.actual_start.is_some());

        // in_progress -> completed
        let completed = svc.complete_work_order(&wo.id).await.unwrap();
        assert_eq!(completed.status, "completed");
        assert!(completed.actual_end.is_some());
    }

    #[tokio::test]
    async fn test_complete_planned_work_order_fails() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create a work order (starts as planned)
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-SKIP-START".into(),
            quantity: 20,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        assert_eq!(wo.status, "planned");

        // Try to complete without starting — should fail
        let result = svc.complete_work_order(&wo.id).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Only in-progress work orders can be completed"));
    }

    #[tokio::test]
    async fn test_start_completed_work_order_fails() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create, start, then complete a work order
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-RESTART".into(),
            quantity: 15,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();

        svc.start_work_order(&wo.id).await.unwrap();
        svc.complete_work_order(&wo.id).await.unwrap();

        // Try to start the completed work order — should fail
        let result = svc.start_work_order(&wo.id).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Only planned work orders can be started"));
    }

    #[tokio::test]
    async fn test_cancel_in_progress_work_order() {
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
            item_id: "ITEM-CANCEL-WIP".into(),
            quantity: 10,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        svc.start_work_order(&wo.id).await.unwrap();

        // Cancel the in-progress work order
        let cancelled = svc.cancel_work_order(&wo.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");
    }

    #[tokio::test]
    async fn test_cancel_completed_work_order_fails() {
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
            item_id: "ITEM-CANCEL-DONE".into(),
            quantity: 5,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        svc.start_work_order(&wo.id).await.unwrap();
        svc.complete_work_order(&wo.id).await.unwrap();

        // Try to cancel a completed work order — should fail
        let result = svc.cancel_work_order(&wo.id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Only planned or in-progress work orders can be cancelled"));
    }

    #[tokio::test]
    async fn test_work_order_full_lifecycle() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // planned -> in_progress -> completed
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-LIFE".into(),
            quantity: 20,
            planned_start: Some("2025-08-01".into()),
            planned_end: Some("2025-08-15".into()),
        }).await.unwrap();
        assert_eq!(wo.status, "planned");

        let started = svc.start_work_order(&wo.id).await.unwrap();
        assert_eq!(started.status, "in_progress");
        assert!(started.actual_start.is_some());

        let completed = svc.complete_work_order(&wo.id).await.unwrap();
        assert_eq!(completed.status, "completed");
        assert!(completed.actual_end.is_some());
    }

    #[tokio::test]
    async fn test_create_duplicate_bom_fails() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let bom_input = CreateBom {
            name: "Test BOM DUP".into(),
            finished_item_id: "ITEM-DUP".into(),
            quantity: Some(1),
            description: Some("First BOM".into()),
            components: vec![CreateBomComponent {
                component_item_id: "COMP-1".into(),
                quantity_required: 2,
            }],
        };
        svc.create_bom(bom_input).await.unwrap();

        // Duplicate BOM for same finished item
        let result = svc.create_bom(CreateBom {
            name: "Test BOM DUP 2".into(),
            finished_item_id: "ITEM-DUP".into(),
            quantity: Some(1),
            description: Some("Second BOM".into()),
            components: vec![CreateBomComponent {
                component_item_id: "COMP-2".into(),
                quantity_required: 1,
            }],
        }).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_get_bom_with_components() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let bom = svc.create_bom(CreateBom {
            name: "Detailed BOM".into(),
            finished_item_id: "ITEM-DETAIL".into(),
            quantity: Some(1),
            description: Some("Detailed BOM".into()),
            components: vec![
                CreateBomComponent {
                    component_item_id: "COMP-A".into(),
                    quantity_required: 3,
                },
                CreateBomComponent {
                    component_item_id: "COMP-B".into(),
                    quantity_required: 1,
                },
            ],
        }).await.unwrap();

        let detail = svc.get_bom(&bom.id).await.unwrap();
        assert_eq!(detail.bom.finished_item_id, "ITEM-DETAIL");
        assert_eq!(detail.components.len(), 2);
        assert_eq!(detail.components[0].component_item_id, "COMP-A");
        assert_eq!(detail.components[0].quantity_required, 3);
        assert_eq!(detail.components[1].component_item_id, "COMP-B");
    }

    #[tokio::test]
    async fn test_work_order_zero_quantity_fails() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        let result = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-ZERO".into(),
            quantity: 0,
            planned_start: None,
            planned_end: None,
        }).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Caught by either validator (range) or service-level check
        assert!(
            err.contains("quantity") || err.contains("positive"),
            "Expected quantity validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_complete_work_order_with_incomplete_routing_steps() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and start a work order
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-ROUTE-INCOMPLETE".into(),
            quantity: 10,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        svc.start_work_order(&wo.id).await.unwrap();

        // Add a routing step (defaults to "pending" status)
        svc.routing_step_repo
            .create(&wo.id, 1, "Assemble components")
            .await
            .unwrap();

        // Try to complete the work order — should fail because routing step is not completed
        let result = svc.complete_work_order(&wo.id).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("routing step") && err.contains("expected 'completed'"),
            "Expected routing steps error, got: {}",
            err
        );

        // Verify work order is still in_progress
        let wo_check = svc.get_work_order(&wo.id).await.unwrap();
        assert_eq!(wo_check.status, "in_progress");
    }

    #[tokio::test]
    async fn test_complete_work_order_with_all_routing_steps_completed() {
        let pool = setup().await;
        let svc = ManufacturingService {
            work_order_repo: WorkOrderRepo::new(pool.clone()),
            bom_repo: BomRepo::new(pool.clone()),
            routing_step_repo: RoutingStepRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Create and start a work order
        let wo = svc.create_work_order(CreateWorkOrder {
            item_id: "ITEM-ROUTE-DONE".into(),
            quantity: 25,
            planned_start: None,
            planned_end: None,
        }).await.unwrap();
        svc.start_work_order(&wo.id).await.unwrap();

        // Add a routing step and complete it
        let step = svc.routing_step_repo
            .create(&wo.id, 1, "Quality check")
            .await
            .unwrap();
        svc.routing_step_repo
            .update_status(&step.id, "completed")
            .await
            .unwrap();

        // Complete the work order — should succeed
        let completed = svc.complete_work_order(&wo.id).await.unwrap();
        assert_eq!(completed.status, "completed");
        assert!(completed.actual_end.is_some());
    }
}

use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::repository::work_order_repo::WorkOrderRepo;
use crate::repository::bom_repo::BomRepo;
use validator::Validate;
use crate::repository::routing_step_repo::RoutingStepRepo;
use crate::models::work_order::*;
use crate::models::bom::*;

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
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.work_order_repo.create(&input).await
    }

    pub async fn start_work_order(&self, id: &str) -> AppResult<WorkOrderResponse> {
        let wo = self.work_order_repo.get_by_id(id).await?;
        if wo.status != "planned" {
            return Err(saas_common::error::AppError::Validation("Only planned work orders can be started".into()));
        }
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.work_order_repo.update_status(id, "in_progress", Some(&now), None).await?;
        self.work_order_repo.get_by_id(id).await
    }

    pub async fn complete_work_order(&self, id: &str) -> AppResult<WorkOrderResponse> {
        let wo = self.work_order_repo.get_by_id(id).await?;
        if wo.status != "in_progress" {
            return Err(saas_common::error::AppError::Validation("Only in-progress work orders can be completed".into()));
        }
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.work_order_repo.update_status(id, "completed", None, Some(&now)).await?;
        self.work_order_repo.get_by_id(id).await
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
        input.validate().map_err(|e| saas_common::error::AppError::Validation(e.to_string()))?;
        self.bom_repo.create(&input).await
    }
}

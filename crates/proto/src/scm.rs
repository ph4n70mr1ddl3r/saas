use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sku(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarehouseId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemType {
    Finished,
    RawMaterial,
    Component,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MovementType {
    Receipt,
    Transfer,
    Pick,
    Adjustment,
    Return,
}

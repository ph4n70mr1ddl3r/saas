use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCode(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JournalEntryStatus {
    Draft,
    Posted,
    Reversed,
}

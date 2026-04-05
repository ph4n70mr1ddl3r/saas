use serde::{Deserialize, Serialize};

// HCM Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeCreated {
    pub employee_id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub department_id: String,
    pub hire_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeUpdated {
    pub employee_id: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeTerminated {
    pub employee_id: String,
    pub termination_date: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayRunCompleted {
    pub pay_run_id: String,
    pub period_start: String,
    pub period_end: String,
    pub payslip_count: u32,
    pub total_net_pay_cents: i64,
}

// HCM Recruiting Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationStatusChanged {
    pub application_id: String,
    pub job_id: String,
    pub candidate_email: String,
    pub old_status: String,
    pub new_status: String,
}

// ERP Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntryPosted {
    pub entry_id: String,
    pub entry_number: String,
    pub lines: Vec<JournalLinePosted>,
    pub posted_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalLinePosted {
    pub account_code: String,
    pub debit_cents: i64,
    pub credit_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorInvoiceApproved {
    pub invoice_id: String,
    pub vendor_id: String,
    pub total_cents: i64,
    pub gl_account_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerInvoiceCreated {
    pub invoice_id: String,
    pub customer_id: String,
    pub total_cents: i64,
}

// SCM Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockReceived {
    pub item_id: String,
    pub warehouse_id: String,
    pub location_id: String,
    pub quantity: i64,
    pub reference_type: String,
    pub reference_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockReserved {
    pub item_id: String,
    pub warehouse_id: String,
    pub quantity: i64,
    pub reference_type: String,
    pub reference_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOrderConfirmed {
    pub order_id: String,
    pub order_number: String,
    pub customer_id: String,
    pub lines: Vec<SalesOrderLineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOrderLineEvent {
    pub item_id: String,
    pub quantity: i64,
    pub warehouse_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderReceived {
    pub po_id: String,
    pub supplier_id: String,
    pub lines: Vec<PurchaseOrderLineReceived>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderLineReceived {
    pub item_id: String,
    pub warehouse_id: String,
    pub quantity_received: i64,
}

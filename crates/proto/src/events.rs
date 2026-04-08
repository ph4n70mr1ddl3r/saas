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
    pub candidate_first_name: String,
    pub candidate_last_name: String,
    pub candidate_email: String,
    pub job_title: String,
    pub department_id: String,
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
pub struct ItemBelowReorderPoint {
    pub item_id: String,
    pub item_name: String,
    pub sku: String,
    pub warehouse_id: String,
    pub available_quantity: i64,
    pub reorder_point: i64,
    pub suggested_order_quantity: i64,
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
    pub unit_price_cents: i64,
}

// HCM Performance Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCycleActivated {
    pub cycle_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSubmitted {
    pub assignment_id: String,
    pub cycle_id: String,
    pub employee_id: String,
    pub reviewer_id: String,
    pub rating: i32,
}

// ERP Expense Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseReportApproved {
    pub report_id: String,
    pub employee_id: String,
    pub total_cents: i64,
    pub gl_account_code: String,
}

// SCM Inventory Cycle Count Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleCountPosted {
    pub session_id: String,
    pub warehouse_id: String,
    pub adjustment_count: u32,
}

// HCM Benefits Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenefitPlanCreated {
    pub plan_id: String,
    pub name: String,
    pub plan_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenefitPlanDeactivated {
    pub plan_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeEnrolled {
    pub enrollment_id: String,
    pub employee_id: String,
    pub plan_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentCancelled {
    pub enrollment_id: String,
    pub employee_id: String,
    pub plan_id: String,
}

// ERP General Ledger Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntryReversed {
    pub entry_id: String,
    pub original_entry_id: String,
    pub reversed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodClosed {
    pub period_id: String,
    pub name: String,
    pub fiscal_year: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetActivated {
    pub budget_id: String,
    pub name: String,
    pub total_budget_cents: i64,
}

// ERP Fixed Assets Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCreated {
    pub asset_id: String,
    pub name: String,
    pub asset_number: String,
    pub category: String,
    pub purchase_cost_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDisposed {
    pub asset_id: String,
    pub name: String,
    pub asset_number: String,
    pub cost_cents: i64,
    pub accumulated_depreciation_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepreciationRunCompleted {
    pub period: String,
    pub asset_count: u32,
    pub total_depreciation_cents: i64,
}

// ERP Cash Management Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccountCreated {
    pub account_id: String,
    pub name: String,
    pub bank_name: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferCompleted {
    pub from_account_id: String,
    pub to_account_id: String,
    pub amount_cents: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationCompleted {
    pub reconciliation_id: String,
    pub bank_account_id: String,
    pub book_balance_cents: i64,
    pub statement_balance_cents: i64,
    pub difference_cents: i64,
}

// ERP Expense Management Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseReportSubmitted {
    pub report_id: String,
    pub employee_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseReportRejected {
    pub report_id: String,
    pub employee_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseReportPaid {
    pub report_id: String,
    pub employee_id: String,
    pub total_cents: i64,
}

// HCM Time & Labor Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimesheetSubmitted {
    pub timesheet_id: String,
    pub employee_id: String,
    pub week_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimesheetApproved {
    pub timesheet_id: String,
    pub employee_id: String,
    pub week_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimesheetRejected {
    pub timesheet_id: String,
    pub employee_id: String,
    pub week_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRequestSubmitted {
    pub request_id: String,
    pub employee_id: String,
    pub leave_type: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRequestApproved {
    pub request_id: String,
    pub employee_id: String,
    pub leave_type: String,
    pub days: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRequestRejected {
    pub request_id: String,
    pub employee_id: String,
    pub leave_type: String,
}

// IAM Role Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleCreated {
    pub role_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleUpdated {
    pub role_id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermissionsChanged {
    pub role_id: String,
    pub permission_count: u32,
}

// Config Service Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigUpdated {
    pub key: String,
    pub value: String,
}

// IAM Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRevoked {
    pub jti: String,
    pub user_id: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeactivated {
    pub user_id: String,
    pub username: String,
}

// SCM Manufacturing Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrderCompleted {
    pub work_order_id: String,
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrderCancelled {
    pub work_order_id: String,
    pub item_id: String,
    pub quantity: i64,
    pub reason: Option<String>,
}

// SCM Order Management Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFulfilled {
    pub order_id: String,
    pub order_number: String,
    pub customer_id: String,
    pub lines: Vec<OrderFulfilledLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFulfilledLine {
    pub item_id: String,
    pub quantity: i64,
    pub warehouse_id: String,
    pub unit_price_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnCreated {
    pub return_id: String,
    pub order_id: String,
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnApproved {
    pub return_id: String,
    pub order_id: String,
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnProcessed {
    pub return_id: String,
    pub order_id: String,
    pub refund_amount_cents: i64,
}

// IAM Role Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDeleted {
    pub role_id: String,
    pub name: String,
}

// SCM Order Management Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOrderCancelled {
    pub order_id: String,
    pub order_number: String,
    pub customer_id: String,
    pub reason: Option<String>,
}

// SCM Procurement Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderCancelled {
    pub po_id: String,
    pub supplier_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderSubmitted {
    pub po_id: String,
    pub supplier_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderApproved {
    pub po_id: String,
    pub supplier_id: String,
}

// ERP Payment Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApPaymentCreated {
    pub payment_id: String,
    pub invoice_id: String,
    pub vendor_id: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArReceiptCreated {
    pub receipt_id: String,
    pub invoice_id: String,
    pub customer_id: String,
    pub amount_cents: i64,
}

// AR Invoice Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArInvoiceApproved {
    pub invoice_id: String,
    pub customer_id: String,
    pub total_cents: i64,
}

// AP Invoice Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApInvoiceCancelled {
    pub invoice_id: String,
    pub vendor_id: String,
}

// AR Invoice Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArInvoiceCancelled {
    pub invoice_id: String,
    pub customer_id: String,
}

// GL Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YearEndClosed {
    pub fiscal_year: i32,
    pub entry_id: String,
}

// Manufacturing Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrderStarted {
    pub work_order_id: String,
    pub item_id: String,
    pub quantity: i64,
}

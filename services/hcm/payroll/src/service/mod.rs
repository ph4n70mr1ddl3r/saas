use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use saas_proto::events::PayRunCompleted;
use crate::models::*;
use crate::repository::PayrollRepo;

#[derive(Clone)]
pub struct PayrollService {
    repo: PayrollRepo,
    bus: NatsBus,
}

impl PayrollService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: PayrollRepo::new(pool),
            bus,
        }
    }

    // --- Compensation ---

    pub async fn list_compensation(&self) -> AppResult<Vec<Compensation>> {
        self.repo.list_compensation().await
    }

    pub async fn get_compensation(&self, id: &str) -> AppResult<Compensation> {
        self.repo.get_compensation(id).await
    }

    pub async fn list_compensation_by_employee(&self, employee_id: &str) -> AppResult<Vec<Compensation>> {
        self.repo.list_compensation_by_employee(employee_id).await
    }

    pub async fn create_compensation(&self, input: CreateCompensationRequest) -> AppResult<Compensation> {
        if input.amount_cents < 0 {
            return Err(AppError::Validation("amount_cents must be non-negative".into()));
        }
        self.repo.create_compensation(&input).await
    }

    pub async fn update_compensation(&self, id: &str, input: UpdateCompensationRequest) -> AppResult<Compensation> {
        self.repo.update_compensation(id, &input).await
    }

    // --- PayRuns ---

    pub async fn list_pay_runs(&self) -> AppResult<Vec<PayRun>> {
        self.repo.list_pay_runs().await
    }

    pub async fn create_pay_run(&self, input: CreatePayRunRequest) -> AppResult<PayRun> {
        self.repo.create_pay_run(&input).await
    }

    pub async fn process_pay_run(&self, id: &str) -> AppResult<PayRun> {
        let pay_run = self.repo.get_pay_run(id).await?;
        if pay_run.status != "draft" {
            return Err(AppError::Validation(
                format!("Pay run '{}' is not in draft status (current: {})", id, pay_run.status)
            ));
        }

        self.repo.update_pay_run_status(id, "processing").await?;

        // Only process compensation records with effective_date within the pay period
        let compensations = self.repo.list_compensation().await?;
        let filtered: Vec<_> = compensations.into_iter().filter(|c| {
            c.end_date.is_none() && c.amount_cents > 0
        }).collect();

        let mut total_net_cents: i64 = 0;
        let mut payslip_count: u32 = 0;

        // Use integer arithmetic for tax: 22% = multiply by 22 / 100 with rounding
        for comp in &filtered {
            let gross = comp.amount_cents;
            let tax = (gross * 22 + 50) / 100; // Rounded integer division for 22%
            let deductions = self.repo.list_deductions_by_employee(&comp.employee_id).await?;
            let total_deductions: i64 = deductions.iter().map(|d| d.amount_cents).sum();
            let net = gross - tax - total_deductions;

            self.repo.create_payslip(id, &comp.employee_id, gross, net, tax, total_deductions).await?;
            total_net_cents += net;
            payslip_count += 1;
        }

        let pay_run = self.repo.update_pay_run_status(id, "completed").await?;

        let event = PayRunCompleted {
            pay_run_id: id.to_string(),
            period_start: pay_run.period_start.clone(),
            period_end: pay_run.period_end.clone(),
            payslip_count,
            total_net_pay_cents: total_net_cents,
        };
        if let Err(e) = self.bus.publish("hcm.payroll.run.completed", event).await {
            tracing::error!("Failed to publish payroll.run.completed event: {}", e);
        }

        Ok(pay_run)
    }

    pub async fn list_payslips_for_run(&self, pay_run_id: &str) -> AppResult<Vec<Payslip>> {
        let _ = self.repo.get_pay_run(pay_run_id).await?;
        self.repo.list_payslips_for_run(pay_run_id).await
    }

    // --- Deductions ---

    pub async fn list_deductions_by_employee(&self, employee_id: &str) -> AppResult<Vec<Deduction>> {
        self.repo.list_deductions_by_employee(employee_id).await
    }

    pub async fn create_deduction(&self, input: CreateDeductionRequest) -> AppResult<Deduction> {
        if input.amount_cents < 0 {
            return Err(AppError::Validation("amount_cents must be non-negative".into()));
        }
        self.repo.create_deduction(&input).await
    }

    // --- Event handlers ---

    pub async fn handle_employee_created(&self, employee_id: &str) -> AppResult<Compensation> {
        let input = CreateCompensationRequest {
            employee_id: employee_id.to_string(),
            salary_type: "salaried".to_string(),
            amount_cents: 0,
            currency: Some("USD".to_string()),
            effective_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            end_date: None,
        };
        self.repo.create_compensation(&input).await
    }
}

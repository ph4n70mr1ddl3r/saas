use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayGradeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmployeeStatus {
    Active,
    OnLeave,
    Terminated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmploymentType {
    FullTime,
    PartTime,
    Contract,
    Intern,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    HcmAdmin,
    HcmViewer,
    ErpAdmin,
    ErpViewer,
    ScmAdmin,
    ScmViewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Permission {
    pub resource: String,
    pub action: String,
}

impl Permission {
    pub fn new(resource: &str, action: &str) -> Self {
        Self { resource: resource.to_string(), action: action.to_string() }
    }

    pub fn code(&self) -> String {
        format!("{}:{}", self.resource, self.action)
    }
}

pub fn has_permission(roles: &[String], required: &str) -> bool {
    if roles.iter().any(|r| r.to_lowercase() == "admin") {
        return true;
    }
    roles.iter().any(|r| r == required)
}

/// Check if the authenticated user has the admin role.
pub fn is_admin(roles: &[String]) -> bool {
    roles.iter().any(|r| r.to_lowercase() == "admin")
}

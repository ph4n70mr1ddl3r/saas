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
    if is_admin(roles) {
        return true;
    }
    let required_lower = required.to_lowercase();
    roles.iter().any(|r| r.to_lowercase() == required_lower)
}

/// Check if the authenticated user has the admin role.
pub fn is_admin(roles: &[String]) -> bool {
    roles.iter().any(|r| r.eq_ignore_ascii_case("admin"))
}

/// Check if the user has any of the specified domain admin roles.
pub fn is_domain_admin(roles: &[String], domain: &str) -> bool {
    if is_admin(roles) {
        return true;
    }
    let admin_role = format!("{}_admin", domain);
    roles.iter().any(|r| r.eq_ignore_ascii_case(&admin_role))
}

/// Require admin or domain-specific admin role, returning a 403 error string if not.
pub fn require_admin(roles: &[String], domain: &str) -> Result<(), String> {
    if is_domain_admin(roles, domain) {
        return Ok(());
    }
    let admin_role = format!("{}_admin", domain);
    Err(format!("Admin or {} role required", admin_role))
}

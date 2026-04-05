use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PaginationParams {
    pub fn offset(&self) -> u32 {
        (self.page().saturating_sub(1)) * self.per_page()
    }
    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).min(100).max(1)
    }
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }
}

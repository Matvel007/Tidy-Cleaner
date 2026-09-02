use crate::applications::models::{ApplicationItem, PackageSource};
use anyhow::Result;

#[allow(dead_code)]
pub trait PackageManagerProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> PackageSource;
    fn is_available(&self) -> bool;
    fn list_installed(&self) -> Result<Vec<ApplicationItem>>;
    fn uninstall(&self, package_id: &str) -> Result<()>;
    fn get_details(&self, package_id: &str) -> Result<Option<String>>;
}

//! Uma funcao por achado. Cada uma le so `dashboard/` e devolve zero ou mais achados.

use super::Finding;
use crate::dashboard::dataset::Dataset;

pub(super) fn pace(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}
pub(super) fn forecast(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}
pub(super) fn categories(_ds: &Dataset, _today: &str) -> Vec<Finding> {
    Vec::new()
}
pub(super) fn budgets(_ds: &Dataset, _today: &str) -> Vec<Finding> {
    Vec::new()
}
pub(super) fn savings(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}
pub(super) fn fixed_weight(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}
pub(super) fn next_month(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}
pub(super) fn emergency(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}
pub(super) fn invoices(_ds: &Dataset, _today: &str) -> Option<Finding> {
    None
}

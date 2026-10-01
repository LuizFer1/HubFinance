//! Interpretacao financeira das linhas: puro, sem rusqlite, iced, axum ou tokio.

// O tema (ui/theme.rs) usa parte da paleta; o resto ganha uso nas telas do plano 2b.
#![allow(dead_code)]

pub mod colors;
pub mod contract;
pub mod dataset;
pub mod money;
pub mod people;
pub mod periods;

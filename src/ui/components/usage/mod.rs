//! Shared, resource-agnostic usage visuals.
//!
//! These leaves render plain values. Pod and node widgets own their own models,
//! view models, and subscriptions.

mod chart;
mod meter;

pub(crate) use chart::{ChartPoint, ChartView, usage_chart};
pub(crate) use meter::{MeterView, usage_meter};

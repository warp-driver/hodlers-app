#![no_std]

mod contract;
mod error;
mod storage;

#[cfg(test)]
mod tests;

pub use contract::{Hodlers, HodlersClient};
pub use error::HodlersError;

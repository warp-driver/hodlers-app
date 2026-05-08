#![no_std]
extern crate alloc;

mod contract;
mod envelope;
mod error;
mod storage;

#[cfg(test)]
mod tests;

pub use contract::{SignatureData, StellarHandler, StellarHandlerClient};
pub use error::HandlerError;

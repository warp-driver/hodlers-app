#![no_std]
extern crate alloc;

mod contract;
mod storage;

#[cfg(test)]
mod tests;

pub use contract::{HodlersPayload, StellarHandler, StellarHandlerClient};
pub use warpdrive_shared::interfaces::handler::{Ed25519SignatureData, HandlerError};

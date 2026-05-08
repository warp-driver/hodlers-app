#![no_std]

mod contract;
pub mod storage;

pub use contract::Secp256k1Security;
pub use contract::Secp256k1SecurityClient;
pub use warpdrive_shared::interfaces::security::SecurityError;

#[cfg(test)]
mod tests;

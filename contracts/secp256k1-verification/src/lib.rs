#![no_std]

mod contract;
pub mod storage;
pub mod utils;

pub use contract::Secp256k1Verification;
pub use contract::Secp256k1VerificationClient;
pub use warpdrive_shared::VerifyError;

#[cfg(test)]
mod tests;

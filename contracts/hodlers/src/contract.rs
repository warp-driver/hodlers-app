use soroban_sdk::{contract, contractimpl, Address, Env};

use crate::error::HodlersError;
use crate::storage;

#[contract]
pub struct Hodlers;

#[contractimpl]
impl Hodlers {
    pub fn add_points(env: Env, trader: Address, delta: i128) -> Result<i128, HodlersError> {
        let current = storage::get_points(&env, &trader);
        let next = current.checked_add(delta).ok_or(if delta >= 0 {
            HodlersError::Overflow
        } else {
            HodlersError::Underflow
        })?;
        storage::set_points(&env, &trader, next);
        Ok(next)
    }

    pub fn points_of(env: Env, trader: Address) -> i128 {
        storage::get_points(&env, &trader)
    }
}

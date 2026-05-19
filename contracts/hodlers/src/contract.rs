use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

use crate::error::HodlersError;
use crate::storage::{self, TraderPoints};

#[contract]
pub struct Hodlers;

#[contractimpl]
impl Hodlers {
    pub fn add_points(env: Env, trader: Address, delta: i128) -> Result<i128, HodlersError> {
        let first_time = !storage::has_seen_trader(&env, &trader);
        let current = storage::get_points(&env, &trader);
        let next = current.checked_add(delta).ok_or(if delta >= 0 {
            HodlersError::Overflow
        } else {
            HodlersError::Underflow
        })?;
        storage::set_points(&env, &trader, next);
        if first_time {
            storage::append_trader(&env, &trader);
        }
        Ok(next)
    }

    pub fn points_of(env: Env, trader: Address) -> i128 {
        storage::get_points(&env, &trader)
    }

    /// Returns every trader that has ever been credited, paired with their
    /// current point total. Caller (typically a frontend) sorts and paginates
    /// however it wants. Unbounded in principle — fine while the trader set
    /// stays in the low thousands; revisit if it grows past that.
    pub fn all_points(env: Env) -> Vec<TraderPoints> {
        let traders = storage::get_traders(&env);
        let mut out = Vec::new(&env);
        for i in 0..traders.len() {
            let trader = traders.get_unchecked(i);
            let points = storage::get_points(&env, &trader);
            out.push_back(TraderPoints { trader, points });
        }
        out
    }
}

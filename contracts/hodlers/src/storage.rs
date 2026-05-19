use soroban_sdk::{contracttype, Address, Env, Vec};
use warpdrive_shared::ttl;

#[contracttype]
#[derive(Clone)]
pub struct TraderPoints {
    pub trader: Address,
    pub points: i128,
}

#[contracttype]
pub enum DataKey {
    Points(Address),
    /// All addresses that have ever been credited at least once. We can't
    /// enumerate per-trader persistent storage keys, so we maintain the
    /// list ourselves and append on first sighting.
    Traders,
}

pub fn get_points(env: &Env, trader: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Points(trader.clone()))
        .unwrap_or(0)
}

pub fn set_points(env: &Env, trader: &Address, balance: i128) {
    let key = DataKey::Points(trader.clone());
    env.storage().persistent().set(&key, &balance);
    env.storage().persistent().extend_ttl(
        &key,
        ttl::PERSISTENT_RENEWAL_THRESHOLD,
        ttl::PERSISTENT_TARGET_TTL,
    );
}

pub fn has_seen_trader(env: &Env, trader: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Points(trader.clone()))
}

pub fn get_traders(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Traders)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn append_trader(env: &Env, trader: &Address) {
    let mut traders = get_traders(env);
    traders.push_back(trader.clone());
    env.storage().instance().set(&DataKey::Traders, &traders);
    env.storage()
        .instance()
        .extend_ttl(ttl::INSTANCE_RENEWAL_THRESHOLD, ttl::INSTANCE_TARGET_TTL);
}

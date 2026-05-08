use soroban_sdk::{contracttype, Address, Env};
use warpdrive_shared::ttl;

#[contracttype]
pub enum DataKey {
    Points(Address),
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

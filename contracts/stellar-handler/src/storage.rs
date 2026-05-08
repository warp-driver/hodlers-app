use soroban_sdk::{contracttype, Address, BytesN, Env, String};
use warpdrive_shared::ttl;

#[contracttype]
pub enum DataKey {
    VerificationContract,
    HodlersContract,
    Version,
    EventSeen(BytesN<20>),
}

pub fn set_verification_contract(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::VerificationContract, addr);
}

pub fn get_verification_contract(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::VerificationContract)
        .expect("verification contract not set")
}

pub fn set_hodlers_contract(env: &Env, addr: &Address) {
    env.storage().instance().set(&DataKey::HodlersContract, addr);
}

pub fn get_hodlers_contract(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::HodlersContract)
        .expect("hodlers contract not set")
}

pub fn set_version(env: &Env, v: &String) {
    env.storage().instance().set(&DataKey::Version, v);
}

pub fn is_event_seen(env: &Env, event_id: &BytesN<20>) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::EventSeen(event_id.clone()))
}

pub fn mark_event_seen(env: &Env, event_id: &BytesN<20>) {
    let key = DataKey::EventSeen(event_id.clone());
    env.storage().persistent().set(&key, &true);
    env.storage().persistent().extend_ttl(
        &key,
        ttl::PERSISTENT_RENEWAL_THRESHOLD,
        ttl::PERSISTENT_TARGET_TTL,
    );
}

pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(ttl::INSTANCE_RENEWAL_THRESHOLD, ttl::INSTANCE_TARGET_TTL);
}

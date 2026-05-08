use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

use crate::{error::HodlersError, Hodlers, HodlersClient};

fn setup(env: &Env) -> HodlersClient<'_> {
    let id = env.register(Hodlers, ());
    HodlersClient::new(env, &id)
}

#[test]
fn starts_at_zero() {
    let env = Env::default();
    let trader = Address::generate(&env);
    let client = setup(&env);

    assert_eq!(client.points_of(&trader), 0);
}

#[test]
fn add_and_subtract_points() {
    let env = Env::default();
    let trader = Address::generate(&env);
    let client = setup(&env);

    assert_eq!(client.add_points(&trader, &100), 100);
    assert_eq!(client.add_points(&trader, &50), 150);
    assert_eq!(client.add_points(&trader, &-30), 120);
    assert_eq!(client.points_of(&trader), 120);
}

#[test]
fn add_points_overflow_returns_error() {
    let env = Env::default();
    let trader = Address::generate(&env);
    let client = setup(&env);

    client.add_points(&trader, &i128::MAX);
    let result = client.try_add_points(&trader, &1);
    assert_eq!(result, Err(Ok(HodlersError::Overflow)));
}

#[test]
fn add_points_underflow_returns_error() {
    let env = Env::default();
    let trader = Address::generate(&env);
    let client = setup(&env);

    client.add_points(&trader, &i128::MIN);
    let result = client.try_add_points(&trader, &-1);
    assert_eq!(result, Err(Ok(HodlersError::Underflow)));
}

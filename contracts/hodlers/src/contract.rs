use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct Hodlers;

#[contractimpl]
impl Hodlers {
    // init(admin), add_points(trader, delta), points_of(trader), set_admin(new).
}

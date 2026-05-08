mod phoenix;
mod state;

#[cfg(test)]
mod tests;

wit_bindgen::generate!({
    world: "circuit-world",
    path: "../../wit-definitions/wit",
    generate_all,
});

use alloy_sol_types::{sol, SolValue};
use warpdrive::vectr::input::TriggerData;

sol! {
    struct HodlersPayload {
        string trader;
        int128 delta;
    }
}

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        run_inner(trigger_action).map_err(|e| format!("{e:?}"))
    }
}

fn run_inner(trigger_action: TriggerAction) -> anyhow::Result<Vec<WasmResponse>> {
    let event = match trigger_action.data {
        TriggerData::StellarContractEvent(e) => e.event,
        _ => anyhow::bail!("expected StellarContractEvent trigger"),
    };

    let key = format!(
        "{}:{}",
        event.transaction_hash,
        event.operation_index.unwrap_or(0)
    );

    let update = phoenix::decode_field(&event.topic_segments, &event.value)?;
    let mut current = state::load(&key)?;
    phoenix::apply(&mut current, update);

    if let Some((trader, delta)) = phoenix::try_finalize(&current) {
        state::delete(&key)?;
        let payload = HodlersPayload { trader, delta }.abi_encode();
        return Ok(vec![WasmResponse {
            payload,
            ordering: None,
            event_id_salt: None,
        }]);
    }

    state::save(&key, &current)?;
    Ok(vec![])
}

export!(Component);

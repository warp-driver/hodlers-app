mod payload;
mod phoenix;
mod state;

#[cfg(test)]
mod tests;

wit_bindgen::generate!({
    world: "circuit-world",
    path: "../../wit-definitions/wit",
    generate_all,
});

use warpdrive::vectr::input::TriggerData;

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

    // CAS-update the accumulator. Returns the (trader, delta) payload only
    // if THIS invocation is the one that completed the SwapState and flipped
    // `finalized` true — wasi:keyvalue/atomics makes the read-modify-write
    // safe under the 8 concurrent events that fire per Phoenix swap.
    let to_emit = state::update_with(&key, |s| {
        phoenix::apply(s, update.clone());
        if !s.finalized {
            if let Some(payload) = phoenix::try_finalize(s) {
                s.finalized = true;
                return Some(payload);
            }
        }
        None
    })?;

    if let Some((trader, delta)) = to_emit {
        let bytes = payload::encode(&trader, delta)?;
        return Ok(vec![WasmResponse {
            payload: bytes,
            ordering: None,
            event_id_salt: None,
        }]);
    }

    Ok(vec![])
}

export!(Component);

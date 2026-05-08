wit_bindgen::generate!({
    world: "circuit-world",
    path: "../wit-definitions/wit",
    generate_all,
});

struct Component;

impl Guest for Component {
    fn run(_trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        Ok(vec![WasmResponse {
            payload: vec![],
            ordering: None,
            event_id_salt: None,
        }])
    }
}

export!(Component);

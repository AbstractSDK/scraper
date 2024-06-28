pub struct AbstractState(serde_json::Value);

impl Default for AbstractState {
    fn default() -> Self {
        Self(abstract_interface::State::load_state())
    }
}

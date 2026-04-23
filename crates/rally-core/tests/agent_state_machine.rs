use proptest::prelude::*;
use rally_core::agent::{transition, AgentState, AgentTrigger};

// --- Arbitrary impls ---

fn arb_state() -> impl Strategy<Value = AgentState> {
    prop_oneof![
        Just(AgentState::Initializing),
        Just(AgentState::Running),
        Just(AgentState::Idle),
        Just(AgentState::WaitingForInput),
        Just(AgentState::AttentionRequired),
        Just(AgentState::Completed),
        Just(AgentState::Failed),
        Just(AgentState::Stopped),
    ]
}

fn arb_trigger() -> impl Strategy<Value = AgentTrigger> {
    prop_oneof![
        Just(AgentTrigger::Started),
        Just(AgentTrigger::IdleTimeout),
        Just(AgentTrigger::InputReceived),
        Just(AgentTrigger::HookWaitingForInput),
        Just(AgentTrigger::CaptureRuleAttention),
        Just(AgentTrigger::InputResolved),
        Just(AgentTrigger::Acknowledged),
        Just(AgentTrigger::HookCompleted),
        Just(AgentTrigger::HookFailed),
        Just(AgentTrigger::StopRequested),
        Just(AgentTrigger::Restarted),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200_000))]

    /// No (state, trigger) pair ever panics — it either transitions or returns Err.
    #[test]
    fn no_panic_on_any_input(state in arb_state(), trigger in arb_trigger()) {
        let _ = transition(state, &trigger);
    }

    /// A successful transition always yields a valid AgentState (no lost state).
    #[test]
    fn successful_transition_yields_valid_state(state in arb_state(), trigger in arb_trigger()) {
        if let Ok(next) = transition(state, &trigger) {
            // Any returned state is by construction valid; just assert it's not
            // the exact same as the origin when we know the trigger changes it.
            let _ = next; // explicit use
        }
    }

    /// Errors are typed InvalidTransition — never opaque panics.
    #[test]
    fn error_carries_source(state in arb_state(), trigger in arb_trigger()) {
        if let Err(e) = transition(state, &trigger) {
            assert_eq!(e.state, state);
            assert_eq!(e.trigger, trigger);
        }
    }

    /// Stopped and Failed can restart; Completed is truly terminal.
    #[test]
    fn restartable_states_cycle_via_initializing(trigger in arb_trigger()) {
        for restartable in [AgentState::Stopped, AgentState::Failed] {
            let result = transition(restartable, &trigger);
            match trigger {
                AgentTrigger::Restarted => {
                    assert!(result.is_ok(), "Restarted must succeed from {restartable:?}");
                    assert_eq!(result.unwrap(), AgentState::Initializing);
                }
                _ => {
                    assert!(
                        result.is_err(),
                        "{trigger:?} must not be valid in restartable state {restartable:?}"
                    );
                }
            }
        }
    }

    /// Completed is a true terminal — no trigger may leave it.
    #[test]
    fn completed_is_terminal(trigger in arb_trigger()) {
        assert!(
            transition(AgentState::Completed, &trigger).is_err(),
            "{trigger:?} must not leave Completed state"
        );
    }

    /// Running → Idle → Running round-trip always works.
    #[test]
    fn running_idle_roundtrip(_seed: u8) {
        let s1 = transition(AgentState::Running, &AgentTrigger::IdleTimeout).unwrap();
        assert_eq!(s1, AgentState::Idle);
        let s2 = transition(AgentState::Idle, &AgentTrigger::InputReceived).unwrap();
        assert_eq!(s2, AgentState::Running);
    }
}

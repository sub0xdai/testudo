// @anchor test:cli:daemon
// @tags infra

use testudo_cli::daemon::DaemonState;

#[test]
fn daemon_state_serializes_to_json() {
    let state = DaemonState {
        phase: "Observing".into(),
        signal_count: 5,
        uptime_secs: 120,
        last_error: None,
    };

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("Observing"));
    assert!(json.contains("5"));
    assert!(json.contains("120"));
}

#[test]
fn daemon_state_default_is_idle() {
    let state = DaemonState::default();
    assert_eq!(state.phase, "Idle");
    assert_eq!(state.signal_count, 0);
    assert_eq!(state.uptime_secs, 0);
    assert!(state.last_error.is_none());
}

#[test]
fn daemon_state_deserializes_from_json() {
    let json = r#"{"phase":"Acting","signal_count":3,"uptime_secs":45,"last_error":null}"#;
    let state: DaemonState = serde_json::from_str(json).unwrap();
    assert_eq!(state.phase, "Acting");
    assert_eq!(state.signal_count, 3);
}

#[test]
fn daemon_state_with_error() {
    let state = DaemonState {
        phase: "Idle".into(),
        signal_count: 0,
        uptime_secs: 0,
        last_error: Some("API timeout".into()),
    };

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("API timeout"));
}

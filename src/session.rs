/// FIX session state machine.
///
/// Manages session lifecycle: logon, heartbeat, sequencing, gap detection,
/// logout, and reconnection. All state transitions are deterministic.

use std::time::{Duration, Instant};

/// Session state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Disconnected,
    Connecting,
    LogonSent,
    Active,
    Resending,
    LogoutSent,
}

/// Session role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Initiator,
    Acceptor,
}

/// Sequence number reset policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceResetPolicy {
    Always,
    Daily,
    Weekly,
    Never,
}

/// Session configuration.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub session_id: String,
    pub fix_version: String,
    pub sender_comp_id: String,
    pub target_comp_id: String,
    pub role: SessionRole,
    pub heartbeat_interval: Duration,
    pub reconnect_interval: Duration,
    pub max_reconnect_attempts: u32,
    pub sequence_reset_policy: SequenceResetPolicy,
    pub validate_comp_ids: bool,
    pub max_msg_rate: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            session_id: String::new(),
            fix_version: "FIX.4.4".to_string(),
            sender_comp_id: String::new(),
            target_comp_id: String::new(),
            role: SessionRole::Initiator,
            heartbeat_interval: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(1),
            max_reconnect_attempts: 0,
            sequence_reset_policy: SequenceResetPolicy::Daily,
            validate_comp_ids: true,
            max_msg_rate: 50_000,
        }
    }
}

/// Actions that the session state machine requests the transport layer to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    /// Send a FIX message.
    Send(Vec<u8>),
    /// Disconnect the transport.
    Disconnect,
    /// No action needed.
    None,
}

/// FIX session — manages state, sequencing, and heartbeats.
pub struct Session {
    config: SessionConfig,
    state: SessionState,

    // Sequence numbers
    outbound_seq_num: u64,
    inbound_seq_num: u64,

    // Heartbeat tracking
    last_sent_time: Instant,
    last_received_time: Instant,
    test_request_pending: bool,
    test_request_sent_time: Option<Instant>,

    // Reconnection
    reconnect_attempts: u32,

    // Rate limiting
    msg_count_window: u32,
    window_start: Instant,
}

impl Session {
    /// Create a new session with the given configuration.
    pub fn new(config: SessionConfig) -> Self {
        let now = Instant::now();
        Session {
            config,
            state: SessionState::Disconnected,
            outbound_seq_num: 1,
            inbound_seq_num: 1,
            last_sent_time: now,
            last_received_time: now,
            test_request_pending: false,
            test_request_sent_time: None,
            reconnect_attempts: 0,
            msg_count_window: 0,
            window_start: now,
        }
    }

    /// Get the current session state.
    #[inline]
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Get the session configuration.
    #[inline]
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Get the next outbound sequence number (and increment).
    #[inline]
    pub fn next_outbound_seq_num(&mut self) -> u64 {
        let seq = self.outbound_seq_num;
        self.outbound_seq_num += 1;
        seq
    }

    /// Get the expected inbound sequence number.
    #[inline]
    pub fn expected_inbound_seq_num(&self) -> u64 {
        self.inbound_seq_num
    }

    /// Get the current outbound sequence number (without incrementing).
    #[inline]
    pub fn current_outbound_seq_num(&self) -> u64 {
        self.outbound_seq_num
    }

    /// Handle a state transition event.
    pub fn on_connected(&mut self) {
        match self.state {
            SessionState::Disconnected | SessionState::Connecting => {
                if self.config.role == SessionRole::Initiator {
                    self.state = SessionState::LogonSent;
                } else {
                    // Acceptor waits for inbound Logon
                    self.state = SessionState::Connecting;
                }
                self.last_received_time = Instant::now();
                self.last_sent_time = Instant::now();
                self.reconnect_attempts = 0;
            }
            _ => {}
        }
    }

    /// Handle successful logon.
    pub fn on_logon(&mut self) {
        self.state = SessionState::Active;
        self.last_received_time = Instant::now();
        self.test_request_pending = false;
    }

    /// Handle inbound message sequence validation.
    /// Returns `Ok(())` if sequence is correct, or `Err` with the gap range.
    pub fn validate_inbound_seq(&mut self, received_seq: u64) -> Result<(), (u64, u64)> {
        if received_seq == self.inbound_seq_num {
            self.inbound_seq_num += 1;
            self.last_received_time = Instant::now();
            Ok(())
        } else if received_seq > self.inbound_seq_num {
            // Gap detected
            let gap_start = self.inbound_seq_num;
            let gap_end = received_seq;
            self.state = SessionState::Resending;
            Err((gap_start, gap_end))
        } else {
            // Duplicate or already processed — ignore
            Ok(())
        }
    }

    /// Handle gap fill completion.
    pub fn on_gap_filled(&mut self, new_seq: u64) {
        self.inbound_seq_num = new_seq;
        self.state = SessionState::Active;
    }

    /// Handle logout request.
    pub fn on_logout_sent(&mut self) {
        self.state = SessionState::LogoutSent;
    }

    /// Handle disconnect.
    pub fn on_disconnected(&mut self) {
        self.state = SessionState::Disconnected;
        self.test_request_pending = false;
    }

    /// Check if heartbeat should be sent (called periodically by the timer).
    pub fn check_heartbeat(&mut self, now: Instant) -> SessionAction {
        if self.state != SessionState::Active {
            return SessionAction::None;
        }

        let since_sent = now.duration_since(self.last_sent_time);
        let since_received = now.duration_since(self.last_received_time);

        // Send heartbeat if we haven't sent anything in heartbeat_interval
        if since_sent >= self.config.heartbeat_interval {
            self.last_sent_time = now;
            return SessionAction::Send(Vec::new()); // Caller fills in actual heartbeat
        }

        // Send TestRequest if we haven't received anything in heartbeat_interval + grace
        let timeout = self.config.heartbeat_interval + Duration::from_secs(5);
        if since_received >= timeout {
            if self.test_request_pending {
                // Already sent a TestRequest and got no response — disconnect
                return SessionAction::Disconnect;
            } else {
                self.test_request_pending = true;
                self.test_request_sent_time = Some(now);
                return SessionAction::Send(Vec::new()); // TestRequest
            }
        }

        SessionAction::None
    }

    /// Record that a message was sent (for heartbeat timing).
    #[inline]
    pub fn on_message_sent(&mut self) {
        self.last_sent_time = Instant::now();
    }

    /// Record that a message was received (for heartbeat timing).
    #[inline]
    pub fn on_message_received(&mut self) {
        self.last_received_time = Instant::now();
        self.test_request_pending = false;
    }

    /// Check if the session should attempt reconnection.
    pub fn should_reconnect(&self) -> bool {
        if self.state != SessionState::Disconnected {
            return false;
        }
        if self.config.role != SessionRole::Initiator {
            return false;
        }
        if self.config.max_reconnect_attempts > 0
            && self.reconnect_attempts >= self.config.max_reconnect_attempts
        {
            return false;
        }
        true
    }

    /// Increment reconnect attempt counter.
    pub fn on_reconnect_attempt(&mut self) {
        self.reconnect_attempts += 1;
        self.state = SessionState::Connecting;
    }

    /// Reset sequence numbers (e.g., at start of day).
    pub fn reset_sequences(&mut self) {
        self.outbound_seq_num = 1;
        self.inbound_seq_num = 1;
    }

    /// Check rate limit. Returns true if the message should be allowed.
    #[inline]
    pub fn check_rate_limit(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);

        if elapsed >= Duration::from_secs(1) {
            self.msg_count_window = 0;
            self.window_start = now;
        }

        self.msg_count_window += 1;
        self.msg_count_window <= self.config.max_msg_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SessionConfig {
        SessionConfig {
            session_id: "TEST-1".to_string(),
            fix_version: "FIX.4.4".to_string(),
            sender_comp_id: "SENDER".to_string(),
            target_comp_id: "TARGET".to_string(),
            role: SessionRole::Initiator,
            heartbeat_interval: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(1),
            max_reconnect_attempts: 5,
            sequence_reset_policy: SequenceResetPolicy::Daily,
            validate_comp_ids: true,
            max_msg_rate: 1000,
        }
    }

    #[test]
    fn test_session_initial_state() {
        let session = Session::new(test_config());
        assert_eq!(session.state(), SessionState::Disconnected);
        assert_eq!(session.expected_inbound_seq_num(), 1);
        assert_eq!(session.current_outbound_seq_num(), 1);
    }

    #[test]
    fn test_session_connect_logon_flow() {
        let mut session = Session::new(test_config());

        session.on_connected();
        assert_eq!(session.state(), SessionState::LogonSent);

        session.on_logon();
        assert_eq!(session.state(), SessionState::Active);
    }

    #[test]
    fn test_session_sequence_numbers() {
        let mut session = Session::new(test_config());
        session.on_connected();
        session.on_logon();

        assert_eq!(session.next_outbound_seq_num(), 1);
        assert_eq!(session.next_outbound_seq_num(), 2);
        assert_eq!(session.next_outbound_seq_num(), 3);

        assert!(session.validate_inbound_seq(1).is_ok());
        assert!(session.validate_inbound_seq(2).is_ok());
        assert_eq!(session.expected_inbound_seq_num(), 3);
    }

    #[test]
    fn test_session_gap_detection() {
        let mut session = Session::new(test_config());
        session.on_connected();
        session.on_logon();

        assert!(session.validate_inbound_seq(1).is_ok());
        // Gap: expected 2, received 5
        let result = session.validate_inbound_seq(5);
        assert_eq!(result, Err((2, 5)));
        assert_eq!(session.state(), SessionState::Resending);
    }

    #[test]
    fn test_session_disconnect_reconnect() {
        let mut session = Session::new(test_config());
        session.on_connected();
        session.on_logon();
        session.on_disconnected();

        assert_eq!(session.state(), SessionState::Disconnected);
        assert!(session.should_reconnect());

        session.on_reconnect_attempt();
        assert_eq!(session.state(), SessionState::Connecting);
    }

    #[test]
    fn test_session_max_reconnect_attempts() {
        let mut session = Session::new(test_config());

        for _ in 0..5 {
            session.on_reconnect_attempt();
            session.on_disconnected();
        }

        assert!(!session.should_reconnect());
    }

    #[test]
    fn test_session_reset_sequences() {
        let mut session = Session::new(test_config());
        session.next_outbound_seq_num();
        session.next_outbound_seq_num();
        assert_eq!(session.current_outbound_seq_num(), 3);

        session.reset_sequences();
        assert_eq!(session.current_outbound_seq_num(), 1);
        assert_eq!(session.expected_inbound_seq_num(), 1);
    }

    #[test]
    fn test_session_rate_limit() {
        let mut config = test_config();
        config.max_msg_rate = 3;
        let mut session = Session::new(config);

        assert!(session.check_rate_limit());
        assert!(session.check_rate_limit());
        assert!(session.check_rate_limit());
        assert!(!session.check_rate_limit());
    }

    #[test]
    fn test_acceptor_session() {
        let mut config = test_config();
        config.role = SessionRole::Acceptor;
        let mut session = Session::new(config);

        session.on_connected();
        assert_eq!(session.state(), SessionState::Connecting);
        assert!(!session.should_reconnect());
    }
}

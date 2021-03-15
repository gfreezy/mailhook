use log::trace;
use std::io;

// Empty response that sends nothing back to the client
pub(crate) const EMPTY_RESPONSE: Response = Response::empty();
// Start TLS handshake
pub(crate) const START_TLS: Response =
    Response::fixed_action(220, "Ready to start TLS", Action::UpgradeTls);
/// Response to indicate that the SMTP session finished
pub const GOODBYE: Response = Response::fixed(221, "Goodbye");
/// Authentication succeeded
pub const AUTH_OK: Response = Response::fixed(235, "Authentication succeeded");
/// OK response
pub const OK: Response = Response::fixed(250, "OK");
// Non-commital response to VERIFY command
pub(crate) const VERIFY_RESPONSE: Response = Response::fixed(252, "Maybe");
// Empty response sent as an auth challenge.
pub(crate) const EMPTY_AUTH_CHALLENGE: Response = Response::fixed(334, "");
/// Response sent to the client before accepting data
pub const START_DATA: Response = Response::fixed(354, "Start mail input; end with <CRLF>.<CRLF>");
// State machine is not accepting commands
pub(crate) const INVALID_STATE: Response =
    Response::fixed(421, "Internal service error, closing connection");
/// Service not available
pub const NO_SERVICE: Response = Response::fixed(421, "Service not available, closing connection");
/// Internal server error
pub const INTERNAL_ERROR: Response = Response::fixed(451, "Aborted: local error in processing");
/// Insufficient system storage
pub const OUT_OF_SPACE: Response = Response::fixed(452, "Insufficient system storage");
/// Authentication system is not working
pub const TEMP_AUTH_FAILURE: Response = Response::fixed(454, "Temporary authentication failure");
// Parser error
pub(crate) const SYNTAX_ERROR: Response = Response::fixed(500, "Syntax error");
// Parser found missing parameter
pub(crate) const MISSING_PARAMETER: Response = Response::fixed(502, "Missing parameter");
// Command is unexpected for the current state
pub(crate) const BAD_SEQUENCE_COMMANDS: Response = Response::fixed(503, "Bad sequence of commands");
/// User storage quota exceeded
pub const NO_STORAGE: Response = Response::fixed(552, "Exceeded storage allocation");
/// Authentication required
pub const AUTHENTICATION_REQUIRED: Response = Response::fixed(530, "Authentication required");
/// Bad authentication attempt
pub const INVALID_CREDENTIALS: Response = Response::fixed(535, "Invalid credentials");
/// Unknown user
pub const NO_MAILBOX: Response = Response::fixed(550, "Mailbox unavailable");
/// Error with HELO
pub const BAD_HELLO: Response = Response::fixed(550, "Bad HELO");
/// IP address on blocklists
pub const BLOCKED_IP: Response = Response::fixed(550, "IP address on blocklists");
/// Invalid mailbox name
pub const BAD_MAILBOX: Response = Response::fixed(553, "Mailbox name not allowed");
/// Error handling incoming message
pub const TRANSACTION_FAILED: Response = Response::fixed(554, "Transaction failed");

/// Response contains a code and message to be sent back to the client
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    /// The three digit response code
    pub code: u16,
    /// The text message
    message: Message,
    /// Is the response an error response?
    pub is_error: bool,
    /// The action to take after sending the response to the client
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Message {
    Fixed(&'static str),
    Custom(String),
    Dynamic(String, Vec<&'static str>),
    Empty,
}

/// Action indicates the recommended action to take on a response
#[derive(PartialEq, Clone, Debug)]
pub enum Action {
    /// Send the response and close the connection
    Close,
    /// Upgrade the connection to use TLS
    UpgradeTls,
    /// Do not reply, wait for the client to send more data
    NoReply,
    /// Send a reply and keep the connection open
    Reply,
}

impl Response {
    // A response that uses a fixed static string
    pub(crate) const fn fixed(code: u16, message: &'static str) -> Self {
        Self::fixed_action(code, message, Response::action_from_code(code))
    }

    const fn action_from_code(code: u16) -> Action {
        match code {
            221 | 421 => Action::Close,
            _ => Action::Reply,
        }
    }

    // A response that uses a fixed static string and a given action
    pub(crate) const fn fixed_action(code: u16, message: &'static str, action: Action) -> Self {
        Self {
            code,
            message: Message::Fixed(message),
            is_error: (code < 200 || code >= 400),
            action,
        }
    }

    /// Create an application defined response.
    pub const fn custom(code: u16, message: String) -> Self {
        Self {
            code,
            message: Message::Custom(message),
            is_error: (code < 200 || code >= 400),
            action: Response::action_from_code(code),
        }
    }

    // A response that is built dynamically and can be a multiline response
    pub(crate) fn dynamic(code: u16, head: String, tail: Vec<&'static str>) -> Self {
        Self {
            code,
            message: Message::Dynamic(head, tail),
            is_error: false,
            action: Action::Reply,
        }
    }

    // An empty response
    pub(crate) const fn empty() -> Self {
        Self {
            code: 0,
            message: Message::Empty,
            is_error: false,
            action: Action::NoReply,
        }
    }

    /// Write the response to the given writer
    pub fn write_to(&self, out: &mut dyn io::Write) -> io::Result<()> {
        match &self.message {
            Message::Dynamic(ref head, ref tail) => {
                if tail.is_empty() {
                    write!(out, "{} {}\r\n", self.code, head)?;
                } else {
                    write!(out, "{}-{}\r\n", self.code, head)?;
                    for i in 0..tail.len() {
                        if tail.len() > 1 && i < tail.len() - 1 {
                            write!(out, "{}-{}\r\n", self.code, tail[i])?;
                        } else {
                            write!(out, "{} {}\r\n", self.code, tail[i])?;
                        }
                    }
                }
            }
            Message::Fixed(s) => write!(out, "{} {}\r\n", self.code, s)?,
            Message::Custom(s) => write!(out, "{} {}\r\n", self.code, s)?,
            Message::Empty => (),
        };
        Ok(())
    }

    // Log the response
    pub(crate) fn log(&self) {
        match self.message {
            Message::Empty => (),
            _ => {
                let mut buf = Vec::new();
                let _ = self.write_to(&mut buf);
                trace!("< {}", String::from_utf8_lossy(&buf));
            }
        }
    }
}

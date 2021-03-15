use std::net::IpAddr;
use std::str;

use crate::fsm::StateMachine;
use crate::response::*;
use crate::{AuthMechanism, Handler};
use either::{Left, Right};

//------ Types -----------------------------------------------------------------

// Smtp commands sent by the client
#[derive(Clone)]
pub enum Cmd<'a> {
    Ehlo {
        domain: &'a str,
    },
    Helo {
        domain: &'a str,
    },
    Mail {
        reverse_path: &'a str,
        is8bit: bool,
    },
    Rcpt {
        forward_path: &'a str,
    },
    Data,
    Rset,
    Noop,
    StartTls,
    Quit,
    Vrfy,
    AuthPlain {
        authorization_id: String,
        authentication_id: String,
        password: String,
    },
    AuthPlainEmpty,
    // Dummy command containing client authentication
    AuthResponse {
        response: &'a [u8],
    },
    // Dummy command to signify end of data
    DataEnd,
    // Dummy command sent when STARTTLS was successful
    StartedTls,
}

pub(crate) struct Credentials {
    pub authorization_id: String,
    pub authentication_id: String,
    pub password: String,
}

/// A single smtp session connected to a single client
pub struct Session<H: Handler> {
    name: String,
    handler: H,
    fsm: StateMachine,
}

#[derive(Clone)]
/// Builds an smtp `Session`
///
/// # Examples
/// ```
/// # use mailin::{Session, SessionBuilder, Handler, AuthMechanism};
///
/// # use std::net::{IpAddr, Ipv4Addr};
/// # struct EmptyHandler{};
/// # impl Handler for EmptyHandler{};
/// # let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
/// # let handler = EmptyHandler{};
/// // Create a session builder that holds the configuration
/// let mut builder = SessionBuilder::new("server_name");
/// builder.enable_start_tls()
///        .enable_auth(AuthMechanism::Plain);
/// // Then when a client connects
/// let mut session = builder.build(addr, handler);
///
pub struct SessionBuilder {
    name: String,
    start_tls_extension: bool,
    auth_mechanisms: Vec<AuthMechanism>,
}

impl SessionBuilder {
    /// Create a new session for the given mailserver name
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            start_tls_extension: false,
            auth_mechanisms: Vec::with_capacity(4),
        }
    }

    /// Enable support for StartTls
    pub fn enable_start_tls(&mut self) -> &mut Self {
        self.start_tls_extension = true;
        self
    }

    /// Enable support for authentication
    pub fn enable_auth(&mut self, auth: AuthMechanism) -> &mut Self {
        self.auth_mechanisms.push(auth);
        self
    }

    /// Build a new session to handle a connection from the given ip address
    pub fn build<H: Handler>(&self, remote: IpAddr, handler: H) -> Session<H> {
        Session {
            name: self.name.clone(),
            handler,
            fsm: StateMachine::new(
                remote,
                self.auth_mechanisms.clone(),
                self.start_tls_extension,
            ),
        }
    }
}

impl<H: Handler> Session<H> {
    /// Get a greeting to send to the client
    pub fn greeting(&self) -> Response {
        Response::dynamic(220, format!("{} ESMTP", self.name), Vec::new())
    }

    /// STARTTLS active
    pub fn tls_active(&mut self) {
        self.command(Cmd::StartedTls);
    }

    /// Process a line sent by the client.
    ///
    /// Returns a response that should be written back to the client.
    ///
    /// # Examples
    /// ```
    /// use mailin::{Session, SessionBuilder, Handler, Action};
    ///
    /// # use std::net::{IpAddr, Ipv4Addr};
    /// # struct EmptyHandler{};
    /// # impl Handler for EmptyHandler{};
    /// # let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    /// # let handler = EmptyHandler{};
    /// # let mut session = SessionBuilder::new("name").build(addr, handler);
    /// let response = session.process(b"HELO example.com\r\n");
    ///
    /// // Check the response
    /// assert_eq!(response.is_error, false);
    /// assert_eq!(response.action, Action::Reply);
    ///
    /// // Write the response
    /// let mut msg = Vec::new();
    /// response.write_to(&mut msg);
    /// assert_eq!(&msg, b"250 OK\r\n");
    /// ```
    pub fn process(&mut self, line: &[u8]) -> Response {
        // TODO: process within fsm
        let response = match self.fsm.process_line(&mut self.handler, line) {
            Left(cmd) => self.command(cmd),
            Right(res) => res,
        };
        response.log();
        response
    }

    fn command(&mut self, cmd: Cmd) -> Response {
        self.fsm.command(&mut self.handler, cmd)
    }
}

//----- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsm::SmtpState;
    use std::net::Ipv4Addr;
    use ternop::ternary;

    struct EmptyHandler {}
    impl Handler for EmptyHandler {}
    struct DataHandler(Vec<u8>);
    impl Handler for DataHandler {
        fn data(&mut self, buf: &[u8]) -> std::io::Result<()> {
            self.0.extend(buf);
            Ok(())
        }
    }

    // Check that the state machine matches the given state pattern
    macro_rules! assert_state {
        ($val:expr, $n:pat ) => {{
            assert!(
                match $val {
                    $n => true,
                    _ => false,
                },
                "{:?} !~ {}",
                $val,
                stringify!($n)
            )
        }};
    }

    fn new_session() -> Session<EmptyHandler> {
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        SessionBuilder::new("some.name").build(addr, EmptyHandler {})
    }

    fn new_data_session() -> Session<DataHandler> {
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        SessionBuilder::new("some.name").build(addr, DataHandler(vec![]))
    }

    #[test]
    fn helo_ehlo() {
        let mut session = new_session();
        let res1 = session.process(b"helo a.domain\r\n");
        assert_eq!(res1.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
        let res2 = session.process(b"ehlo b.domain\r\n");
        assert_eq!(res2.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
    }

    #[test]
    fn mail_from() {
        let mut session = new_session();
        session.process(b"helo a.domain\r\n");
        let res = session.process(b"mail from:<ship@sea.com>\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Mail);
    }

    #[test]
    fn domain_badchars() {
        let mut session = new_session();
        let res = session.process(b"helo world\x40\xff\r\n");
        assert_eq!(res.code, 500);
        assert_state!(session.fsm.current_state(), SmtpState::Idle);
    }

    #[test]
    fn rcpt_to() {
        let mut session = new_session();
        session.process(b"helo a.domain\r\n");
        session.process(b"mail from:<ship@sea.com>\r\n");
        let res1 = session.process(b"rcpt to:<fish@sea.com>\r\n");
        assert_eq!(res1.code, 250);
        let res2 = session.process(b"rcpt to:<kraken@sea.com>\r\n");
        assert_eq!(res2.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Rcpt);
    }

    #[test]
    fn data() {
        let mut session = new_data_session();
        session.process(b"helo a.domain\r\n");
        session.process(b"mail from:<ship@sea.com>\r\n");
        session.process(b"rcpt to:<fish@sea.com>\r\n");
        let res1 = session.process(b"data\r\n");
        assert_eq!(res1.code, 354);
        let res2 = session.process(b"Hello World\r\n");
        assert_eq!(res2.action, Action::NoReply);
        let res3 = session.process(b".\r\n");
        assert_eq!(res3.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
        assert_eq!(&session.handler.0, b"Hello World\r\n");
    }

    #[test]
    fn dot_stuffed_data() {
        let mut session = new_data_session();
        session.process(b"helo a.domain\r\n");
        session.process(b"mail from:<ship@sea.com>\r\n");
        session.process(b"rcpt to:<fish@sea.com>\r\n");
        let res1 = session.process(b"data\r\n");
        assert_eq!(res1.code, 354);
        let res2 = session.process(b"Hello World\r\n");
        assert_eq!(res2.action, Action::NoReply);
        let res3 = session.process(b"..\r\n");
        assert_eq!(res3.action, Action::NoReply);
        let res3 = session.process(b".\r\n");
        assert_eq!(res3.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
        assert_eq!(&session.handler.0, b"Hello World\r\n.\r\n");
    }

    #[test]
    fn data_8bit() {
        let mut session = new_session();
        session.process(b"helo a.domain\r\n");
        session.process(b"mail from:<ship@sea.com> body=8bitmime\r\n");
        session.process(b"rcpt to:<fish@sea.com>\r\n");
        let res1 = session.process(b"data\r\n");
        assert_eq!(res1.code, 354);
        // Send illegal utf-8 but valid 8bit mime
        let res2 = session.process(b"Hello 8bit world \x40\x7f\r\n");
        assert_eq!(res2.action, Action::NoReply);
        let res3 = session.process(b".\r\n");
        assert_eq!(res3.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
    }

    #[test]
    fn rset_hello() {
        let mut session = new_session();
        session.process(b"helo some.domain\r\n");
        session.process(b"mail from:<ship@sea.com>\r\n");
        let res = session.process(b"rset\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
    }

    #[test]
    fn rset_idle() {
        let mut session = new_session();
        let res = session.process(b"rset\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::Idle);
    }

    #[test]
    fn quit() {
        let mut session = new_session();
        session.process(b"helo a.domain\r\n");
        session.process(b"mail from:<ship@sea.com>\r\n");
        let res = session.process(b"quit\r\n");
        assert_eq!(res.code, 221);
        assert_eq!(res.action, Action::Close);
        assert_state!(session.fsm.current_state(), SmtpState::Invalid);
    }

    #[test]
    fn vrfy() {
        let mut session = new_session();
        session.process(b"helo a.domain\r\n");
        let res1 = session.process(b"vrfy kraken\r\n");
        assert_eq!(res1.code, 252);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
        session.process(b"mail from:<ship@sea.com>\r\n");
        let res2 = session.process(b"vrfy boat\r\n");
        assert_eq!(res2.code, 503);
        assert_state!(session.fsm.current_state(), SmtpState::Mail);
    }

    struct AuthHandler {}
    impl Handler for AuthHandler {
        fn auth_plain(
            &mut self,
            authorization_id: &str,
            authentication_id: &str,
            password: &str,
        ) -> Response {
            ternary!(
                authorization_id == "test" && authentication_id == "test" && password == "1234",
                AUTH_OK,
                INVALID_CREDENTIALS
            )
        }
    }

    fn new_auth_session(with_start_tls: bool) -> Session<AuthHandler> {
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let mut builder = SessionBuilder::new("some.domain");
        builder.enable_auth(AuthMechanism::Plain);
        if with_start_tls {
            builder.enable_start_tls();
        }
        builder.build(addr, AuthHandler {})
    }

    fn start_tls(session: &mut Session<AuthHandler>) {
        let res = session.process(b"ehlo a.domain\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
        let res = session.process(b"starttls\r\n");
        assert_eq!(res.code, 220);
        session.tls_active();
    }

    #[test]
    fn noauth_denied() {
        let mut session = new_auth_session(true);
        session.process(b"ehlo a.domain\r\n");
        let res = session.process(b"mail from:<ship@sea.com>\r\n");
        assert_eq!(res.code, 503);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
    }

    #[test]
    fn auth_plain_param() {
        let mut session = new_auth_session(true);
        start_tls(&mut session);
        let mut res = session.process(b"ehlo a.domain\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
        res = session.process(b"auth plain dGVzdAB0ZXN0ADEyMzQ=\r\n");
        assert_eq!(res.code, 235);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
    }

    #[test]
    fn bad_auth_plain_param() {
        let mut session = new_auth_session(true);
        start_tls(&mut session);
        let mut res = session.process(b"ehlo a.domain\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
        res = session.process(b"auth plain eGVzdAB0ZXN0ADEyMzQ=\r\n");
        assert_eq!(res.code, 535);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
    }

    #[test]
    fn auth_plain_challenge() {
        let mut session = new_auth_session(true);
        start_tls(&mut session);
        let res = session.process(b"ehlo a.domain\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
        let res = session.process(b"auth plain\r\n");
        assert_eq!(res.code, 334);
        if res != EMPTY_AUTH_CHALLENGE {
            assert!(false, "Server did not send empty challenge");
        }
        assert_state!(session.fsm.current_state(), SmtpState::Auth);
        let res = session.process(b"dGVzdAB0ZXN0ADEyMzQ=\r\n");
        assert_eq!(res.code, 235);
        assert_state!(session.fsm.current_state(), SmtpState::Hello);
    }

    #[test]
    fn auth_without_tls() {
        let mut session = new_auth_session(true);
        let mut res = session.process(b"ehlo a.domain\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
        res = session.process(b"auth plain dGVzdAB0ZXN0ADEyMzQ=\r\n");
        assert_eq!(res.code, 503);
    }

    #[test]
    fn bad_auth_plain_challenge() {
        let mut session = new_auth_session(true);
        start_tls(&mut session);
        session.process(b"ehlo a.domain\r\n");
        session.process(b"auth plain\r\n");
        let res = session.process(b"eGVzdAB0ZXN0ADEyMzQ=\r\n");
        assert_eq!(res.code, 535);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
    }

    #[test]
    fn rset_with_auth() {
        let mut session = new_auth_session(true);
        start_tls(&mut session);
        let res = session.process(b"ehlo some.domain\r\n");
        assert_eq!(res.code, 250);
        let res = session.process(b"auth plain dGVzdAB0ZXN0ADEyMzQ=\r\n");
        assert_eq!(res.code, 235);
        let res = session.process(b"mail from:<ship@sea.com>\r\n");
        assert_eq!(res.code, 250);
        let res = session.process(b"rset\r\n");
        assert_eq!(res.code, 250);
        assert_state!(session.fsm.current_state(), SmtpState::HelloAuth);
    }
}

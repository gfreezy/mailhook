//! A library for building smtp servers.
//!
//! The library supplies a parser and SMTP state machine. The user of the library
//! supplies I/O code and a `Handler` implementation for controlling SMTP sessions.
//!
//! The code using the library, sends
//! lines received to the `Session.process_line()` method. The user also supplies a
//! `Handler` implementation that makes decisions on whether to accept or reject email
//! messages. After consulting the `Handler` the `Session.process_line()` function will
//! return a response that can be sent back to the email client.
//!
//! # Pseudo Code
//! ```rust,ignore
//! // Create a handler which will control the SMTP session
//! let hander = create_handler();
//!
//! // Create a SMTP session when a new client connects
//! let session = SessionBuilder::new("mailserver_name").build(client_ip, handler);
//!
//! // Read a line from the client
//! let line = read_line(tcp_connection);
//! // Send the line to the session
//! let res = session.process(line);
//!
//! // Act on the response
//! match res.action {
//!     Action::Reply => {
//!         write_response(tcp_connection, &res)?;
//!     }
//!     Action::Close => {
//!         write_response(tcp_connection, &res)?;
//!         close(tcp_connection);
//!     }
//!     Action::NoReply => (), // No response needed
//! }
//! ```

// Use write! for /r/n
#![cfg_attr(feature = "cargo-clippy", allow(clippy::write_with_newline))]
#![forbid(unsafe_code)]
#![forbid(missing_docs)]

use std::io;
use std::net::IpAddr;
mod fsm;
mod parser;
/// Response contains a selection of SMTP responses for use in handlers.
pub mod response;
mod smtp;

pub use crate::{
    response::{Action, Response},
    smtp::{Session, SessionBuilder},
};

/// A `Handler` makes decisions about incoming mail commands.
///
/// A Handler implementation must be provided by code using the mailin library.
///
/// All methods have a default implementation that does nothing. A separate handler instance
/// should be created for each connection.
///
/// # Examples
/// ```
/// # use mailin::{Handler, Response};
/// # use mailin::response::{OK, BAD_HELLO, NO_MAILBOX};
///
/// # use std::net::IpAddr;
/// # struct MyHandler{};
/// impl Handler for MyHandler {
///     fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
///        if domain == "this.is.spam.com" {
///            OK
///        } else {
///            BAD_HELLO
///        }
///     }
///
///     fn rcpt(&mut self, to: &str) -> Response {
///        if to == "alienscience" {
///            OK
///        } else {
///            NO_MAILBOX
///        }
///     }
/// }
/// ```
pub trait Handler {
    /// Called when a client sends a ehlo or helo message
    fn helo(&mut self, _ip: IpAddr, _domain: &str) -> Response {
        response::OK
    }

    /// Called when a mail message is started
    fn mail(&mut self, _ip: IpAddr, _domain: &str, _from: &str) -> Response {
        response::OK
    }

    /// Called when a mail recipient is set
    fn rcpt(&mut self, _to: &str) -> Response {
        response::OK
    }

    /// Called when a data command is received
    fn data_start(
        &mut self,
        _domain: &str,
        _from: &str,
        _is8bit: bool,
        _to: &[String],
    ) -> Response {
        response::OK
    }

    /// Called when a data buffer is received
    fn data(&mut self, _buf: &[u8]) -> io::Result<()> {
        Ok(())
    }

    /// Called at the end of receiving data
    fn data_end(&mut self) -> Response {
        response::OK
    }

    /// Called when a plain authentication request is received
    fn auth_plain(
        &mut self,
        _authorization_id: &str,
        _authentication_id: &str,
        _password: &str,
    ) -> Response {
        response::INVALID_CREDENTIALS
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Supported authentication mechanisms
pub enum AuthMechanism {
    /// Plain user/password over TLS
    Plain,
}

impl AuthMechanism {
    // Show the AuthMechanism text as an SMTP extension
    fn extension(&self) -> &'static str {
        match self {
            AuthMechanism::Plain => "AUTH PLAIN",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::*;
    use std::io::{Cursor, Write};
    use std::net::Ipv4Addr;

    struct TestHandler {
        ip: IpAddr,
        domain: String,
        from: String,
        to: Vec<String>,
        is8bit: bool,
        expected_data: Vec<u8>,
        cursor: Cursor<Vec<u8>>,
        // Booleans set when callbacks are successful
        helo_called: bool,
        mail_called: bool,
        rcpt_called: bool,
        data_start_called: bool,
        data_called: bool,
        data_end_called: bool,
    }

    impl<'a> Handler for &'a mut TestHandler {
        fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
            assert_eq!(self.ip, ip);
            assert_eq!(self.domain, domain);
            self.helo_called = true;
            OK
        }

        // Called when a mail message is started
        fn mail(&mut self, ip: IpAddr, domain: &str, from: &str) -> Response {
            assert_eq!(self.ip, ip);
            assert_eq!(self.domain, domain);
            assert_eq!(self.from, from);
            self.mail_called = true;
            OK
        }

        // Called when a mail recipient is set
        fn rcpt(&mut self, to: &str) -> Response {
            let valid_to = self.to.iter().any(|elem| elem == to);
            assert!(valid_to, "Invalid to address");
            self.rcpt_called = true;
            OK
        }

        // Called to start writing an email message to a writer
        fn data_start(
            &mut self,
            domain: &str,
            from: &str,
            is8bit: bool,
            to: &[String],
        ) -> Response {
            assert_eq!(self.domain, domain);
            assert_eq!(self.from, from);
            assert_eq!(self.to, to);
            assert_eq!(self.is8bit, is8bit);
            self.data_start_called = true;
            OK
        }

        fn data(&mut self, buf: &[u8]) -> io::Result<()> {
            self.data_called = true;
            self.cursor.write(buf).map(|_| ())
        }

        fn data_end(&mut self) -> Response {
            self.data_end_called = true;
            let actual_data = self.cursor.get_ref();
            assert_eq!(actual_data, &self.expected_data);
            OK
        }
    }

    #[test]
    fn callbacks() {
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let domain = "some.domain";
        let from = "ship@sea.com";
        let to = vec!["fish@sea.com".to_owned(), "seaweed@sea.com".to_owned()];
        let data = vec![
            b"Hello 8bit world \x40\x7f\r\n" as &[u8],
            b"Hello again\r\n" as &[u8],
        ];
        let mut expected_data = Vec::with_capacity(2);
        for line in data.clone() {
            expected_data.extend(line);
        }
        let mut handler = TestHandler {
            ip: ip.clone(),
            domain: domain.to_owned(),
            from: from.to_owned(),
            to: to.clone(),
            is8bit: true,
            expected_data,
            cursor: Cursor::new(Vec::with_capacity(80)),
            helo_called: false,
            mail_called: false,
            rcpt_called: false,
            data_called: false,
            data_start_called: false,
            data_end_called: false,
        };
        let mut session =
            smtp::SessionBuilder::new("server.domain").build(ip.clone(), &mut handler);
        let helo = format!("helo {}\r\n", domain).into_bytes();
        session.process(&helo);
        let mail = format!("mail from:<{}> body=8bitmime\r\n", from).into_bytes();
        session.process(&mail);
        let rcpt0 = format!("rcpt to:<{}>\r\n", &to[0]).into_bytes();
        let rcpt1 = format!("rcpt to:<{}>\r\n", &to[1]).into_bytes();
        session.process(&rcpt0);
        session.process(&rcpt1);
        session.process(b"data\r\n");
        for line in data {
            session.process(line);
        }
        session.process(b".\r\n");
        assert_eq!(handler.helo_called, true);
        assert_eq!(handler.mail_called, true);
        assert_eq!(handler.rcpt_called, true);
        assert_eq!(handler.data_called, true);
    }
}

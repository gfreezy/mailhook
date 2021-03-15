use crate::parser::{decode_sasl_plain, parse, parse_auth_response};
use crate::response::*;

use crate::smtp::Cmd;
use crate::{AuthMechanism, Handler, Response};
use either::*;
use log::{error, trace};
use std::borrow::BorrowMut;
use std::net::IpAddr;
use ternop::ternary;

#[cfg(test)]
#[derive(Debug)]
pub(crate) enum SmtpState {
    Invalid,
    Idle,
    Hello,
    HelloAuth,
    Auth,
    Mail,
    Rcpt,
    Data,
}

#[derive(PartialEq)]
enum TlsState {
    Unavailable,
    Inactive,
    Active,
}

enum AuthState {
    Unavailable,
    RequiresAuth,
    Authenticated,
}

trait State {
    #[cfg(test)]
    fn id(&self) -> SmtpState;

    // Handle an incoming command and return the next state
    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>);

    // Most state will convert an input line into a command.
    // Some states, e.g Data, need to process input lines differently and will
    // override this method.
    fn process_line<'a>(
        self: &mut Self,
        _handler: &mut dyn Handler,
        line: &'a [u8],
    ) -> Either<Cmd<'a>, Response> {
        trace!("> {}", String::from_utf8_lossy(line));
        parse(line).map(Left).unwrap_or_else(Right)
    }
}

//------------------------------------------------------------------------------

// Return the next state depending on the response
fn next_state<F>(
    current: Box<dyn State>,
    res: Response,
    next_state: F,
) -> (Response, Option<Box<dyn State>>)
where
    F: FnOnce() -> Box<dyn State>,
{
    if res.action == Action::Close {
        (res, None)
    } else if res.is_error {
        (res, Some(current))
    } else {
        (res, Some(next_state()))
    }
}

// Convert the current state to the next state depending on the response
fn transform_state<S, F>(
    current: Box<S>,
    res: Response,
    next_state: F,
) -> (Response, Option<Box<dyn State>>)
where
    S: State + 'static,
    F: FnOnce(S) -> Box<dyn State>,
{
    if res.action == Action::Close {
        (res, None)
    } else if res.is_error {
        (res, Some(current))
    } else {
        (res, Some(next_state(*current)))
    }
}

fn default_handler(
    current: Box<dyn State>,
    fsm: &StateMachine,
    handler: &mut dyn Handler,
    cmd: &Cmd,
) -> (Response, Option<Box<dyn State>>) {
    match *cmd {
        Cmd::Quit => (GOODBYE.clone(), None),
        Cmd::Helo { domain } => handle_helo(current, fsm, handler, domain),
        Cmd::Ehlo { domain } => handle_ehlo(current, fsm, handler, domain),
        _ => unhandled(current),
    }
}

fn unhandled(current: Box<dyn State>) -> (Response, Option<Box<dyn State>>) {
    (BAD_SEQUENCE_COMMANDS.clone(), Some(current))
}

fn handle_rset(fsm: &StateMachine, domain: &str) -> (Response, Option<Box<dyn State>>) {
    match fsm.auth_state {
        AuthState::Unavailable => (
            OK.clone(),
            Some(Box::new(Hello {
                domain: domain.to_string(),
            })),
        ),
        _ => (
            OK.clone(),
            Some(Box::new(HelloAuth {
                domain: domain.to_string(),
            })),
        ),
    }
}

fn handle_helo(
    current: Box<dyn State>,
    fsm: &StateMachine,
    handler: &mut dyn Handler,
    domain: &str,
) -> (Response, Option<Box<dyn State>>) {
    match fsm.auth_state {
        AuthState::Unavailable => {
            let res = Response::from(handler.helo(fsm.ip, domain));
            next_state(current, res, || {
                Box::new(Hello {
                    domain: domain.to_owned(),
                })
            })
        }
        _ => {
            // If authentication is required the client should be using EHLO
            (BAD_HELLO.clone(), Some(current))
        }
    }
}

fn handle_ehlo(
    current: Box<dyn State>,
    fsm: &StateMachine,
    handler: &mut dyn Handler,
    domain: &str,
) -> (Response, Option<Box<dyn State>>) {
    let mut res = handler.helo(fsm.ip, domain);
    if res.code == 250 {
        res = fsm.ehlo_response();
    }
    match fsm.auth_state {
        AuthState::Unavailable => next_state(current, res, || {
            Box::new(Hello {
                domain: domain.to_owned(),
            })
        }),
        AuthState::RequiresAuth | AuthState::Authenticated => next_state(current, res, || {
            Box::new(HelloAuth {
                domain: domain.to_owned(),
            })
        }),
    }
}

fn authenticate(
    fsm: &mut StateMachine,
    handler: &mut dyn Handler,
    authorization_id: &str,
    authentication_id: &str,
    password: &str,
) -> Response {
    let auth_res = handler.auth_plain(authorization_id, authentication_id, password);
    fsm.auth_state = ternary!(
        auth_res.code == 235,
        AuthState::Authenticated,
        AuthState::RequiresAuth
    );
    Response::from(auth_res)
}

//------------------------------------------------------------------------------

struct Idle {}

impl State for Idle {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::Idle
    }

    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::StartedTls => {
                fsm.tls = TlsState::Active;
                (EMPTY_RESPONSE.clone(), Some(self))
            }
            Cmd::Rset => (OK.clone(), Some(self)),
            _ => default_handler(self, fsm, handler, &cmd),
        }
    }
}

//------------------------------------------------------------------------------

struct Hello {
    domain: String,
}

impl State for Hello {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::Hello
    }

    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::Mail {
                reverse_path,
                is8bit,
            } => {
                let res = Response::from(handler.mail(fsm.ip, &self.domain, reverse_path));
                transform_state(self, res, |s| {
                    Box::new(Mail {
                        domain: s.domain,
                        reverse_path: reverse_path.to_owned(),
                        is8bit,
                    })
                })
            }
            Cmd::StartTls if fsm.tls == TlsState::Inactive => {
                (START_TLS.clone(), Some(Box::new(Idle {})))
            }
            Cmd::Vrfy => (VERIFY_RESPONSE.clone(), Some(self)),
            Cmd::Rset => handle_rset(fsm, &self.domain),
            _ => default_handler(self, fsm, handler, &cmd),
        }
    }
}

//------------------------------------------------------------------------------

struct HelloAuth {
    domain: String,
}

impl State for HelloAuth {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::HelloAuth
    }

    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::StartTls => (START_TLS.clone(), Some(Box::new(Idle {}))),
            Cmd::AuthPlain {
                ref authorization_id,
                ref authentication_id,
                ref password,
            } if fsm.allow_auth_plain() => {
                let res = authenticate(fsm, handler, authorization_id, authentication_id, password);
                transform_state(self, res, |s| Box::new(Hello { domain: s.domain }))
            }
            Cmd::AuthPlainEmpty if fsm.allow_auth_plain() => {
                let domain = self.domain.clone();
                (
                    EMPTY_AUTH_CHALLENGE,
                    Some(Box::new(Auth {
                        domain,
                        mechanism: AuthMechanism::Plain,
                    })),
                )
            }
            Cmd::Rset => handle_rset(fsm, &self.domain),
            _ => default_handler(self, fsm, handler, &cmd),
        }
    }
}

//------------------------------------------------------------------------------

struct Auth {
    domain: String,
    mechanism: AuthMechanism,
}

impl State for Auth {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::Auth
    }

    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::AuthResponse { response } => {
                let res = match self.mechanism {
                    AuthMechanism::Plain => {
                        let creds = decode_sasl_plain(response);
                        authenticate(
                            fsm,
                            handler,
                            &creds.authorization_id,
                            &creds.authentication_id,
                            &creds.password,
                        )
                    }
                };
                let domain = self.domain.clone();
                if res.is_error {
                    (res, Some(Box::new(HelloAuth { domain })))
                } else {
                    (res, Some(Box::new(Hello { domain })))
                }
            }
            _ => unhandled(self),
        }
    }

    fn process_line<'a>(
        self: &mut Self,
        _handler: &mut dyn Handler,
        line: &'a [u8],
    ) -> Either<Cmd<'a>, Response> {
        trace!("> {}", String::from_utf8_lossy(line));
        parse_auth_response(line)
            .map(|r| Left(Cmd::AuthResponse { response: r }))
            .unwrap_or_else(Right)
    }
}

//------------------------------------------------------------------------------

struct Mail {
    domain: String,
    reverse_path: String,
    is8bit: bool,
}

impl State for Mail {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::Mail
    }

    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::Rcpt { forward_path } => {
                let res = Response::from(handler.rcpt(forward_path));
                transform_state(self, res, |s| {
                    let fp = vec![forward_path.to_owned()];
                    Box::new(Rcpt {
                        domain: s.domain,
                        reverse_path: s.reverse_path,
                        is8bit: s.is8bit,
                        forward_path: fp,
                    })
                })
            }
            Cmd::Rset => handle_rset(fsm, &self.domain),
            _ => default_handler(self, fsm, handler, &cmd),
        }
    }
}

//------------------------------------------------------------------------------

struct Rcpt {
    domain: String,
    reverse_path: String,
    is8bit: bool,
    forward_path: Vec<String>,
}

impl State for Rcpt {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::Rcpt
    }

    fn handle(
        self: Box<Self>,
        fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::Data => {
                let res = handler.data_start(
                    &self.domain,
                    &self.reverse_path,
                    self.is8bit,
                    &self.forward_path,
                );
                let res = ternary!(res.is_error, res, START_DATA);
                transform_state(self, res, |s| Box::new(Data { domain: s.domain }))
            }
            Cmd::Rcpt { forward_path } => {
                let res = Response::from(handler.rcpt(forward_path));
                transform_state(self, res, |s| {
                    let mut fp = s.forward_path;
                    fp.push(forward_path.to_owned());
                    Box::new(Rcpt {
                        domain: s.domain,
                        reverse_path: s.reverse_path,
                        is8bit: s.is8bit,
                        forward_path: fp,
                    })
                })
            }
            Cmd::Rset => handle_rset(fsm, &self.domain),
            _ => default_handler(self, fsm, handler, &cmd),
        }
    }
}

//------------------------------------------------------------------------------

struct Data {
    domain: String,
}

impl State for Data {
    #[cfg(test)]
    fn id(&self) -> SmtpState {
        SmtpState::Data
    }

    fn handle(
        self: Box<Self>,
        _fsm: &mut StateMachine,
        handler: &mut dyn Handler,
        cmd: Cmd,
    ) -> (Response, Option<Box<dyn State>>) {
        match cmd {
            Cmd::DataEnd => {
                let res = Response::from(handler.data_end());
                transform_state(self, res, |s| {
                    Box::new(Hello {
                        domain: s.domain.clone(),
                    })
                })
            }
            _ => unhandled(self),
        }
    }

    fn process_line<'a>(
        self: &mut Self,
        handler: &mut dyn Handler,
        mut line: &'a [u8],
    ) -> Either<Cmd<'a>, Response> {
        if line == b".\r\n" {
            trace!("> _data_");
            Left(Cmd::DataEnd)
        } else {
            if line.starts_with(b".") {
                line = &line[1..];
            }
            match handler.data(line) {
                Ok(_) => Right(EMPTY_RESPONSE.clone()),
                Err(e) => {
                    error!("Error saving message: {}", e);
                    Right(TRANSACTION_FAILED.clone())
                }
            }
        }
    }
}
//------------------------------------------------------------------------------

pub(crate) struct StateMachine {
    ip: IpAddr,
    auth_mechanisms: Vec<AuthMechanism>,
    auth_state: AuthState,
    tls: TlsState,
    smtp: Option<Box<dyn State>>,
    auth_plain: bool,
}

impl StateMachine {
    pub fn new(ip: IpAddr, auth_mechanisms: Vec<AuthMechanism>, allow_start_tls: bool) -> Self {
        let auth_state = ternary!(
            auth_mechanisms.is_empty(),
            AuthState::Unavailable,
            AuthState::RequiresAuth
        );
        let tls = ternary!(allow_start_tls, TlsState::Inactive, TlsState::Unavailable);
        let auth_plain = auth_mechanisms.contains(&AuthMechanism::Plain);
        Self {
            ip,
            auth_mechanisms,
            auth_state,
            tls,
            smtp: Some(Box::new(Idle {})),
            auth_plain,
        }
    }

    // Respond and change state with the given command
    pub fn command(&mut self, handler: &mut dyn Handler, cmd: Cmd) -> Response {
        let (response, next_state) = match self.smtp.take() {
            Some(last_state) => last_state.handle(self, handler, cmd),
            None => (INVALID_STATE.clone(), None),
        };
        self.smtp = next_state;
        response
    }

    pub fn process_line<'a>(
        &mut self,
        handler: &mut dyn Handler,
        line: &'a [u8],
    ) -> Either<Cmd<'a>, Response> {
        match self.smtp {
            Some(ref mut s) => {
                let s: &mut dyn State = s.borrow_mut();
                s.process_line(handler, line)
            }
            None => Right(INVALID_STATE.clone()),
        }
    }

    #[cfg(test)]
    pub fn current_state(&self) -> SmtpState {
        let id = self.smtp.as_ref().map(|s| s.id());
        id.unwrap_or(SmtpState::Invalid)
    }

    fn ehlo_response(&self) -> Response {
        let mut extensions = vec!["8BITMIME"];
        if self.tls == TlsState::Inactive {
            extensions.push("STARTTLS");
        } else {
            for auth in &self.auth_mechanisms {
                extensions.push(auth.extension());
            }
        }
        Response::dynamic(250, "server offers extensions:".to_string(), extensions)
    }

    fn allow_auth_plain(&self) -> bool {
        self.auth_plain && self.tls == TlsState::Active
    }
}

use base64;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, tag_no_case, take_while1};
use nom::character::is_alphanumeric;
use nom::combinator::{map, map_res, value};
use nom::sequence::{pair, preceded, separated_pair, terminated};
use nom::IResult;

use crate::response::*;
use crate::smtp::{Cmd, Credentials};
use nom::character::complete::space0;
use std::str;

//----- Parser -----------------------------------------------------------------

// Parse a line from the client
pub fn parse(line: &[u8]) -> Result<Cmd, Response> {
    command(line).map(|r| r.1).map_err(|e| match e {
        nom::Err::Incomplete(_) => MISSING_PARAMETER.clone(),
        nom::Err::Error(_) => SYNTAX_ERROR.clone(),
        nom::Err::Failure(_) => SYNTAX_ERROR.clone(),
    })
}

// Parse an authentication response from the client
pub fn parse_auth_response(line: &[u8]) -> Result<&[u8], Response> {
    auth_response(line)
        .map(|r| r.1)
        .map_err(|_| SYNTAX_ERROR.clone())
}

fn command(buf: &[u8]) -> IResult<&[u8], Cmd> {
    terminated(
        alt((
            helo, ehlo, mail, rcpt, data, rset, quit, vrfy, noop, starttls, auth,
        )),
        tag(b"\r\n"),
    )(buf)
}

fn hello_domain(buf: &[u8]) -> IResult<&[u8], &str> {
    map_res(is_not(b" \t\r\n" as &[u8]), str::from_utf8)(buf)
}

fn helo(buf: &[u8]) -> IResult<&[u8], Cmd> {
    let parse_domain = preceded(cmd(b"helo"), hello_domain);
    map(parse_domain, |domain| Cmd::Helo { domain })(buf)
}

fn ehlo(buf: &[u8]) -> IResult<&[u8], Cmd> {
    let parse_domain = preceded(cmd(b"ehlo"), hello_domain);
    map(parse_domain, |domain| Cmd::Ehlo { domain })(buf)
}

fn mail_path(buf: &[u8]) -> IResult<&[u8], &str> {
    map_res(is_not(b" <>\t\r\n" as &[u8]), str::from_utf8)(buf)
}

fn take_all(buf: &[u8]) -> IResult<&[u8], &str> {
    map_res(is_not(b"\r\n" as &[u8]), str::from_utf8)(buf)
}

fn body_eq_8bit(buf: &[u8]) -> IResult<&[u8], bool> {
    let preamble = pair(space, tag_no_case(b"body="));
    let is8bit = alt((
        value(true, tag_no_case(b"8bitmime")),
        value(false, tag_no_case(b"7bit")),
    ));
    preceded(preamble, is8bit)(buf)
}

fn is8bitmime(buf: &[u8]) -> IResult<&[u8], bool> {
    body_eq_8bit(buf).or_else(|_| Ok((buf, false)))
}

fn mail(buf: &[u8]) -> IResult<&[u8], Cmd> {
    let from = separated_pair(tag_no_case(b"from:"), space0, tag_no_case(b"<"));
    let preamble = pair(cmd(b"mail"), from);
    let mail_path_parser = preceded(preamble, mail_path);
    let parser = separated_pair(mail_path_parser, tag(b">"), is8bitmime);
    map(parser, |r| Cmd::Mail {
        reverse_path: r.0,
        is8bit: r.1,
    })(buf)
}

fn rcpt(buf: &[u8]) -> IResult<&[u8], Cmd> {
    let to = separated_pair(tag_no_case(b"to:"), space0, tag_no_case(b"<"));
    let preamble = pair(cmd(b"rcpt"), to);
    let mail_path_parser = preceded(preamble, mail_path);
    let parser = terminated(mail_path_parser, tag(b">"));
    map(parser, |path| Cmd::Rcpt { forward_path: path })(buf)
}

fn data(buf: &[u8]) -> IResult<&[u8], Cmd> {
    value(Cmd::Data, tag_no_case(b"data"))(buf)
}

fn rset(buf: &[u8]) -> IResult<&[u8], Cmd> {
    value(Cmd::Rset, tag_no_case(b"rset"))(buf)
}

fn quit(buf: &[u8]) -> IResult<&[u8], Cmd> {
    value(Cmd::Quit, tag_no_case(b"quit"))(buf)
}

fn vrfy(buf: &[u8]) -> IResult<&[u8], Cmd> {
    let preamble = preceded(cmd(b"vrfy"), take_all);
    value(Cmd::Vrfy, preamble)(buf)
}

fn noop(buf: &[u8]) -> IResult<&[u8], Cmd> {
    value(Cmd::Noop, tag_no_case(b"noop"))(buf)
}

fn starttls(buf: &[u8]) -> IResult<&[u8], Cmd> {
    value(Cmd::StartTls, tag_no_case(b"starttls"))(buf)
}

fn is_base64(chr: u8) -> bool {
    is_alphanumeric(chr) || (chr == b'+') || (chr == b'/' || chr == b'=')
}

fn auth_initial(buf: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(space, take_while1(is_base64))(buf)
}

fn auth_response(buf: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_while1(is_base64), tag("\r\n"))(buf)
}

fn empty(buf: &[u8]) -> IResult<&[u8], &[u8]> {
    Ok((buf, b"" as &[u8]))
}

fn auth_plain(buf: &[u8]) -> IResult<&[u8], Cmd> {
    let parser = preceded(tag_no_case(b"plain"), alt((auth_initial, empty)));
    map(parser, |initial| sasl_plain_cmd(initial))(buf)
}

fn auth(buf: &[u8]) -> IResult<&[u8], Cmd> {
    preceded(cmd(b"auth"), auth_plain)(buf)
}

//---- Helper functions ---------------------------------------------------------

// Return a parser to match the given command
fn cmd(cmd_tag: &[u8]) -> impl Fn(&[u8]) -> IResult<&[u8], (&[u8], &[u8])> + '_ {
    move |buf: &[u8]| pair(tag_no_case(cmd_tag), space)(buf)
}

// Match one or more spaces
fn space(buf: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(|b| b == b' ')(buf)
}

fn sasl_plain_cmd(param: &[u8]) -> Cmd {
    if param.is_empty() {
        Cmd::AuthPlainEmpty
    } else {
        let creds = decode_sasl_plain(param);
        Cmd::AuthPlain {
            authorization_id: creds.authorization_id,
            authentication_id: creds.authentication_id,
            password: creds.password,
        }
    }
}

// Decodes the base64 encoded plain authentication parameter
pub(crate) fn decode_sasl_plain(param: &[u8]) -> Credentials {
    let decoded = base64::decode(param);
    if let Ok(bytes) = decoded {
        let mut fields = bytes.split(|b| b == &0u8);
        let authorization_id = next_string(&mut fields);
        let authentication_id = next_string(&mut fields);
        let password = next_string(&mut fields);
        Credentials {
            authorization_id,
            authentication_id,
            password,
        }
    } else {
        Credentials {
            authorization_id: String::default(),
            authentication_id: String::default(),
            password: String::default(),
        }
    }
}

fn next_string(it: &mut dyn Iterator<Item = &[u8]>) -> String {
    it.next()
        .map(|s| str::from_utf8(s).unwrap_or_default())
        .unwrap_or_default()
        .to_owned()
}

//---- Tests --------------------------------------------------------------------

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn auth_initial() {
        let res = parse(b"auth plain dGVzdAB0ZXN0ADEyMzQ=\r\n");
        match res {
            Ok(Cmd::AuthPlain {
                authorization_id,
                authentication_id,
                password,
            }) => {
                assert_eq!(authorization_id, "test");
                assert_eq!(authentication_id, "test");
                assert_eq!(password, "1234");
            }
            _ => assert!(false, "Auth plain with initial response incorrectly parsed"),
        };
    }

    #[test]
    fn auth_empty() {
        let res = parse(b"auth plain\r\n");
        match res {
            Ok(Cmd::AuthPlainEmpty) => {}
            _ => assert!(
                false,
                "Auth plain without initial response incorrectly parsed"
            ),
        };
    }
}

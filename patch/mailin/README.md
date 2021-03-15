# Mailin

This is a library for writing SMTP servers in Rust. The library handles parsing, the SMTP state machine and building responses.

Programs using the Mailin library are responsible for all IO including opening sockets and storing messages. Mailin makes the lifecycle of an SMTP session available by calling methods on an object that implements the `Handler` trait.



//! `prim lsp` — a Language Server Protocol server exposing prim's formatter as
//! a whole-document formatting provider, so editors format prim-owned files on
//! save through their native LSP client instead of a bespoke shell hook.
//!
//! The server advertises **Full** document sync (never splicing incremental
//! deltas) and `documentFormattingProvider`; it runs the same engine as
//! `prim fmt`, so an editor save and a CLI run produce identical bytes.

mod protocol;
mod server;
mod transport;

use std::io::{BufReader, Write};

use self::server::{Reaction, Server};

/// Run the LSP server over stdin/stdout until the client sends `exit`,
/// returning the process exit code (`0` after a clean `shutdown`/`exit`
/// handshake, `1` if `exit` arrives without `shutdown`, `2` on a transport
/// error). All diagnostics go to stderr so stdout stays a clean LSP channel.
pub fn run() -> i32 {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut server = Server::new();

    loop {
        let message = match transport::read_message(&mut reader) {
            Ok(Some(message)) => message,
            Ok(None) => return exit_code_for_stream_end(&server),
            Err(err) => {
                eprintln!("prim lsp: transport error: {err}");
                return 2;
            }
        };

        match server.handle(&message) {
            Reaction::Reply(reply) => {
                if let Err(err) = transport::write_message(&mut writer, &reply) {
                    eprintln!("prim lsp: failed to write response: {err}");
                    return 2;
                }
            }
            Reaction::None => {}
            Reaction::Exit(code) => {
                let _ = writer.flush();
                return code;
            }
        }
    }
}

/// A client that drops the connection without an `exit` notification is a
/// clean stream end; mirror LSP's `exit` contract — `0` if `shutdown` was
/// requested first, otherwise `1`.
fn exit_code_for_stream_end(server: &Server) -> i32 {
    if server.shutdown_requested() { 0 } else { 1 }
}

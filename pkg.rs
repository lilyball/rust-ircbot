#[crate_id="github.com/kballard/rust-ircbot#ircbot:0.1"];
#[crate_type="bin"];

extern mod lua;
extern mod irc;

use irc::conn::{Conn, Line, Event, IRCCmd, IRCCode, IRCAction};

fn main() {
    let mut opts = irc::conn::Options::new("chat.freenode.net", irc::conn::DefaultPort);

    opts.nick = "rustircbot";
    match irc::conn::connect(opts, handler) {
        Ok(()) => println!("Exiting..."),
        Err(err) => println!("Connection error: {}", err)
    }
}

fn handler(conn: &mut Conn, event: Event) {
    match event {
        irc::conn::Connected => println!("Connected"),
        irc::conn::Disconnected => println!("Disconnected"),
        irc::conn::LineReceived(line) => {
            let Line{command, args, prefix} = line;
            match command {
                IRCCode(1) => {
                    println!("Logged in");
                    conn.join(bytes!("##rustircbot"));
                }
                IRCCmd(~"PRIVMSG") if prefix.is_some() && !args.is_empty() => {

                }
                IRCAction(_dst) => {
                    if prefix.is_none() || args.is_empty() { return; }

                }
                _ => ()
            }
        }
    }
}

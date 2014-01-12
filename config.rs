use std::{io,os};
use extra::getopts::groups::{getopts, optflag, optopt, usage, OptGroup};
use toml;

static CONFIG_EXAMPLE: &'static str = include_str!("config.example.toml");

pub struct Config {
    plugin_dir: ~str,
    reconnect_time: Option<uint>,
    reconnect_backoff: bool,
    servers: ~[Server]
}

pub struct Server {
    name: ~str,
    host: ~str,
    port: uint,
    use_ssl: bool,
    nick: ~str,
    user: ~str,
    real: ~str
}

pub fn print_usage(opts: &[OptGroup]) {
    writeln!(&mut io::stderr(), "{}", usage(format!("Usage: {} [OPTIONS]", os::args()[0]), opts));
}

pub fn parse_args() -> Result<Config,()> {
    let args = os::args();

    let opts = [
        optflag("h", "help", "Displays this help"),
        optopt("c", "config", "Path for the config file, defaults to ~/.rustirc", "file")
    ];

    let matches = match getopts(args.tail(), opts) {
        Ok(m) => m,
        Err(f) => {
            writeln!(&mut io::stderr(), "error: {}\n", f.to_err_msg());
            print_usage(opts);
            return Err(());
        }
    };

    if matches.opt_present("h") {
        print_usage(opts);
        return Err(());
    }

    let path = match matches.opt_str("c") {
        None => {
            let p = os::homedir().expect("can't find user's home dir").join(".rustirc");
            if !p.exists() {
                println!("No config file ~/.rustirc exists, writing default file");
                let mut f = io::File::create(&p).unwrap(); // None should have raised
                f.write(CONFIG_EXAMPLE.as_bytes());
                return Err(());
            }
            p
        }
        Some(p) => {
            let p = Path::new(p);
            if !p.exists() {
                writeln!(&mut io::stderr(), "error: config file {} does not exist", p.display());
                return Err(());
            }
            p
        }
    };

    let root = toml::parse_from_path(&path);

    let plugin_dir = match root.lookup("plugin.dir").and_then(|v| v.get_str()) {
        None => {
            writeln!(&mut io::stderr(), "error: required string config value plugin.dir missing");
            return Err(());
        }
        Some(s) => s.clone()
    };
    let reconnect = match root.lookup("general.reconnect").and_then(|v| v.get_int()) {
        None => Some(5),
        Some(x) if x < 0 => None,
        Some(x) => Some(x.to_uint().unwrap())
    };
    let backoff = root.lookup("general.reconnect_backoff").and_then(|v| v.get_bool())
                      .unwrap_or(true);
    let default_nick = root.lookup("general.defaults.nick").and_then(|v| v.get_str())
                           .map(|s| s.clone()).unwrap_or_else(|| ~"rustbot");
    let default_user = root.lookup("general.defaults.user").and_then(|v| v.get_str())
                           .map(|s| s.clone()).unwrap_or_else(|| ~"rustbot");
    let default_real = root.lookup("general.defaults.real").and_then(|v| v.get_str())
                           .map(|s| s.clone()).unwrap_or_else(|| ~"Rust IRC Bot");

    let mut servers = ~[];
    let server_list = match root.lookup_key("servers").and_then(|v| v.get_table_array()) {
        None => {
            writeln!(&mut io::stderr(), "error: at least one server must be defined");
            return Err(());
        }
        Some(ary) => ary.as_slice()
    };
    for elem in server_list.iter() {
        let name = match elem.lookup_key("name").and_then(|v| v.get_str()) {
            None => {
                writeln!(&mut io::stderr(), "error: server entry missing required 'name' key");
                return Err(());
            }
            Some(s) => s.clone()
        };
        let server = match elem.lookup_key("server").and_then(|v| v.get_str()) {
            None => {
                writeln!(&mut io::stderr(), "error: server entry missing required 'server' key");
                return Err(());
            }
            Some(s) => s.clone()
        };
        let use_ssl = elem.lookup_key("use_ssl").and_then(|v| v.get_bool()).unwrap_or(false);
        if use_ssl {
            writeln!(&mut io::stderr(), "error: use_ssl is not currently implemented");
            return Err(());
        }
        let default_port = if use_ssl { 6697 } else { 6667 };
        let port = match elem.lookup_key("port").and_then(|v| v.get_int()).unwrap_or(default_port)
                             .to_uint() {
            None => {
                writeln!(&mut io::stderr(), "error: port is out of range");
                return Err(());
            }
            Some(p) => p
        };
        let nick = elem.lookup_key("nick").and_then(|v| v.get_str()).map(|s| s.clone())
                       .unwrap_or_else(|| default_nick.clone());
        let user = elem.lookup_key("user").and_then(|v| v.get_str()).map(|s| s.clone())
                       .unwrap_or_else(|| default_user.clone());
        let real = elem.lookup_key("real").and_then(|v| v.get_str()).map(|s| s.clone())
                       .unwrap_or_else(|| default_real.clone());
        servers.push(Server{ name: name, host: server, port: port, use_ssl: use_ssl,
                             nick: nick, user: user, real: real });
    }

    Ok(Config{
        plugin_dir: plugin_dir,
        reconnect_time: reconnect,
        reconnect_backoff: backoff,
        servers: servers
    })
}

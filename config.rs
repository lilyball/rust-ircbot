use std::{io, os};
use std::io::{IoError, FileNotFound, PathAlreadyExists};
use getopts::{getopts, optflag, optopt, usage, OptGroup};
use toml;

static CONFIG_EXAMPLE: &'static str = include_str!("config.example.toml");

#[deriving(Clone)]
pub struct Config {
    config_dir: Path, // path for the dir where the config file resides
    plugin_dir: Path, // path for the dir where plugins exist
    reconnect_time: Option<uint>,
    reconnect_backoff: bool,
    servers: ~[Server]
}

#[deriving(Clone)]
pub struct Server {
    name: ~str,
    host: ~str,
    port: u16,
    use_ssl: bool,
    nick: ~str,
    user: ~str,
    real: ~str,
    autojoin: ~[Channel]
}

#[deriving(Clone)]
pub struct Channel {
    name: ~str,
    password: Option<~str>
}

pub fn print_usage(opts: &[OptGroup]) {
    let s = usage(format!("Usage: {} [OPTIONS]", os::args()[0]), opts);
    let _ = writeln!(&mut io::stderr(), "{}", s);
}

pub enum Error {
    ErrBadFlag,
    ErrHelpFlag,
    ErrWroteConfig,
    ErrBadConfig,
    ErrIO(IoError)
}

pub fn parse_args() -> Result<Config,Error> {
    let args = os::args();

    let opts = [
        optflag("h", "help", "Displays this help"),
        optopt("c", "config", "Path for the config file, defaults to ~/.rustirc/config", "file")
    ];

    let matches = match getopts(args.tail(), opts) {
        Ok(m) => m,
        Err(f) => {
            let _ = writeln!(&mut io::stderr(), "error: {}\n", f.to_err_msg());
            print_usage(opts);
            return Err(ErrBadFlag);
        }
    };

    if matches.opt_present("h") {
        print_usage(opts);
        return Err(ErrHelpFlag);
    }

    let path = match matches.opt_str("c") {
        None => {
            let p = os::homedir().expect("can't find user's home dir").join(".rustirc/config");
            if !p.exists() {
                let _ = println!("No config file ~/.rustirc/config exists, writing default file");
                match io::fs::mkdir(&p.dir_path(), io::UserDir) {
                    Ok(()) => (),
                    Err(IoError { kind: PathAlreadyExists, .. }) => (),
                    Err(e) => return Err(ErrIO(e))
                }
                let mut f = match io::File::create(&p) {
                    Ok(f) => f,
                    Err(e) => return Err(ErrIO(e))
                };
                match f.write(CONFIG_EXAMPLE.as_bytes()) {
                    Err(e) => return Err(ErrIO(e)),
                    Ok(_) => ()
                }
                return Err(ErrWroteConfig);
            }
            p
        }
        Some(p) => {
            let p = os::make_absolute(&Path::new(p));
            if !p.exists() {
                let _ = writeln!(&mut io::stderr(), "error: config file {} does not exist",
                                 p.display());
                let e = IoError { kind: FileNotFound, desc: "file not found", detail: None };
                return Err(ErrIO(e));
            }
            p
        }
    };

    let root = match toml::parse_from_path(&path) {
        Ok(v) => v,
        Err(toml::ParseError) => return Err(ErrBadConfig),
        Err(toml::IOError(e)) => return Err(ErrIO(e))
    };

    let plugin_dir = match root.lookup("plugin.dir").and_then(|v| v.get_str()) {
        None => {
            let _ = writeln!(&mut io::stderr(),
                             "error: required string config value plugin.dir missing");
            return Err(ErrBadConfig);
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
            let _ = writeln!(&mut io::stderr(), "error: at least one server must be defined");
            return Err(ErrBadConfig);
        }
        Some(ary) => ary.as_slice()
    };
    for elem in server_list.iter() {
        let name = match elem.lookup_key("name").and_then(|v| v.get_str()) {
            None => {
                let _ = writeln!(&mut io::stderr(),
                                 "error: server entry missing required 'name' key");
                return Err(ErrBadConfig);
            }
            Some(s) => s.clone()
        };
        let server = match elem.lookup_key("server").and_then(|v| v.get_str()) {
            None => {
                let _ = writeln!(&mut io::stderr(),
                                 "error: server entry missing required 'server' key");
                return Err(ErrBadConfig);
            }
            Some(s) => s.clone()
        };
        let use_ssl = elem.lookup_key("use_ssl").and_then(|v| v.get_bool()).unwrap_or(false);
        if use_ssl {
            let _ = writeln!(&mut io::stderr(), "error: use_ssl is not currently implemented");
            return Err(ErrBadConfig);
        }
        let default_port = if use_ssl { 6697 } else { 6667 };
        let port = match elem.lookup_key("port").and_then(|v| v.get_int()).unwrap_or(default_port)
                             .to_u16() {
            None => {
                let _ = writeln!(&mut io::stderr(), "error: port is out of range");
                return Err(ErrBadConfig);
            }
            Some(p) => p
        };
        let nick = elem.lookup_key("nick").and_then(|v| v.get_str()).map(|s| s.clone())
                       .unwrap_or_else(|| default_nick.clone());
        let user = elem.lookup_key("user").and_then(|v| v.get_str()).map(|s| s.clone())
                       .unwrap_or_else(|| default_user.clone());
        let real = elem.lookup_key("real").and_then(|v| v.get_str()).map(|s| s.clone())
                       .unwrap_or_else(|| default_real.clone());
        let mut channels = ~[];
        match elem.lookup_key("autojoin").and_then(|v| v.get_vec()) {
            None => (),
            Some(v) => {
                for val in v.iter() {
                    let s = match val.get_str() {
                        None => continue,
                        Some(s) => s
                    };
                    let (name, pass) = match s.find(',') {
                        None => (s.to_owned(), None),
                        Some(idx) => {
                            (s.slice_to(idx).to_owned(), Some(s.slice_from(idx+1).to_owned()))
                        }
                    };
                    channels.push(Channel{ name: name, password: pass });
                }
            }
        }
        servers.push(Server{ name: name, host: server, port: port, use_ssl: use_ssl,
                             nick: nick, user: user, real: real, autojoin: channels });
    }

    let config_dir = path.dir_path();
    let plugin_dir = config_dir.join(plugin_dir);
    Ok(Config{
        config_dir: config_dir,
        plugin_dir: plugin_dir,
        reconnect_time: reconnect,
        reconnect_backoff: backoff,
        servers: servers
    })
}

/* Copyright (C) 2025  Anthony DeDominic
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::{
    collections::HashSet,
    fs,
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    process::exit,
    time::Duration,
};

use serde::{de::DeserializeOwned, Deserialize};

#[derive(Default, Deserialize, Clone)]
pub struct Config {
    pub nick: String,
    pub pass: Option<String>,
    pub host: String,
    #[serde(default)]
    pub tls: bool,
    pub nickserv: Option<String>,
    #[serde(default, alias = "send-burst")]
    pub send_burst: usize,
    #[serde(default, deserialize_with = "from_dur_str", alias = "send-delay")]
    pub send_delay: Duration,
    #[serde(default, deserialize_with = "from_dur_str", alias = "moose-delay")]
    pub moose_delay: Duration,
    #[serde(default = "default_moose_url", alias = "moose-url")]
    pub moose_url: String,
    #[serde(default)]
    pub channels: HashSet<String>,
    #[serde(alias = "invite-file")]
    pub invite_file: Option<PathBuf>,
    #[serde(default, alias = "disable-search")]
    pub disable_search: bool,
}

fn from_dur_str<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    String::deserialize(deserializer).and_then(|dur_str| {
        if dur_str.is_empty() {
            return Err(serde::de::Error::custom(
                "Empty duration is not allowed; please omit or set a value of zero.",
            ));
        }
        let Some(non_num) = dur_str.bytes().position(|b| !b.is_ascii_digit()) else {
            let num = dur_str
                .parse::<u64>()
                .expect("ascii digits should always parse as a valid number.");
            return Ok(Duration::from_millis(num));
        };
        let (num, unit) = dur_str.split_at(non_num);
        if num.is_empty() {
            return Err(serde::de::Error::custom(
                "You must enter a valid number.".to_owned(),
            ));
        }
        let num = num
            .parse()
            .expect("ascii digits should always parse as a valid number.");
        let unit = unit.trim();
        match unit {
            "s" | "secs" | "seconds" => Ok(Duration::from_secs(num)),
            "ns" => Ok(Duration::from_nanos(num)),
            "us" => Ok(Duration::from_micros(num)),
            "ms" => Ok(Duration::from_millis(num)),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid duration unit `{unit}`. should be `s`, `ms`, `us`, `ns`"
            ))),
        }
    })
}

fn default_moose_url() -> String {
    "https://moose2.ghetty.space".to_owned()
}

const EXAMPLE_CONFIG: &[u8] = br###"{ "nick": "MrMoose"
, "host": "irc.rizon.net:6697"
, "// pass": "you can append any field with // to comment it out."
, "pass": "server pass, omit or leave empty."
, "//": "uses NICKSERV IDENTIFY :PASSWORD"
, "nickserv": "nickserv password."
, "tls": true
, "channels":
  [ "#moose-irc2"
  ]
, "//": "how many messages we can send before being throttled."
, "send-burst": 3
, "//": "how long to refill one send token; see above."
, "send-delay": "350ms"
, "//": "time to delay before allowing another moose request."
, "moose-delay": "10s"
, "moose-url": "https://moose2.ghetty.space"
, "//": "you can leave it undefined or blank to disable invites."
, "invite-file": "file to persist invites"
, "//": "some networks may ban you for certain texts that may be repeated in a moose name (Rizon)."
, "disable-search": false
}
"###;

#[derive(clap::Parser, Debug)]
#[command(
    name = "moose-irc2",
    version,
    about = "IRC Bot for serving moose2 content."
)]
pub struct Args {
    #[arg(short, long, help = "Configuration file.")]
    pub config: PathBuf,
    #[arg(short, long, help = "File to persist invites.")]
    pub invites: Option<PathBuf>,
    #[command(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(clap::Subcommand, Clone, Debug)]
pub enum SubCommand {
    #[command(about = "Create example configuration.")]
    Init,
    #[command(about = "Run bot (can be omitted).")]
    Run,
}

fn create_parent_dirs<T: AsRef<Path>>(path: T) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)
    } else {
        Ok(())
    }
}

fn write_default<T>(config_path: T)
where
    T: std::fmt::Debug + AsRef<Path>,
{
    println!("Creating example configuration at: {:?}", &config_path);
    create_parent_dirs(&config_path).unwrap();
    let mut file = std::fs::File::create(&config_path).unwrap();
    file.write_all(EXAMPLE_CONFIG).unwrap();
    println!("Configuration created: Edit the file and restart the application.");
}

fn open_path_and_deserialize<P, D>(path: P) -> Result<D, io::Error>
where
    P: std::fmt::Debug + AsRef<Path>,
    D: DeserializeOwned,
{
    let file = fs::File::open(&path)?;
    let file = BufReader::new(file);
    Ok(serde_json::from_reader(file)?)
}

pub fn save_invite<T>(path: T, invites: &HashSet<String>) -> io::Result<()>
where
    T: AsRef<Path>,
{
    let tdir = path
        .as_ref()
        .parent()
        .expect("Should be unreachable; is only None when PathBuf is an empty string.");
    let r: u64 = rand::random();
    let tdir = tdir.join(format!(".invite.json.{r:x}"));
    let mut invite_tmp = fs::File::create(tdir.clone())?;

    invite_tmp.write_all(
        &serde_json::to_vec(&invites).expect("Should be infallible. it's just a list of strings."),
    )?;
    invite_tmp.sync_data()?;
    drop(invite_tmp);

    fs::rename(tdir, path)?;

    Ok(())
}

pub fn parse_args() -> (Config, Option<HashSet<String>>) {
    let args = <Args as clap::Parser>::parse();
    match args.subcommand.unwrap_or(SubCommand::Run) {
        SubCommand::Init => {
            write_default(&args.config);
            exit(1);
        }
        SubCommand::Run => {
            let mut config = open_path_and_deserialize::<_, Config>(args.config)
                .map_err(|e| {
                    eprintln!("Failed to open configuration: {e}");
                    e
                })
                .unwrap();
            if let Some(invite_file) = args.invites {
                config.invite_file = Some(invite_file)
            };
            let invites = match config.invite_file {
                Some(ref invite) if invite.parent().is_some() => {
                    let invites_list = open_path_and_deserialize::<_, HashSet<String>>(invite)
                        .or_else(|e| match e.kind() {
                            io::ErrorKind::NotFound => Ok(HashSet::from([])),
                            _ => Err(e),
                        })
                        .expect("Could not open invite file.");
                    config.channels.extend(invites_list.clone());
                    Some(invites_list)
                }
                // if parent is none, the path is invalid junk => "" or "/"
                Some(_) => None,
                None => None,
            };
            (config, invites)
        }
    }
}

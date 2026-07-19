use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub cards_dir: PathBuf,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        // read from env so its easier to setup to docker etc.
        // you can pass CARDS_DIR env before running
        // for now we will use our default path and default port 8888
        let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"));
        let default_cards_dir = home.map(PathBuf::from).map_or_else(
            || PathBuf::from("assets/cards"),
            |p| p.join("Documents").join("kizunari").join("cards"),
        );

        let cards_dir = env::var("CARDS_DIR").map_or_else(|_| default_cards_dir, PathBuf::from);

        let port = match env::var("PORT") {
            Ok(s) => s.parse().expect("must be a valid port number"),
            Err(env::VarError::NotPresent) => 8888,
            Err(e) => panic!("sumi failed to read port env\n      reason: {e}"),
        };

        tracing::info!(
            "config loaded\n      port: [{}]\n      card: [{}]",
            port,
            cards_dir.display()
        );

        Self { cards_dir, port }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

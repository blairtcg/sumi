use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub cards_dir: PathBuf,
    pub port: u16,
    pub host: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
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
            Ok(s) => s.parse().map_err(|_| "PORT is not a valid u16 port number".to_string())?,
            Err(env::VarError::NotPresent) => 8888,
            Err(e) => return Err(format!("failed to read PORT env ({e})")),
        };

        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        tracing::info!(
            "config loaded\n      host: [{}]\n      port: [{}]\n      card: [{}]",
            host,
            port,
            cards_dir.display()
        );

        Ok(Self { cards_dir, port, host })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env().expect("failed to load default config")
    }
}

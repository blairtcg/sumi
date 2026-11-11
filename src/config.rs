use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub cards_dir: PathBuf,
    pub port: u16,
    pub host: String,
}

impl Config {
    /*
    =============
    from_env
    =============
    */
    pub fn from_env() -> Result<Self, String> {
        // try to find the user home directory to set a default path.
        // check "USERPROFILE" for windows and "HOME" for macOS/linux.
        let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"));
        
        let default_cards_dir = home.map(PathBuf::from).map_or_else(
            || PathBuf::from("assets/cards"),
            |p| p.join("Documents").join("kizunari").join("cards"), // seth's
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
    /*
    =============
    default
    =============
    */
    fn default() -> Self {
        Self {
            cards_dir: PathBuf::from("assets/cards"),
            port: 8888,
            host: "0.0.0.0".to_string(),
        }
    }
}

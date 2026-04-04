use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub cards_dir: PathBuf,
}

impl Config {
    pub fn load() -> Self {
        // read from env so its easier to setup to docker etc.
        // for now we dont have env set but we will use our default winslop path and default port 8888
        // quick note to velo, kartu ditemuin di folder huty bukan di folder sumi, folder asset sini
        // cuman buat font nya saja buat nomor print di kartu drop.
        let cards_dir = env::var("CARDS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\hp\\Documents\\huty\\cards"));

        let port = env::var("PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8888);

        Self { port, cards_dir }
    }
}

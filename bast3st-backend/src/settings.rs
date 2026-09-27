use config::Config;
use derive_getters::Getters;
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct ServerSettings {
    port: u16,
    #[serde(default)]
    cors: CorsSettings,
    #[serde(default)]
    limits: LimitSettings,
    workers: Option<usize>,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct LimitSettings {
    json: Option<usize>,
    form: Option<usize>,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct CorsSettings {
    #[serde(default)]
    allowed_origins: Vec<String>,
    #[serde(default)]
    allowed_headers: Vec<String>,
    max_age: Option<usize>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            port: 42139,
            cors: Default::default(),
            limits: Default::default(),
            workers: Default::default(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct Settings {
    server: ServerSettings,
    database: Option<DatabaseSettings>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct DatabaseSettings {
    host: String,
    port: Option<u16>,
    database: String,
    username: String,
    password: Option<String>,
}

impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        let Some(configpath) = std::env::args().nth(1) else {
            log::info!("No config file as first argument, use default config");
            return Ok(Default::default());
        };
        let s = Config::builder()
            .add_source(config::File::with_name(&configpath))
            .add_source(
                config::Environment::with_prefix("BAST3ST")
                    .prefix_separator("__")
                    .separator("__"),
            )
            .build()?;

        s.try_deserialize()
    }
}

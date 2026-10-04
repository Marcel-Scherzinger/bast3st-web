use std::{collections::BTreeMap, path::PathBuf};

use config::Config;
use derive_getters::Getters;
use serde::Deserialize;
use sqlx::Either;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct ServerSettings {
    port: u16,
    #[serde(default)]
    cors: CorsSettings,
    #[serde(default)]
    limits: LimitSettings,
    workers: Option<usize>,
}
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct AdminServerSettings {
    enable: bool,
    host: Option<String>,
    port: Option<u16>,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(untagged)]
pub enum NetworkAllowSettings {
    Cmd {
        command: PathBuf,
        #[serde(default)]
        args: Vec<String>,
    },
}

pub type NetworkPolicy =
    EitherUntagged<BTreeMap<String, NetworkAllowSettings>, NetworkAllowSettings>;

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct PolicySettings {
    network: Option<NetworkPolicy>,
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct Settings {
    admin: Option<AdminServerSettings>,
    server: ServerSettings,
    database: DatabaseSettings,
    policy: Option<PolicySettings>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Getters)]
pub struct DatabaseSettings {
    host: String,
    port: Option<u16>,
    database: String,
    username: String,
    password: Option<String>,
}
impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            host: "/var/run/postgresql".into(),
            port: None,
            database: "bast3st".into(),
            username: "postgres".into(),
            password: None,
        }
    }
}

impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        let Some(configpath) = std::env::args()
            .nth(1)
            .or(std::env::var("BAST3ST_CONFIG").ok())
        else {
            let default: Self = Default::default();
            log::warn!("no config file as first argument, use default config: {default:?}");
            log::warn!(
                "the default database configuration is probably not what you want: {:?}",
                default.database()
            );
            return Ok(default);
        };
        log::warn!("reading configuration from {configpath}");
        let s = Config::builder()
            .add_source(config::File::with_name(&configpath))
            .add_source(
                config::Environment::with_prefix("BAST3ST")
                    .prefix_separator("__")
                    .separator("__"),
            )
            .build()?;
        let settings = s.try_deserialize()?;
        log::debug!("apply configuration: {settings:?}");
        Ok(settings)
    }
    pub fn admin_port(&self) -> Option<u16> {
        self.admin()
            .as_ref()
            .map(|admin| admin.port().unwrap_or(42039))
    }
    pub fn network_policy(&self) -> Option<&NetworkPolicy> {
        self.policy.as_ref().and_then(|x| x.network.as_ref())
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(untagged)]
pub enum EitherUntagged<L, R> {
    Left(L),
    Right(R),
}
impl<L, R> EitherUntagged<L, R> {
    pub fn into_either(self) -> Either<L, R> {
        self.into()
    }
    pub fn as_either(&self) -> Either<&L, &R> {
        match self {
            Self::Left(l) => Either::Left(l),
            Self::Right(l) => Either::Right(l),
        }
    }
}
impl<L, R> From<EitherUntagged<L, R>> for Either<L, R> {
    fn from(value: EitherUntagged<L, R>) -> Self {
        match value {
            EitherUntagged::Left(x) => Self::Left(x),
            EitherUntagged::Right(x) => Self::Right(x),
        }
    }
}

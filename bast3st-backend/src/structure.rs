use std::borrow::Cow;

use bast3st_eval::spec::InnerNetworkRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub struct AllowNetworkCmdInput<'a> {
    pub(crate) user: Option<Cow<'a, str>>,
    pub(crate) slot: Option<Cow<'a, str>>,
    pub(crate) scheme: Cow<'a, str>,
    pub(crate) host: Option<Cow<'a, str>>,
    pub(crate) port: Option<u16>,
    pub(crate) path: Cow<'a, str>,
    /// The query parameters
    pub(crate) query: Vec<(Cow<'a, str>, Cow<'a, str>)>,
    /// The unparsed url, if you need to do anything that is lost
    /// in the preprocessed fields
    pub(crate) url: Cow<'a, str>,
    /// The remaining request data
    pub(crate) data: Cow<'a, InnerNetworkRequest>,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub struct ExerciseId<'a> {
    pub(crate) user: Cow<'a, str>,
    pub(crate) slot: Cow<'a, str>,
}
impl ExerciseId<'static> {
    pub fn new_static(user: String, slot: String) -> Self {
        Self {
            user: Cow::Owned(user),
            slot: Cow::Owned(slot),
        }
    }
}

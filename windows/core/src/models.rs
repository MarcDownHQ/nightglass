use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::{Http, M3uSource, StalkerClient, XtreamClient};

/// A live channel, whatever protocol it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub number: Option<i64>,
    pub group: Option<String>,
    pub logo: Option<Url>,
    pub epg_id: Option<String>,
    /// Direct stream for M3U/Xtream. For Stalker this is the portal `cmd`, resolved by `Client::stream_url`.
    pub stream: Url,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Movie {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub poster: Option<Url>,
    pub stream: Url,
    pub rating: Option<f64>,
    pub plot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub poster: Option<Url>,
    pub rating: Option<f64>,
    pub plot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub title: String,
    pub season: i64,
    pub number: i64,
    pub stream: Url,
    pub plot: Option<String>,
}

/// One guide entry. Times are Unix seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Programme {
    pub channel: String,
    pub start: i64,
    pub end: i64,
    pub title: String,
    pub desc: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub channels: Vec<Channel>,
    pub movies: Vec<Movie>,
    pub series: Vec<Series>,
    /// XMLTV guide advertised by the source (M3U header, Xtream `xmltv.php`).
    pub epg_url: Option<Url>,
}

/// Same JSON shape as the macOS app's `SourceKind`: `{"m3u": {"url": …, "epg": …}}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    M3u { url: Url, epg: Option<Url> },
    Xtream { server: Url, username: String, password: String },
    Stalker { portal: Url, mac: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: Uuid,
    pub name: String,
    pub kind: SourceKind,
}

impl Source {
    pub fn new(name: impl Into<String>, kind: SourceKind) -> Self {
        Source { id: Uuid::new_v4(), name: name.into(), kind }
    }

    pub fn client(&self, http: &Http) -> Client {
        match &self.kind {
            SourceKind::M3u { url, epg } => Client::M3u(M3uSource::new(url.clone(), epg.clone(), http.clone())),
            SourceKind::Xtream { server, username, password } => {
                Client::Xtream(XtreamClient::new(server.clone(), username, password, http.clone()))
            }
            SourceKind::Stalker { portal, mac } => Client::Stalker(StalkerClient::new(portal.clone(), mac, http.clone())),
        }
    }
}

/// What every protocol client provides to the app.
pub enum Client {
    M3u(M3uSource),
    Xtream(XtreamClient),
    Stalker(StalkerClient),
}

impl Client {
    pub async fn catalog(&self) -> Result<Catalog> {
        match self {
            Client::M3u(c) => c.catalog().await,
            Client::Xtream(c) => c.catalog().await,
            Client::Stalker(c) => c.catalog().await,
        }
    }

    pub async fn stream_url(&self, channel: &Channel) -> Result<Url> {
        match self {
            Client::Stalker(c) => c.stream_url(channel).await,
            _ => Ok(channel.stream.clone()),
        }
    }

    pub async fn episodes(&self, series: &Series) -> Result<Vec<Episode>> {
        match self {
            Client::Xtream(c) => c.episodes(series).await,
            _ => Ok(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IptvError {
    #[error("Le serveur a répondu {0}.")]
    Http(u16),
    #[error("Identifiants refusés par le serveur.")]
    AuthFailed,
    #[error("Réponse inattendue : {0}.")]
    BadResponse(String),
    #[error("Connexion impossible : {0}")]
    Network(String),
    #[error("Fichier illisible : {0}")]
    File(String),
}

pub type Result<T> = std::result::Result<T, IptvError>;

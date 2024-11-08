use log;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

// TODO Read rhsm.conf instead
const RHSM_IDENTITY_DIRECTORY: &str = "/etc/pki/consumer/";
const ETC_CLIENT: &str = "/etc/insights-client/";
const API_URI: &str = "https://console.redhat.com/api/v1/static/release/";
const USER_AGENT: &str = "insights-core-updater/0.0";

// TODO Respect 'HTTP_PROXY' environment variable family

// Returns true if the system is registered and can fetch Core updates.
pub fn is_registered() -> bool {
    let certificate_path = Path::new(RHSM_IDENTITY_DIRECTORY).join("cert.pem");
    let key_path = Path::new(RHSM_IDENTITY_DIRECTORY).join("key.pem");
    match certificate_path.exists() && key_path.exists() {
        false => {
            log::debug!("RHSM identity does not exist.");
            return false;
        }
        _ => log::debug!("RHSM identity found."),
    }

    match (Path::new(ETC_CLIENT).join(".registered")).exists() {
        false => {
            log::debug!("File .registered does not exist.");
            false
        }
        _ => {
            log::debug!("File .registered found.");
            true
        }
    }
}


#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CoreInfo {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}


impl From<&http::header::HeaderMap> for CoreInfo {
    fn from(headers: &http::header::HeaderMap) -> Self {
        CoreInfo {
            etag: match headers.get("ETag") {
                Some(value) => Some(value.to_str().unwrap().to_string()),
                None => None,
            },
            last_modified: match headers.get("Last-Modified") {
                Some(value) => Some(value.to_str().unwrap().to_string()),
                None => None,
            },
        }
    }
}


impl CoreInfo {
    pub async fn fetch() -> Option<Self> {
        let core_path = format!("{}{}", API_URI, "insights-core.egg");
        log::debug!("Querying for Core cache at {}.", core_path);

        let client = reqwest::Client::new();
        let resp: Result<reqwest::Response, reqwest::Error> = client
            .head(core_path.as_str())
            .header("User-Agent", USER_AGENT)
            .send()
            .await;

        if let Err(e) = resp {
            log::error!("Could not query for Core cache: {}.", e);
            return None;
        }
        if resp.is_err() {
            log::error!("Could not query for Core cache: {}.", resp.err().unwrap());
            return None;
        }

        let resp: reqwest::Response = resp.unwrap();
        let core_info: CoreInfo = CoreInfo::from(resp.headers());
        log::info!("Received Core cache: {:?}.", core_info);
        return Some(core_info);
    }

    pub fn new() -> Self {
        Self { etag: None, last_modified: None }
    }

    pub fn from_cache(path: &str) -> Option<Self> {
        let fp = match File::open(path) {
            Err(e) => {
                log::warn!("Could not open cache file: {}.", e);
                return None;
            }
            Ok(f) => f,
        };
        let core_info = match serde_json::from_reader(fp) {
            Err(e) => {
                log::warn!("Could not parse cache file: {}.", e);
                return None;
            }
            Ok(i) => i,
        };
        let core_info = core_info;
        log::debug!("Loaded Core cache: {:?}.", core_info);
        return Some(core_info);
    }

    // TODO Return a rich error
    pub fn cache(&self, path: &str) -> Option<()> {
        let fp: File = match OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
        {
            Ok(f) => f,
            Err(e) => {
                log::error!("Could not open Core file: {}", e);
                return None;
            }
        };
        _ = match serde_json::to_writer_pretty(fp, self) {
            Err(e) => {
                log::warn!("Could not serialize cache file: {}.", e);
                return None;
            }
            Ok(i) => i,
        };
        log::debug!("Core cache cached to {}.", path);

        return Some(());
    }
}

#[derive(Debug)]
pub struct Core {
    pub info: CoreInfo,
    pub data: bytes::Bytes,
    signature: bytes::Bytes,
}

impl Core {
    pub async fn fetch() -> Option<Self> {
        let core_path = format!("{}{}", API_URI, "insights-core.egg");
        log::debug!("Querying for Core at {}.", core_path);

        // TODO Add If-None-Match when non-legacy API is fixed
        //  request.header("If-None-Match", "");
        let client = reqwest::Client::new();
        let resp: Result<reqwest::Response, reqwest::Error> = client
            .get(core_path.as_str())
            .header("User-Agent", USER_AGENT)
            .send()
            .await;

        if let Err(e) = resp {
            log::error!("Could not query for Core: {}.", e);
            return None;
        }
        if resp.is_err() {
            log::error!("Could not query for Core: {}.", resp.err().unwrap());
            return None;
        }

        let resp: reqwest::Response = resp.unwrap();
        let core_info = CoreInfo::from(resp.headers());

        let core_data = match resp.bytes().await {
            Err(e) => {
                log::error!("Could not read Core data: {}.", e);
                return None;
            }
            Ok(i) => i,
        };
        let core = Core { info: core_info, data: core_data, signature: bytes::Bytes::new() };
        log::info!("Core received.");
        return Some(core);
    }

    pub async fn fetch_signature(&mut self) -> Option<()> {
        let signature_path = format!("{}{}", API_URI, "insights-core.egg.asc");
        log::debug!("Querying for Core signature at {}.", signature_path);

        let client = reqwest::Client::new();
        let resp: Result<reqwest::Response, reqwest::Error> = client
            .get(signature_path.as_str())
            .header("User-Agent", USER_AGENT)
            .send()
            .await;

        if let Err(e) = resp {
            log::error!("Could not query for Core signature: {}.", e);
            return None;
        }
        if resp.is_err() {
            log::error!("Could not query for Core signature: {}.", resp.err().unwrap());
            return None;
        }

        let resp: reqwest::Response = resp.unwrap();
        let signature_data: bytes::Bytes = match resp.bytes().await {
            Err(e) => {
                log::error!("Could not read Core signature data: {}.", e);
                return None;
            }
            Ok(i) => i,
        };
        self.signature = signature_data;
        log::info!("Core signature received.");
        return Some(());
    }

    // TODO Return rich error
    pub fn cache(&self, path: &str, signature_path: &str) -> Option<()> {
        let mut fp: File = match OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
        {
            Ok(f) => f,
            Err(e) => {
                log::error!("Could not open Core file: {}", e);
                return None;
            }
        };
        match fp.write(self.data.as_ref()) {
            Ok(_) => (),
            Err(e) => {
                log::error!("Could not write Core file: {}.", e);
                return None;
            }
        }
        log::debug!("Core cached into {}.", path);

        let mut fp: File = match OpenOptions::new()
            .write(true)
            .create(true)
            .open(signature_path)
        {
            Ok(f) => f,
            Err(e) => {
                log::error!("Could not open Core signature file: {}.", e);
                return None;
            }
        };
        match fp.write(self.signature.as_ref()) {
            Ok(_) => (),
            Err(e) => {
                log::error!("Could not write Core signature file: {}.", e);
                return None;
            }
        }
        log::debug!("Core signature cached into {}.", signature_path);

        return Some(());
    }
}

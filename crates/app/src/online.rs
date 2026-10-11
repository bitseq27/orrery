//! Optional online layer (R15): three official endpoints, event-driven only.
//!   Horizons  ssd.jpl.nasa.gov/api/horizons.api   (keyless)  verify/refresh
//!   Exo TAP   exoplanetarchive.ipac.caltech.edu   (keyless)  catalog source
//!   NEO feed  api.nasa.gov/neo                    (KEYED)    live Sol NEO list
//! Every call routes through the global token bucket (§5.4). LINK = OFF ⇒ zero calls.

use crate::rate_limiter::TokenBucket;

#[derive(Clone, Debug, PartialEq)]
pub enum SvcStatus {
    Idle,
    Ok(String),
    Wait,
    NoKey,
    Fail(String),
}

impl Default for SvcStatus {
    fn default() -> Self {
        SvcStatus::Idle
    }
}

impl SvcStatus {
    pub fn label(&self) -> String {
        match self {
            SvcStatus::Idle => "--".into(),
            SvcStatus::Ok(s) => format!("OK {s}"),
            SvcStatus::Wait => "WAIT".into(),
            SvcStatus::NoKey => "NO KEY".into(),
            SvcStatus::Fail(_) => "FAIL".into(),
        }
    }
}

#[derive(Default)]
pub struct LinkState {
    pub horizons: SvcStatus,
    pub exo: SvcStatus,
    pub neo: SvcStatus,
    pub last_test: String,
    pub resid_arcsec: Option<f64>,
}

fn now_s() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// One throwaway call per service (TEST key), respecting the bucket.
/// Runs on a worker thread; results land in the shared LinkState.
pub fn run_test_suite(
    key: Option<String>,
    bucket: std::sync::Arc<std::sync::Mutex<TokenBucket>>,
    state: std::sync::Arc<std::sync::Mutex<LinkState>>,
) {
    std::thread::spawn(move || {
        // 1) Horizons: Mars vector, single epoch
        let h = gated_call(&bucket, || {
            let u = "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='499'&OBJ_DATA=NO&MAKE_EPHEM=YES&EPHEM_TYPE=VECTORS&CENTER='500@10'&TLIST=2461322.5&REF_SYSTEM=ICRF&OUT_UNITS=AU-D&VEC_TABLE=2";
            get(u).map(|t| format!("{}B", t.len()))
        });
        state.lock().unwrap().horizons = h;

        // 2) Exoplanet Archive TAP: one-row query
        let e = gated_call(&bucket, || {
            let u = "https://exoplanetarchive.ipac.caltech.edu/TAP/sync?query=select+pl_name+from+pscomppars+where+hostname='TRAPPIST-1'&format=csv";
            get(u).map(|t| format!("{}B", t.len()))
        });
        state.lock().unwrap().exo = e;

        // 3) NEO feed: keyed
        let n = match &key {
            None => SvcStatus::NoKey,
            Some(k) => gated_call(&bucket, || {
                let u = format!("https://api.nasa.gov/neo/rest/v1/feed?api_key={k}");
                get(&u).map(|t| format!("{}B", t.len()))
            }),
        };
        state.lock().unwrap().neo = n;
        state.lock().unwrap().last_test = orrery_astro::time::jd_to_utc_str(orrery_astro::time::now_jd_tt());
    });
}

fn gated_call<F: FnOnce() -> Result<String, String>>(bucket: &std::sync::Mutex<TokenBucket>, f: F) -> SvcStatus {
    let mut b = bucket.lock().unwrap();
    let t = now_s();
    if !b.try_acquire(t) {
        return SvcStatus::Wait;
    }
    drop(b);
    match f() {
        Ok(s) => SvcStatus::Ok(s),
        Err(e) => SvcStatus::Fail(e),
    }
}

fn get(url: &str) -> Result<String, String> {
    use ureq::config::Config;
    let agent = ureq::Agent::new_with_config(
        Config::builder()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build(),
    );
    agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// NEO close-approach list for the Sol scene (only ever called with LINK = ON).
pub fn neo_feed(_key: &str) -> Result<String, String> {
    Err("neo feed listing arrives with M6 polish".into())
}

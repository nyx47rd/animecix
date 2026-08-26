use std::sync::{Arc, mpsc};
use std::time::Duration;

enum Method {
    Get,
    Post,
    Head,
}

struct Spec {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    query: Option<Vec<(String, String)>>,
    json_body: Option<serde_json::Value>,
    timeout_secs: Option<u64>,
}

struct RawResp {
    status: u16,
    url: String,
    content_length: Option<u64>,
    body: Vec<u8>,
}

type Job = Box<dyn FnOnce(&wreq::Client, &tokio::runtime::Runtime) + Send>;

struct Inner {
    client: wreq::Client,
    tx: mpsc::Sender<Job>,
}

#[derive(Clone)]
pub(crate) struct Http {
    inner: Arc<Inner>,
}

pub(crate) struct Resp {
    raw: RawResp,
}

impl Resp {
    pub fn url(&self) -> String {
        self.raw.url.clone()
    }

    pub fn content_length(&self) -> Option<u64> {
        self.raw.content_length
    }

    pub fn error_for_status(self) -> Result<Self, String> {
        if (200..300).contains(&self.raw.status) {
            Ok(self)
        } else {
            Err(format!("HTTP {}", self.raw.status))
        }
    }

    pub fn text(self) -> Result<String, String> {
        String::from_utf8(self.raw.body).map_err(|_| "yanıt UTF-8 değil".to_string())
    }

    pub fn bytes(self) -> Result<Vec<u8>, String> {
        Ok(self.raw.body)
    }

    pub fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, String> {
        serde_json::from_slice(&self.raw.body).map_err(|e| format!("JSON ayrıştırma hatası: {e}"))
    }
}

pub(crate) struct ReqB<'a> {
    http: &'a Http,
    spec: Spec,
}

impl ReqB<'_> {
    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.spec.headers.push((k.to_string(), v.to_string()));
        self
    }

    pub fn query(mut self, q: &[(&str, &str)]) -> Self {
        self.spec.query = Some(
            q.iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        );
        self
    }

    pub fn json(mut self, v: &serde_json::Value) -> Self {
        self.spec.json_body = Some(v.clone());
        self
    }

    pub fn timeout(mut self, secs: u64) -> Self {
        self.spec.timeout_secs = Some(secs);
        self
    }

    pub fn send(self) -> Result<Resp, String> {
        let (tx, rx) = mpsc::channel::<Result<RawResp, String>>();
        let inner = self.http.inner.clone();
        let spec = self.spec;
        let job: Job = Box::new(move |client, rt| {
            let _ = tx.send(exec_on(client, rt, spec));
        });
        inner
            .tx
            .send(job)
            .map_err(|_| "HTTP arka plan iş parçacığı kapandı".to_string())?;
        rx.recv()
            .map_err(|_| "HTTP arka plan iş parçacığı kapandı".to_string())?
            .map(|raw| Resp { raw })
    }

}

fn exec_on(client: &wreq::Client, rt: &tokio::runtime::Runtime, spec: Spec) -> Result<RawResp, String> {
    let mut url = spec.url.clone();
    if let Some(q) = &spec.query {
        if !q.is_empty() {
            let qs: Vec<String> = q
                .iter()
                .map(|(k, v)| format!("{}={}", pct_encode(k), pct_encode(v)))
                .collect();
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(&qs.join("&"));
        }
    }
    let mut rb = match spec.method {
        Method::Get => client.get(&url),
        Method::Post => client.post(&url),
        Method::Head => client.head(&url),
    };
    for (k, v) in &spec.headers {
        rb = rb.header(k, v);
    }
    if let Some(j) = &spec.json_body {
        let body = serde_json::to_vec(j).map_err(|e| e.to_string())?;
        rb = rb.header("content-type", "application/json");
        rb = rb.body(body);
    }
    if let Some(t) = spec.timeout_secs {
        rb = rb.timeout(Duration::from_secs(t));
    }
    let resp = rt.block_on(rb.send()).map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let final_url = resp.uri().to_string();
    let content_length = resp.content_length();
    let body = rt.block_on(resp.bytes()).map_err(|e| e.to_string())?.to_vec();
    Ok(RawResp {
        status,
        url: final_url,
        content_length,
        body,
    })
}

impl Http {
    pub fn new(proxy: Option<&str>) -> Result<Self, String> {
        let mut b = wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome149)
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5));
        if let Some(p) = proxy {
            let pr = wreq::Proxy::all(p).map_err(|e| e.to_string())?;
            b = b.proxy(pr);
        }
        let client = b.build().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel::<Job>();
        let thread_client = client.clone();
        std::thread::Builder::new()
            .name("http".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                for job in rx {
                    job(&thread_client, &rt);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            inner: Arc::new(Inner { client, tx }),
        })
    }

    pub fn get(&self, url: impl Into<String>) -> ReqB<'_> {
        ReqB {
            http: self,
            spec: Spec {
                method: Method::Get,
                url: url.into(),
                headers: Vec::new(),
                query: None,
                json_body: None,
                timeout_secs: None,
            },
        }
    }

    pub fn post(&self, url: impl Into<String>) -> ReqB<'_> {
        ReqB {
            http: self,
            spec: Spec {
                method: Method::Post,
                url: url.into(),
                headers: Vec::new(),
                query: None,
                json_body: None,
                timeout_secs: None,
            },
        }
    }

    pub fn head(&self, url: impl Into<String>) -> ReqB<'_> {
        ReqB {
            http: self,
            spec: Spec {
                method: Method::Head,
                url: url.into(),
                headers: Vec::new(),
                query: None,
                json_body: None,
                timeout_secs: None,
            },
        }
    }
}


fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'%' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

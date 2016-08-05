use std::io;
use std::fmt;
use std::time;
use std::io::{Read, Write};
use std::cell::{RefMut, RefCell};

use monitorid::MonitorId;

use curl;
use serde::{Serialize, Deserialize};
use serde_json;


#[derive(Debug, Deserialize)]
struct ErrorInfo {
    detail: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct RunStart {
    pub timestamp: Option<f64>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub from_cron: Option<bool>,
}

#[derive(Serialize)]
pub struct RunFailure {
    pub status: i32,
    pub timestamp: f64,
    pub output: Option<String>,
}

#[derive(Serialize)]
pub struct RunComplete {
    pub timestamp: f64,
}

#[derive(Deserialize)]
pub struct MonitorStatus {
    pub status: i32,
}

pub struct Api<'a> {
    monitor_id: &'a MonitorId,
    shared_handle: RefCell<curl::easy::Easy>,
}

#[derive(PartialEq, Debug)]
pub enum Method {
    Get,
    Post,
}

pub enum Error {
    Http(u32, String),
    Curl(curl::Error),
    Io(io::Error),
    Json(serde_json::Error),
}

pub type ApiResult<T> = Result<T, Error>;

pub struct ApiRequest<'a> {
    handle: RefMut<'a, curl::easy::Easy>,
    headers: curl::easy::List,
    body: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct ApiResponse {
    status: u32,
    body: Vec<u8>,
}

impl<'a> ApiRequest<'a> {

    fn new(mut handle: RefMut<'a, curl::easy::Easy>,
           method: Method, url: &str)
        -> ApiResult<ApiRequest<'a>>
    {
        let mut headers = curl::easy::List::new();
        headers.append("Expect:").ok();
        headers.append(&format!("User-Agent: sentry-cronwell")).ok();

        match method {
            Method::Get => handle.get(true)?,
            Method::Post => handle.custom_request("POST")?,
        }

        handle.url(&url)?;

        Ok(ApiRequest {
            handle: handle,
            headers: headers,
            body: None,
        })
    }

    pub fn with_json_body<S: Serialize>(mut self, body: &S) -> ApiResult<ApiRequest<'a>> {
        let mut body_bytes : Vec<u8> = vec![];
        serde_json::to_writer(&mut body_bytes, &body)?;
        self.body = Some(body_bytes);
        self.headers.append("Content-Type: application/json")?;
        Ok(self)
    }

    pub fn send(mut self) -> ApiResult<ApiResponse> {
        let mut out = vec![];
        self.handle.http_headers(self.headers)?;
        let (status, _) = send_req(&mut self.handle, &mut out, self.body)?;
        Ok(ApiResponse {
            status: status,
            body: out,
        })
    }
}


impl<'a> Api<'a> {

    pub fn new(monitor_id: &'a MonitorId) -> Api<'a> {
        Api {
            monitor_id: monitor_id,
            shared_handle: RefCell::new(curl::easy::Easy::new()),
        }
    }

    pub fn request(&'a self, method: Method, url: &str) -> ApiResult<ApiRequest<'a>> {
        let mut handle = self.shared_handle.borrow_mut();
        ApiRequest::new(handle, method, url)
    }

    pub fn post<S: Serialize>(&self, path: &str, body: &S) -> ApiResult<ApiResponse> {
        self.request(Method::Post, path)?.with_json_body(body)?.send()
    }

    pub fn send_start(&self, res: &RunStart) -> ApiResult<MonitorStatus>
    {
        self.post(&format!("{}start/", self.monitor_id.api_url()), res)?.convert()
    }

    pub fn send_failure(&self, res: &RunFailure) -> ApiResult<MonitorStatus>
    {
        self.post(&format!("{}fail/", self.monitor_id.api_url()), res)?.convert()
    }

    pub fn send_complete(&self, res: &RunComplete) -> ApiResult<MonitorStatus>
    {
        self.post(&format!("{}complete/", self.monitor_id.api_url()), res)?.convert()
    }
}

fn send_req<W: Write>(handle: &mut curl::easy::Easy,
                      out: &mut W, body: Option<Vec<u8>>)
    -> ApiResult<(u32, Vec<String>)>
{
    match body {
        Some(body) => {
            let mut body = &body[..];
            handle.upload(true)?;
            handle.in_filesize(body.len() as u64)?;
            handle_req(handle, out,
                       &mut |buf| body.read(buf).unwrap_or(0))
        },
        None => {
            handle_req(handle, out, &mut |_| 0)
        }
    }
}

fn handle_req<W: Write>(handle: &mut curl::easy::Easy,
                        out: &mut W,
                        read: &mut FnMut(&mut [u8]) -> usize)
    -> ApiResult<(u32, Vec<String>)>
{
    let mut headers = Vec::new();
    {
        let mut handle = handle.transfer();
        handle.read_function(|buf| Ok(read(buf)))?;
        handle.write_function(|data| {
            Ok(match out.write_all(data) {
                Ok(_) => data.len(),
                Err(_) => 0,
            })
        })?;
        handle.header_function(|data| {
            headers.push(String::from_utf8_lossy(data).into_owned());
            true
        })?;
        handle.perform()?;
    }

    Ok((handle.response_code()?, headers))
}

impl ApiResponse {

    pub fn status(&self) -> u32 {
        self.status
    }

    pub fn failed(&self) -> bool {
        self.status >= 400 && self.status <= 600
    }

    pub fn ok(&self) -> bool {
        !self.failed()
    }

    pub fn to_result(self) -> ApiResult<ApiResponse> {
        if self.ok() {
            return Ok(self);
        }
        if let Ok(err) = self.deserialize::<ErrorInfo>() {
            if let Some(detail) = err.detail.or(err.error) {
                fail!(Error::Http(self.status(), detail));
            }
        }
        fail!(Error::Http(self.status(), "generic error".into()));
    }

    pub fn deserialize<T: Deserialize>(&self) -> ApiResult<T> {
        Ok(serde_json::from_reader(&self.body[..])?)
    }

    pub fn convert<T: Deserialize>(self) -> ApiResult<T> {
        self.to_result().and_then(|x| x.deserialize())
    }
}

impl From<curl::Error> for Error {
    fn from(err: curl::Error) -> Error {
        Error::Curl(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error::Json(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Http(status, ref msg) => write!(f, "http error: {} ({})",
                                                   msg, status),
            Error::Curl(ref err) => write!(f, "http error: {}", err),
            Error::Io(ref err) => write!(f, "io error: {}", err),
            Error::Json(ref err) => write!(f, "bad json: {}", err),
        }
    }
}

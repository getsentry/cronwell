use std::time;
use std::str::FromStr;
use std::borrow::Cow;

use curl;
use url::Url;
use base64;

use api::{Api, RunStart, RunFailure, RunComplete, MonitorStatus};
use error::Error;
use utils::{run_from_cron, to_timestamp};


#[derive(Debug)]
pub struct MonitorId {
    url: Url,
}

fn try_decode_monitor_token(s: &str) -> Option<Cow<str>> {
    if s.starts_with("http://") || s.starts_with("https://") {
        Some(Cow::Borrowed(s))
    } else if let Ok(x) = base64::decode(s) {
        String::from_utf8(x).ok().map(|x| Cow::Owned(x))
    } else {
        None
    }
}


impl MonitorId {
    pub fn is_secure(&self) -> bool {
        self.url.scheme() == "https"
    }

    pub fn api_url(&self) -> &Url {
        &self.url
    }

    pub fn token(&self) -> String {
        let mut rv = base64::encode(self.url.as_str().as_bytes());
        let mut new_len = rv.len();
        while new_len > 0 && &rv[new_len - 1..new_len] == "=" {
            new_len -= 1;
        }
        rv.truncate(new_len);
        rv
    }

    pub fn report_start(&self, cmd: &str, args: &[&str])
        -> Result<MonitorStatus, Error>
    {
        Ok(Api::new(self).send_start(&RunStart {
            timestamp: Some(to_timestamp(time::SystemTime::now())),
            command: Some(cmd.to_string()),
            args: Some(args.iter().map(|x| x.to_string()).collect()),
            from_cron: Some(run_from_cron()),
        })?)
    }

    pub fn report_failure<I>(&self, lines: I, status: i32)
        -> Result<MonitorStatus, Error>
    where
        I: Iterator<Item=String>
    {
        let mut output = String::new();
        for (idx, line) in lines.enumerate() {
            if idx > 0 {
                output.push('\n');
            }
            output.push_str(&line);
        }
        Ok(Api::new(self).send_failure(&RunFailure {
            status: status,
            timestamp: to_timestamp(time::SystemTime::now()),
            output: Some(output),
        })?)
    }

    pub fn report_complete(&self) -> Result<MonitorStatus, Error> {
        Ok(Api::new(self).send_complete(&RunComplete {
            timestamp: to_timestamp(time::SystemTime::now()),
        })?)
    }
}

impl FromStr for MonitorId {
    type Err = Error;

    fn from_str(s: &str) -> Result<MonitorId, Error> {
        let url = try_decode_monitor_token(s)
            .and_then(|x| Url::parse(&x).ok())
            .ok_or("Malformed monitor token")?;

        if url.scheme() != "http" && url.scheme() != "https" {
            fail!("Unsupported monitor token: bad scheme {}", url.scheme());
        }

        Ok(MonitorId {
            url: url,
        })
    }
}

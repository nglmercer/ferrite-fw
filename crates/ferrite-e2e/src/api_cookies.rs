//! Enumerable RFC6265 cookie provider, preserving cookies across redirects.
use crate::{Cookie, E2eError, E2eResult};
use reqwest::{header::HeaderValue, Url};
use std::sync::Mutex;

#[derive(Default)]
struct State {
    store: cookie_store::CookieStore,
    changes: Vec<(Cookie, String)>,
}
#[derive(Default)]
pub(crate) struct ApiCookieJar {
    state: Mutex<State>,
}
impl ApiCookieJar {
    pub(crate) fn clear(&self) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = State::default();
    }
    pub(crate) fn snapshot(&self) -> Vec<Cookie> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut cookies: Vec<_> = state.store.iter_unexpired().map(export).collect();
        cookies.sort_by(|a, b| (&a.domain, &a.path, &a.name).cmp(&(&b.domain, &b.path, &b.name)));
        cookies
    }
    pub(crate) fn take_changes(&self) -> Vec<(Cookie, String)> {
        std::mem::take(&mut self.state.lock().unwrap_or_else(|e| e.into_inner()).changes)
    }
    pub(crate) fn restore(&self, cookies: &[Cookie], base_url: Option<&str>) -> E2eResult<()> {
        // Validate before replacing the live jar, so invalid state leaves it intact.
        let mut store = cookie_store::CookieStore::default();
        for value in cookies {
            let url = match &value.domain {
                Some(domain) => Url::parse(&format!(
                    "{}://{}/",
                    if value.secure { "https" } else { "http" },
                    domain.trim_start_matches('.')
                )),
                None => Url::parse(base_url.ok_or_else(|| {
                    E2eError::Config("API storage cookie requires a domain or base URL".into())
                })?),
            }
            .map_err(|error| E2eError::Config(error.to_string()))?;
            let mut builder = cookie::Cookie::build((value.name.clone(), value.value.clone()))
                .path(value.path.clone().unwrap_or_else(|| "/".into()))
                .secure(value.secure)
                .http_only(value.http_only);
            if let Some(domain) = value.domain.as_ref().filter(|d| d.starts_with('.')) {
                builder = builder.domain(domain.clone());
            }
            if let Some(policy) = &value.same_site {
                let policy = match policy.to_ascii_lowercase().as_str() {
                    "strict" => cookie::SameSite::Strict,
                    "lax" => cookie::SameSite::Lax,
                    "none" => cookie::SameSite::None,
                    _ => return Err(E2eError::Config("invalid SameSite policy".into())),
                };
                builder = builder.same_site(policy);
            }
            if let Some(expiry) = value.expires.filter(|e| *e >= 0) {
                builder = builder.expires(
                    cookie::time::OffsetDateTime::from_unix_timestamp(expiry)
                        .map_err(|e| E2eError::Config(e.to_string()))?,
                );
            }
            match store.insert_raw(&builder.build(), &url) {
                Ok(_) | Err(cookie_store::CookieError::Expired) => {}
                Err(error) => {
                    return Err(E2eError::Config(format!(
                        "invalid API storage cookie: {error}"
                    )))
                }
            }
        }
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = State {
            store,
            changes: Vec::new(),
        };
        Ok(())
    }
}
impl reqwest::cookie::CookieStore for ApiCookieJar {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        for header in headers {
            if let Some(raw) = header
                .to_str()
                .ok()
                .and_then(|s| cookie::Cookie::parse(s.to_string()).ok())
            {
                if let Ok(parsed) = cookie_store::Cookie::try_from_raw_cookie(&raw, url) {
                    let value = export(&parsed);
                    if matches!(
                        state.store.insert_raw(&raw, url),
                        Ok(_) | Err(cookie_store::CookieError::Expired)
                    ) {
                        state.changes.push((value, url.to_string()));
                    }
                }
            }
        }
    }
    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let value = state
            .store
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        if value.is_empty() {
            None
        } else {
            HeaderValue::from_str(&value).ok()
        }
    }
}
fn export(cookie: &cookie_store::Cookie<'_>) -> Cookie {
    let domain = match &cookie.domain {
        cookie_store::CookieDomain::HostOnly(domain) => domain.clone(),
        cookie_store::CookieDomain::Suffix(domain) => format!(".{domain}"),
        _ => String::new(),
    };
    Cookie {
        name: cookie.name().into(),
        value: cookie.value().into(),
        domain: Some(domain),
        path: Some(cookie.path.as_ref().into()),
        http_only: cookie.http_only().unwrap_or(false),
        secure: cookie.secure().unwrap_or(false),
        same_site: cookie.same_site().map(|policy| format!("{policy:?}")),
        expires: match &cookie.expires {
            cookie_store::CookieExpiration::AtUtc(time) => Some(time.unix_timestamp()),
            cookie_store::CookieExpiration::SessionEnd => None,
        },
    }
}

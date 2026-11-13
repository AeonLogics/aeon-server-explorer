#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub enum UploadEvent {
    Progress { sent: f64, total: f64 },
    Done(Result<String, String>),
}

pub use imp::{send_picked, UploadHandle};

// XMLHttpRequest rather than fetch, because only XHR reports upload progress.
#[cfg(feature = "hydrate")]
mod imp {
    use std::cell::Cell;
    use std::rc::Rc;

    use send_wrapper::SendWrapper;
    use wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{Event, HtmlInputElement, ProgressEvent, XmlHttpRequest};

    use super::UploadEvent;

    struct Inner {
        xhr: XmlHttpRequest,
        finished: Rc<Cell<bool>>,
        _progress: Closure<dyn FnMut(ProgressEvent)>,
        _load: Closure<dyn FnMut(Event)>,
        _error: Closure<dyn FnMut(Event)>,
    }

    // Dropping the handle (Cancel, or leaving the page) stops an unfinished upload.
    impl Drop for Inner {
        fn drop(&mut self) {
            if let Ok(upload) = self.xhr.upload() {
                upload.set_onprogress(None);
            }
            self.xhr.set_onload(None);
            self.xhr.set_onerror(None);
            if !self.finished.get() {
                let _ = self.xhr.abort();
            }
        }
    }

    pub struct UploadHandle(#[allow(dead_code)] SendWrapper<Inner>);

    // Sends the file picked in a file input. Returns its name and size, plus the handle.
    pub fn send_picked(
        ev: &leptos::ev::Event,
        url: &str,
        content_type: &str,
        on_event: impl Fn(UploadEvent) + 'static,
    ) -> Option<(String, f64, UploadHandle)> {
        let input: HtmlInputElement = ev.target()?.dyn_into().ok()?;
        let file = input.files()?.get(0)?;
        input.set_value("");
        let name = file.name();
        let size = file.size();

        let xhr = XmlHttpRequest::new().ok()?;
        xhr.open("POST", url).ok()?;
        xhr.set_request_header("Content-Type", content_type).ok()?;
        let ascii: String = name.chars().map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '_' }).collect();
        xhr.set_request_header("X-File-Name", &ascii).ok()?;

        let on_event = Rc::new(on_event);
        let finished = Rc::new(Cell::new(false));

        let progress = {
            let on_event = on_event.clone();
            Closure::<dyn FnMut(ProgressEvent)>::new(move |e: ProgressEvent| {
                on_event(UploadEvent::Progress { sent: e.loaded(), total: e.total() });
            })
        };
        xhr.upload().ok()?.set_onprogress(Some(progress.as_ref().unchecked_ref()));

        let load = {
            let (on_event, finished, xhr) = (on_event.clone(), finished.clone(), xhr.clone());
            Closure::<dyn FnMut(Event)>::new(move |_: Event| {
                finished.set(true);
                let status = xhr.status().unwrap_or(0);
                let text = xhr.response_text().ok().flatten().unwrap_or_default();
                let result = match status {
                    200 => Ok(text),
                    401 => Err("Your session has ended. Sign in again, then upload again.".into()),
                    _ if text.trim().is_empty() => Err(format!("The server answered {status}.")),
                    _ => Err(text),
                };
                on_event(UploadEvent::Done(result));
            })
        };
        xhr.set_onload(Some(load.as_ref().unchecked_ref()));

        let error = {
            let (on_event, finished) = (on_event.clone(), finished.clone());
            Closure::<dyn FnMut(Event)>::new(move |_: Event| {
                finished.set(true);
                on_event(UploadEvent::Done(Err(
                    "The upload was cut off. Is the connection (or SSH tunnel) still open?".into(),
                )));
            })
        };
        xhr.set_onerror(Some(error.as_ref().unchecked_ref()));

        xhr.send_with_opt_blob(Some(&file)).ok()?;
        let inner = Inner { xhr, finished, _progress: progress, _load: load, _error: error };
        Some((name, size, UploadHandle(SendWrapper::new(inner))))
    }
}

#[cfg(not(feature = "hydrate"))]
mod imp {
    use super::UploadEvent;

    pub struct UploadHandle;

    pub fn send_picked(
        _ev: &leptos::ev::Event,
        _url: &str,
        _content_type: &str,
        _on_event: impl Fn(UploadEvent) + 'static,
    ) -> Option<(String, f64, UploadHandle)> {
        None
    }
}

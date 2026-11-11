#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub enum SseEvent {
    Open,
    Data(String),
    Failure(String),
    End,
    Lost { closed: bool },
}

pub use imp::{connect, SseHandle};

#[cfg(feature = "hydrate")]
mod imp {
    use std::rc::Rc;

    use send_wrapper::SendWrapper;
    use wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{Event, EventSource, MessageEvent};

    use super::SseEvent;

    struct Inner {
        source: EventSource,
        _message: Closure<dyn FnMut(MessageEvent)>,
        _named: Vec<Closure<dyn FnMut(MessageEvent)>>,
        _plain: Vec<Closure<dyn FnMut(Event)>>,
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            self.source.close();
        }
    }

    // Closes the stream when dropped (put it in `on_cleanup`).
    pub struct SseHandle(#[allow(dead_code)] SendWrapper<Inner>);

    pub fn connect(url: &str, on_event: impl Fn(SseEvent) + 'static) -> Option<SseHandle> {
        let source = EventSource::new(url).ok()?;
        let on_event = Rc::new(on_event);

        let text = |ev: &MessageEvent| ev.data().as_string().unwrap_or_default();

        let message = {
            let on_event = on_event.clone();
            Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
                on_event(SseEvent::Data(text(&ev)))
            })
        };
        source.set_onmessage(Some(message.as_ref().unchecked_ref()));

        let failure = {
            let on_event = on_event.clone();
            let source = source.clone();
            Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
                source.close();
                on_event(SseEvent::Failure(text(&ev)))
            })
        };
        let end = {
            let on_event = on_event.clone();
            let source = source.clone();
            Closure::<dyn FnMut(MessageEvent)>::new(move |_: MessageEvent| {
                source.close();
                on_event(SseEvent::End)
            })
        };
        let _ = source.add_event_listener_with_callback("failure", failure.as_ref().unchecked_ref());
        let _ = source.add_event_listener_with_callback("end", end.as_ref().unchecked_ref());

        let open = {
            let on_event = on_event.clone();
            Closure::<dyn FnMut(Event)>::new(move |_: Event| on_event(SseEvent::Open))
        };
        source.set_onopen(Some(open.as_ref().unchecked_ref()));

        let lost = {
            let on_event = on_event.clone();
            let source = source.clone();
            Closure::<dyn FnMut(Event)>::new(move |_: Event| {
                let closed = source.ready_state() == EventSource::CLOSED;
                on_event(SseEvent::Lost { closed })
            })
        };
        source.set_onerror(Some(lost.as_ref().unchecked_ref()));

        Some(SseHandle(SendWrapper::new(Inner {
            source,
            _message: message,
            _named: vec![failure, end],
            _plain: vec![open, lost],
        })))
    }
}

#[cfg(not(feature = "hydrate"))]
mod imp {
    use super::SseEvent;

    pub struct SseHandle;

    pub fn connect(_url: &str, _on_event: impl Fn(SseEvent) + 'static) -> Option<SseHandle> {
        None
    }
}

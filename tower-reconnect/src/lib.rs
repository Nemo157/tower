extern crate futures;
#[macro_use]
extern crate log;
extern crate tower_service;

use futures::{Future, Async, Poll};
use tower_service::{Service, NewService};

use std::{error, fmt};

pub struct Reconnect<T, R>
where T: NewService<R>,
{
    new_service: T,
    state: State<T, R>,
}

#[derive(Debug)]
pub enum Error<T, U> {
    Inner(T),
    Connect(U),
    NotReady,
}

pub struct ResponseFuture<T, R>
where T: NewService<R>
{
    inner: Option<<T::Service as Service<R>>::Future>,
}

enum State<T, R>
where T: NewService<R>
{
    Idle,
    Connecting(T::Future),
    Connected(T::Service),
}

// ===== impl Reconnect =====

impl<T, R> Reconnect<T, R>
where T: NewService<R>,
{
    pub fn new(new_service: T) -> Self {
        Reconnect {
            new_service,
            state: State::Idle,
        }
    }
}

impl<T, R> Service<R> for Reconnect<T, R>
where T: NewService<R>
{
    type Response = T::Response;
    type Error = Error<T::Error, T::InitError>;
    type Future = ResponseFuture<T, R>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        use self::State::*;

        let ret;
        let mut state;

        loop {
            match self.state {
                Idle => {
                    trace!("poll_ready; idle");
                    let fut = self.new_service.new_service();
                    self.state = Connecting(fut);
                    continue;
                }
                Connecting(ref mut f) => {
                    trace!("poll_ready; connecting");
                    match f.poll() {
                        Ok(Async::Ready(service)) => {
                            state = Connected(service);
                        }
                        Ok(Async::NotReady) => {
                            trace!("poll_ready; not ready");
                            return Ok(Async::NotReady);
                        }
                        Err(e) => {
                            trace!("poll_ready; error");
                            state = Idle;
                            ret = Err(Error::Connect(e));
                            break;
                        }
                    }
                }
                Connected(ref mut inner) => {
                    trace!("poll_ready; connected");
                    match inner.poll_ready() {
                        Ok(Async::Ready(_)) => {
                            trace!("poll_ready; ready");
                            return Ok(Async::Ready(()));
                        }
                        Ok(Async::NotReady) => {
                            trace!("poll_ready; not ready");
                            return Ok(Async::NotReady);
                        }
                        Err(_) => {
                            trace!("poll_ready; error");
                            state = Idle;
                        }
                    }
                }
            }

            self.state = state;
        }

        self.state = state;
        ret
    }

    fn call(&mut self, request: R) -> Self::Future {
        use self::State::*;

        trace!("call");

        let service = match self.state {
            Connected(ref mut service) => service,
            _ => return ResponseFuture { inner: None },
        };

        let fut = service.call(request);
        ResponseFuture { inner: Some(fut) }
    }
}

impl<T, R> fmt::Debug for Reconnect<T, R>
where T: NewService<R> + fmt::Debug,
      T::Future: fmt::Debug,
      T::Service: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Reconnect")
            .field("new_service", &self.new_service)
            .field("state", &self.state)
            .finish()
    }
}

// Manual impl of Debug for State because we should be able to
// format a state regardless of whether or not the request type
// is debug.
impl<T, R> fmt::Debug for State<T, R>
where T: NewService<R> + fmt::Debug,
      T::Future: fmt::Debug,
      T::Service: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            State::Idle => fmt.pad("State::Idle"),
            State::Connecting(ref f) => write!(fmt, "State::Connecting({:?})", f),
            State::Connected(ref t) => write!(fmt, "State::Connected({:?})", t),
        }

    }
}


// ===== impl ResponseFuture =====

impl<T: NewService<R>, R> Future for ResponseFuture<T, R> {
    type Item = T::Response;
    type Error = Error<T::Error, T::InitError>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        trace!("poll response");

        match self.inner {
            Some(ref mut f) => {
                f.poll().map_err(Error::Inner)
            }
            None => Err(Error::NotReady),
        }
    }
}


// ===== impl Error =====

impl<T, U> fmt::Display for Error<T, U>
where
    T: fmt::Display,
    U: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Inner(ref why) => fmt::Display::fmt(why, f),
            Error::Connect(ref why) => write!(f, "connection failed: {}", why),
            Error::NotReady => f.pad("not ready"),
        }
    }
}

impl<T, U> error::Error for Error<T, U>
where
    T: error::Error,
    U: error::Error,
{
    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Inner(ref why) => Some(why),
            Error::Connect(ref why) => Some(why),
            _ => None,
        }
    }

    fn description(&self) -> &str {
        match *self {
            Error::Inner(_) => "inner service error",
            Error::Connect(_) => "connection failed",
            Error::NotReady => "not ready",
        }
    }
}

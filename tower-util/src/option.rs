//! Contains `OptionService` and related types and functions.
//!
//! See `OptionService` documentation for more details.
//!
use futures::{Future, Poll};
use tower_service::Service;
use std::marker::PhantomData;

/// Optionally forwards requests to an inner service.
///
/// If the inner service is `None`, `Error::None` is returned as the response.
pub struct OptionService<T, R> {
    inner: Option<T>,
    _req: PhantomData<fn() -> R>,
}

/// Response future returned by `OptionService`.
pub struct ResponseFuture<T> {
    inner: Option<T>,
}

/// Error produced by `OptionService` responding to a request.
#[derive(Debug)]
pub enum Error<T> {
    Inner(T),
    None,
}

// ===== impl OptionService =====

impl<T, R> OptionService<T, R> {
    /// Returns an `OptionService` that forwards requests to `inner`.
    pub fn some(inner: T) -> Self {
        OptionService {
            inner: Some(inner),
            _req: PhantomData,
        }
    }

    /// Returns an `OptionService` that responds to all requests with
    /// `Error::None`.
    pub fn none() -> Self {
        OptionService {
            inner: None,
            _req: PhantomData,
        }
    }
}

impl<T, R> Service<R> for OptionService<T, R>
where T: Service<R>,
{
    type Response = T::Response;
    type Error = Error<T::Error>;
    type Future = ResponseFuture<T::Future>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        match self.inner {
            Some(ref mut inner) => inner.poll_ready().map_err(Error::Inner),
            // None services are always ready
            None => Ok(().into()),
        }
    }

    fn call(&mut self, request: R) -> Self::Future {
        let inner = self.inner.as_mut().map(|i| i.call(request));
        ResponseFuture { inner }
    }
}

// ===== impl ResponseFuture =====

impl<T> Future for ResponseFuture<T>
where T: Future,
{
    type Item = T::Item;
    type Error = Error<T::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner {
            Some(ref mut inner) => inner.poll().map_err(Error::Inner),
            None => Err(Error::None),
        }
    }
}

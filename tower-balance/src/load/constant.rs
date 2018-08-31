use futures::{Async, Poll};
use tower_discover::{Change, Discover};
use tower_service::Service;

use Load;

use std::marker::PhantomData;

/// Wraps a type so that `Load::load` returns a constant value.
pub struct Constant<T, M, R> {
    inner: T,
    load: M,
    _req: PhantomData<fn() -> R>,
}

// ===== impl Constant =====

impl<T, M: Copy, R> Constant<T, M, R> {
    pub fn new(inner: T, load: M) -> Self {
        Self {
            inner,
            load,
            _req: PhantomData,
        }
    }
}

impl<T, M: Copy, R> Load for Constant<T, M, R> {
    type Metric = M;

    fn load(&self) -> M {
        self.load
    }
}

impl<S: Service<R>, M: Copy, R> Service<R> for Constant<S, M, R> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.inner.poll_ready()
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.inner.call(req)
    }
}

/// Proxies `Discover` such that all changes are wrapped with a constant load.
impl<D: Discover<R>, M: Copy, R> Discover<R> for Constant<D, M, R> {
    type Key = D::Key;
    type Response = D::Response;
    type Error = D::Error;
    type Service = Constant<D::Service, M, R>;
    type DiscoverError = D::DiscoverError;

    /// Yields the next discovery change set.
    fn poll(&mut self) -> Poll<Change<D::Key, Self::Service>, D::DiscoverError> {
        use self::Change::*;

        let change = match try_ready!(self.inner.poll()) {
            Insert(k, svc) => Insert(k, Constant::new(svc, self.load)),
            Remove(k) => Remove(k),
        };

        Ok(Async::Ready(change))
    }
}

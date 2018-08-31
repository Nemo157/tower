use futures::IntoFuture;
use tower_service::{Service, NewService};
use std::marker::PhantomData;

/// A `NewService` implemented by a closure.
pub struct NewServiceFn<T, R> {
    f: T,
    _req: PhantomData<fn() -> R>,
}

// ===== impl NewServiceFn =====

impl<T, N, R> NewServiceFn<T, R>
where T: Fn() -> N,
      N: Service<R>,
{
    /// Returns a new `NewServiceFn` with the given closure.
    pub fn new(f: T) -> Self {
        NewServiceFn {
            f,
            _req: PhantomData,
        }
    }
}

impl<T, R, S, Q> NewService<Q> for NewServiceFn<T, Q>
where T: Fn() -> R,
      R: IntoFuture<Item = S>,
      S: Service<Q>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Service = R::Item;
    type InitError = R::Error;
    type Future = R::Future;

    fn new_service(&self) -> Self::Future {
        (self.f)().into_future()
    }
}

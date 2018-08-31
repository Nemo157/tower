use futures::IntoFuture;
use tower_service::{Service, NewService};

/// A `NewService` implemented by a closure.
pub struct NewServiceFn<T> {
    f: T,
}

// ===== impl NewServiceFn =====

impl<T, N> NewServiceFn<T>
where T: Fn() -> N,
{
    /// Returns a new `NewServiceFn` with the given closure.
    pub fn new(f: T) -> Self {
        NewServiceFn {
            f,
        }
    }
}

impl<T, R, S, Q> NewService<Q> for NewServiceFn<T>
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

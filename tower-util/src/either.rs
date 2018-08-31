//! Contains `EitherService` and related types and functions.
//!
//! See `EitherService` documentation for more details.

use futures::Poll;
use futures::future::Either;
use tower_service::Service;
use std::marker::PhantomData;

/// Combine two different service types into a single type.
///
/// Both services must be of the same request, response, and error types.
/// `EitherService` is useful for handling conditional branching in service
/// middleware to different inner service types.
pub struct EitherService<A, B, R> {
    inner: EitherServiceInner<A, B>,
    _req: PhantomData<fn() -> R>,
}

enum EitherServiceInner<A, B> {
    A(A),
    B(B),
}
impl<A, B, R> EitherService<A, B, R>
where
    A: Service<R>,
    B: Service<R,
            Response = A::Response,
                Error = A::Error>,
{
    pub fn a(a: A) -> Self {
        EitherService {
            inner: EitherServiceInner::A(a),
            _req: PhantomData,
        }
    }

    pub fn b(b: B) -> Self {
        EitherService {
            inner: EitherServiceInner::B(b),
            _req: PhantomData,
        }
    }

}

impl<A, B, R> Service<R> for EitherService<A, B, R>
where A: Service<R>,
      B: Service<R,
                Response = A::Response,
                   Error = A::Error>,
{
    type Response = A::Response;
    type Error = A::Error;
    type Future = Either<A::Future, B::Future>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        use self::EitherServiceInner::*;

        match self.inner {
            A(ref mut service) => service.poll_ready(),
            B(ref mut service) => service.poll_ready(),
        }
    }

    fn call(&mut self, request: R) -> Self::Future {
        use self::EitherServiceInner::*;

        match self.inner {
            A(ref mut service) => Either::A(service.call(request)),
            B(ref mut service) => Either::B(service.call(request)),
        }
    }
}

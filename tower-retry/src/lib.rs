#![feature(existential_type, pin, futures_api, async_await, await_macro)]

extern crate futures;
extern crate futures03;
extern crate futures03_test;
extern crate tower_service;

use futures::{Future, Poll};
use futures03::compat::Future01CompatExt;
use futures03::future::{FutureExt, TryFutureExt};
use futures03_test::task::PanicSpawner;
use tower_service::Service;

#[derive(Clone, Debug)]
pub struct Retry<P, S> {
    policy: P,
    service: S,
}

pub trait Policy<Req, Res, E>: Sized {
    type Future: Future<Item = Self, Error = ()>;
    fn retry(&self, req: &Req, res: Result<&Res, &E>) -> Option<Self::Future>;
    fn clone_request(&self, req: &Req) -> Option<Req>;
}

// ===== impl Retry =====

impl<P, S> Retry<P, S>
where
    P: Policy<S::Request, S::Response, S::Error> + Clone,
    S: Service + Clone,
{
    pub fn new(policy: P, service: S) -> Self {
        Retry { policy, service }
    }
}

existential type RetryFuture<P, S: Service>: Future<Item = S::Response, Error = S::Error>;

async fn retry<P, S>(
    mut retry: Retry<P, S>,
    mut request: S::Request,
) -> Result<S::Response, S::Error>
where
    P: Policy<S::Request, S::Response, S::Error> + Clone,
    S: Service + Clone,
{
    loop {
        let req = retry.policy.clone_request(&request);
        let result = await!(retry.service.call(request).compat());

        request = match req {
            Some(req) => req,
            None => {
                // request wasn't cloned, so no way to retry it
                return result;
            }
        };

        let check = match retry.policy.retry(&request, result.as_ref()) {
            Some(check) => check,
            None => return result,
        };

        match await!(check.compat()) {
            Ok(policy) => {
                retry.policy = policy;
            }
            Err(()) => {
                // if Policy::retry() fails, return the original
                // result...
                return result;
            }
        }

        await!(futures::future::poll_fn(|| retry.poll_ready()).compat());
    }
}

impl<P, S> Service for Retry<P, S>
where
    P: Policy<S::Request, S::Response, S::Error> + Clone,
    S: Service + Clone,
{
    type Request = S::Request;
    type Response = S::Response;
    type Error = S::Error;
    type Future = RetryFuture<P, S>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, request: Self::Request) -> Self::Future {
        retry(self.clone(), request)
            .boxed()
            .compat(PanicSpawner::new())
    }
}

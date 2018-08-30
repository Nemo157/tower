#[macro_use]
extern crate futures;
extern crate tower_service;

use futures::{Async, Future, Poll};
use tower_service::Service;

use std::marker::PhantomData;

#[derive(Debug)]
pub struct Retry<P, S, R> {
    policy: P,
    service: S,
    _req: PhantomData<fn() -> R>,
}

#[derive(Debug)]
pub struct ResponseFuture<P: Policy<R, S::Response, S::Error>, S: Service<R>, R> {
    request: Option<R>,
    retry: Retry<P, S, R>,
    state: State<S::Future, P::Future, S::Response, S::Error>,
}

#[derive(Debug)]
enum State<F, P, R, E> {
    /// Polling the future from `Service::call`
    Called(F),
    /// Polling the future from `Policy::retry`
    Checking(P, Option<Result<R, E>>),
    /// Polling `Service::poll_ready` after `Checking` was OK.
    Retrying,
}

pub trait Policy<Req, Res, E>: Sized {
    type Future: Future<Item=Self, Error=()>;
    fn retry(&self, req: &Req, res: Result<&Res, &E>) -> Option<Self::Future>;
    fn clone_request(&self, req: &Req) -> Option<Req>;
}


// ===== impl Retry =====

impl<P, S, R> Retry<P, S, R>
where
    P: Policy<R, S::Response, S::Error> + Clone,
    S: Service<R> + Clone,
{
    pub fn new(policy: P, service: S) -> Self {
        Retry {
            policy,
            service,
            _req: PhantomData,
        }
    }
}

impl<P, S, R> Service<R> for Retry<P, S, R>
where
    P: Policy<R, S::Response, S::Error> + Clone,
    S: Service<R> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<P, S, R>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, request: R) -> Self::Future {
        let cloned = self.policy.clone_request(&request);
        let future = self.service.call(request);
        ResponseFuture {
            request: cloned,
            retry: self.clone(),
            state: State::Called(future),
        }
    }
}

// Manual impl of Clone for Retry since the Request type need not be clone.

impl<P, S, R> Clone for Retry<P, S, R>
where
    P: Policy<R, S::Response, S::Error> + Clone,
    S: Service<R> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            policy: self.policy.clone(),
            service: self.service.clone(),
            _req: PhantomData,
        }
    }
}

// ===== impl ResponseFuture =====

impl<P, S, R> Future for ResponseFuture<P, S, R>
where
    P: Policy<R, S::Response, S::Error> + Clone,
    S: Service<R> + Clone,
{
    type Item = S::Response;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let next = match self.state {
                State::Called(ref mut future) => {
                    let result = match future.poll() {
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Ok(Async::Ready(res)) => Ok(res),
                        Err(err) => Err(err),
                    };

                    if let Some(ref req) = self.request {
                        match self.retry.policy.retry(req, result.as_ref()) {
                            Some(checking) => State::Checking(checking, Some(result)),
                            None => return result.map(Async::Ready),
                        }
                    } else {
                        // request wasn't cloned, so no way to retry it
                        return result.map(Async::Ready);
                    }
                },
                State::Checking(ref mut future, ref mut result) => {
                    let policy = match future.poll() {
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Ok(Async::Ready(policy)) => policy,
                        Err(()) => {
                            // if Policy::retry() fails, return the original
                            // result...
                            return result
                                .take()
                                .expect("polled after complete")
                                .map(Async::Ready);
                        }
                    };
                    self.retry.policy = policy;
                    State::Retrying
                },
                State::Retrying => {
                    try_ready!(self.retry.poll_ready());
                    let req = self
                        .request
                        .take()
                        .expect("retrying requires cloned request");
                    self.request = self.retry.policy.clone_request(&req);
                    State::Called(self.retry.service.call(req))
                }
            };
            self.state = next;
        }
    }
}


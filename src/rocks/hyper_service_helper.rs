#![allow(dead_code)]

use tokio::prelude::future::FutureResult;
use hyper::service::{MakeService,Service};
use hyper::body::Payload;
use tokio::prelude::IntoFuture;
use hyper::{Request,Response};
use std::marker::PhantomData;
use std::error::Error;

pub fn service_fn_once_clone<F,Ret,ReqBody,ResBody>(f: F) -> impl Service
where
    F: Clone + FnOnce(Request<ReqBody>) -> Ret,
    Ret: IntoFuture<Item=Response<ResBody>>,
    Ret::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ReqBody: Payload,
    ResBody: Payload
{
    struct ServiceFnOnceClone<F,ReqBody,Ret>(F,PhantomData<ReqBody>,PhantomData<Ret>);
    impl<F,Ret,ReqBody,ResBody> Service for ServiceFnOnceClone<F,ReqBody,Ret>
    where
        F: Clone + FnOnce(Request<ReqBody>) -> Ret,
        Ret: IntoFuture<Item=Response<ResBody>>,
        Ret::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        ReqBody: Payload,
        ResBody: Payload
    {
        type ReqBody = ReqBody;
        type ResBody = ResBody;
        type Error = Ret::Error;
        type Future = <Ret as IntoFuture>::Future;
        fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
            (self.0.clone())(req).into_future()
        }
    }
    ServiceFnOnceClone(f,PhantomData,PhantomData)
}
pub struct ServiceFnMut<F, R> {
    f: F,
    _req: PhantomData<fn(R)>,
}
impl<F, ReqBody, Ret, ResBody> Service for ServiceFnMut<F, ReqBody>
where
    F: FnMut(Request<ReqBody>) -> Ret,
    ReqBody: Payload,
    Ret: IntoFuture<Item=Response<ResBody>>,
    Ret::Error: Into<Box<dyn Error + Send + Sync>>,
    ResBody: Payload,
{
    type ReqBody = ReqBody;
    type ResBody = ResBody;
    type Error = Ret::Error;
    type Future = Ret::Future;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        (self.f)(req).into_future()
    }
}
impl<F, ReqBody> IntoFuture for ServiceFnMut<F, ReqBody> {
    type Future=FutureResult<Self::Item, Self::Error>;
    type Item=Self;
    type Error=!;
    fn into_future(self) -> Self::Future {
        Ok(self).into()
    }
}

pub fn service_fn_mut<F,Ret,ReqBody>(f: F) -> ServiceFnMut<F,ReqBody>
where
    F: FnMut(Request<ReqBody>) -> Ret,
    Ret: IntoFuture,
{
    ServiceFnMut{f,_req:PhantomData}
}
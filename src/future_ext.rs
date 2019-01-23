use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::task::{
    LocalWaker,
    Poll::{self, Pending, Ready},
};

pub fn alt<T1, T2>(
    f1: impl Future<Output = T1>,
    f2: impl Future<Output = T2>,
) -> impl Future<Output = (Option<T1>, Option<T2>)> {
    struct AltFuture<F1, F2>(F1, F2);
    impl<T1, T2, F1, F2> Future for AltFuture<F1, F2>
    where
        F1: Future<Output = T1>,
        F2: Future<Output = T2>,
    {
        type Output = (Option<T1>, Option<T2>);
        fn poll(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
            let this = unsafe {
                let this = self.get_unchecked_mut();
                (
                    Pin::new_unchecked(&mut this.0),
                    Pin::new_unchecked(&mut this.1),
                )
            };
            let mut pending = true;
            let v1 = if let Ready(v1) = this.0.poll(lw) {
                pending = false;
                Some(v1)
            } else { None };
            let v2 = if let Ready(v2) = this.1.poll(lw) {
                pending = false;
                Some(v2)
            } else { None};
            if pending { Pending } else { Ready((v1, v2)) }
        }
    }
    AltFuture(f1, f2)
}

pub fn join<T1, T2>(
    f1: impl Future<Output = T1>,
    f2: impl Future<Output = T2>,
) -> impl Future<Output = (T1, T2)> {
    struct JoinFuture<T1, T2, F1, F2>(F1, F2, Option<T1>, Option<T2>);
    impl<T1, T2, F1, F2> Future for JoinFuture<T1, T2, F1, F2>
    where
        F1: Future<Output = T1>,
        F2: Future<Output = T2>,
    {
        type Output = (T1, T2);
        fn poll(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
            let this = unsafe {
                let this = self.get_unchecked_mut();
                (
                    Pin::new_unchecked(&mut this.0),
                    Pin::new_unchecked(&mut this.1),
                    &mut this.2,
                    &mut this.3,
                )
            };
            match (&this.2, &this.3) {
                (Some(_), Some(_)) => unreachable!(),
                (Some(_), _) => match this.1.poll(lw) {
                    Ready(v2) => Ready((this.2.take().unwrap(), v2)),
                    _ => Pending,
                },
                (_, Some(_)) => match this.0.poll(lw) {
                    Ready(v1) => Ready((v1, this.3.take().unwrap())),
                    _ => Pending,
                },
                _ => match (this.0.poll(lw), this.1.poll(lw)) {
                    (Ready(v1), Ready(v2)) => Ready((v1, v2)),
                    (Ready(v1), _) => {
                        *this.2 = Some(v1);
                        Pending
                    }
                    (_, Ready(v2)) => {
                        *this.3 = Some(v2);
                        Pending
                    }
                    _ => Pending,
                },
            }
        }
    }
    JoinFuture(f1, f2, None, None)
}

#[allow(dead_code)]
pub fn join_on_ok<T1, T2, E1, E2>(
    f1: impl Future<Output = Result<T1, E1>>,
    f2: impl Future<Output = Result<T2, E2>>,
) -> impl Future<Output = (Option<Result<T1,E1>>,Option<Result<T2,E2>>)> {
    struct JoinFuture<T1, T2, F1, F2>(F1, F2, Option<T1>, Option<T2>);
    impl<T1, T2, E1, E2, F1, F2> Future for JoinFuture<T1, T2, F1, F2>
    where
        F1: Future<Output = Result<T1, E1>>,
        F2: Future<Output = Result<T2, E2>>,
    {
        type Output = (Option<Result<T1,E1>>,Option<Result<T2,E2>>);
        fn poll(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
            let this = unsafe {
                let this = self.get_unchecked_mut();
                (
                    Pin::new_unchecked(&mut this.0),
                    Pin::new_unchecked(&mut this.1),
                    &mut this.2,
                    &mut this.3,
                )
            };
            match (&this.2, &this.3) {
                (Some(_), Some(_)) => unreachable!(),
                (Some(_), _) => match this.1.poll(lw) {
                    Ready(v2) => Ready((Some(Ok(this.2.take().unwrap())), Some(v2))),
                    _ => Pending,
                },
                (_, Some(_)) => match this.0.poll(lw) {
                    Ready(v1) => Ready((Some(v1), Some(Ok(this.3.take().unwrap())))),
                    _ => Pending,
                },
                _ => match (this.0.poll(lw), this.1.poll(lw)) {
                    (Ready(v1), Ready(v2)) => Ready((Some(v1), Some(v2))),
                    (Ready(Ok(v1)), Pending) => {
                        *this.2 = Some(v1);
                        Pending
                    },
                    (Ready(Err(e1)), _) => Ready((Some(Err(e1)), None)),
                    (Pending, Ready(Ok(v2))) => {
                        *this.3 = Some(v2);
                        Pending
                    },
                    (_, Ready(Err(e2))) => Ready((None, Some(Err(e2)))),
                    _ => Pending,
                },
            }
        }
    }
    JoinFuture(f1, f2, None, None)
}

pub async fn wrap_future<T, E>(f: impl Future<Output = Result<T, E>>, prefix: impl AsRef<str>) -> T
where
    T: Default,
    E: Display,
{
    await!(f)
        .map_err(|e| error!("{} error: {}", prefix.as_ref(), e))
        .unwrap_or(T::default())
}

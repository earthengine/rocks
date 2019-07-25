
use std::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::future::Future;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{
    Poll::{self, Pending, Ready}, Context,
};

#[allow(dead_code)]
pub fn drop_guard(f: impl Unpin + FnOnce()) -> impl Drop {
    struct DropGuard<F>(MaybeUninit<F>)
    where
        F: FnOnce();
    impl<F> Drop for DropGuard<F>
    where
        F: FnOnce(),
    {
        fn drop(&mut self) {
            let f = std::mem::replace(&mut self.0, MaybeUninit::uninit());
            (unsafe { f.assume_init() })()
        }
    }
    DropGuard(MaybeUninit::new(f))
}

#[allow(dead_code)]
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
        fn poll(self: Pin<&mut Self>, ct: &mut Context) -> Poll<Self::Output> {
            let this = unsafe {
                let this = self.get_unchecked_mut();
                (
                    Pin::new_unchecked(&mut this.0),
                    Pin::new_unchecked(&mut this.1),
                )
            };
            match (this.0.poll(ct), this.1.poll(ct)) {
                (Pending, Pending) => Pending,
                (Pending, Ready(v2)) => Ready((None, Some(v2))),
                (Ready(v1), Pending) => Ready((Some(v1), None)),
                (Ready(v1), Ready(v2)) => Ready((Some(v1), Some(v2))),
            }
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
        fn poll(self: Pin<&mut Self>, ct: &mut Context) -> Poll<Self::Output> {
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
                (Some(_), _) => match this.1.poll(ct) {
                    Ready(v2) => Ready((this.2.take().unwrap(), v2)),
                    _ => Pending,
                },
                (_, Some(_)) => match this.0.poll(ct) {
                    Ready(v1) => Ready((v1, this.3.take().unwrap())),
                    _ => Pending,
                },
                _ => match (this.0.poll(ct), this.1.poll(ct)) {
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
) -> impl Future<Output = (Option<Result<T1, E1>>, Option<Result<T2, E2>>)> {
    struct JoinFuture<T1, T2, F1, F2>(F1, F2, Option<T1>, Option<T2>);
    impl<T1, T2, E1, E2, F1, F2> Future for JoinFuture<T1, T2, F1, F2>
    where
        F1: Future<Output = Result<T1, E1>>,
        F2: Future<Output = Result<T2, E2>>,
    {
        type Output = (Option<Result<T1, E1>>, Option<Result<T2, E2>>);
        fn poll(self: Pin<&mut Self>, ct: &mut Context) -> Poll<Self::Output> {
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
                (Some(_), _) => match this.1.poll(ct) {
                    Ready(v2) => Ready((Some(Ok(this.2.take().unwrap())), Some(v2))),
                    _ => Pending,
                },
                (_, Some(_)) => match this.0.poll(ct) {
                    Ready(v1) => Ready((Some(v1), Some(Ok(this.3.take().unwrap())))),
                    _ => Pending,
                },
                _ => match (this.0.poll(ct), this.1.poll(ct)) {
                    (Ready(v1), Ready(v2)) => Ready((Some(v1), Some(v2))),
                    (Ready(Ok(v1)), Pending) => {
                        *this.2 = Some(v1);
                        Pending
                    }
                    (Ready(Err(e1)), _) => Ready((Some(Err(e1)), None)),
                    (Pending, Ready(Ok(v2))) => {
                        *this.3 = Some(v2);
                        Pending
                    }
                    (_, Ready(Err(e2))) => Ready((None, Some(Err(e2)))),
                    _ => Pending,
                },
            }
        }
    }
    JoinFuture(f1, f2, None, None)
}

#[derive(Clone)]
pub struct FlagFuture {
    flag: Arc<AtomicBool>
}
impl Future for FlagFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _:&mut Context) -> Poll<()> {
        if self.is_set() {
            Ready(())
        } else {
            Pending
        }
    }
}
impl Default for FlagFuture {
    fn default() -> Self {
        Self { flag: Arc::new(AtomicBool::new(false)) }
    }
}
impl FlagFuture {
    pub fn set(&self) {
        self.flag.store(true, Relaxed)
    }
    pub fn is_set(&self) -> bool {
        self.flag.load(Relaxed)
    }
}
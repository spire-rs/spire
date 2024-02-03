// use core::fmt;
// use std::convert::Infallible;
// use std::marker::PhantomData;
// use std::num::NonZeroUsize;
//
// use crate::extract::FromContextParts;
// use crate::handler::HandlerContext;
//
// mod sealed {
//     pub trait AccessType {}
//
//     impl AccessType for super::ReadOnly {}
//
//     impl AccessType for super::WriteOnly {}
// }
//
// pub struct ReadOnly;
//
// pub struct WriteOnly;
//
// /// Notice: Reading from the [`TaskQueue`] removes tasks without notification.
// /// TODO: Best practice is to allow [`Collector`]..
// pub struct TaskQueue<T = WriteOnly>
// where
//     T: sealed::AccessType,
// {
//     marker: PhantomData<T>,
// }
//
// impl<T> TaskQueue<T> where T: sealed::AccessType {}
//
// impl TaskQueue<WriteOnly> {
//     pub async fn write(&self, task: ()) {
//         todo!()
//     }
//
//     pub async fn write_many<I, U>(&self, tasks: I)
//     where
//         I: Iterator<Item = U>,
//         U: Into<()>,
//     {
//         todo!()
//     }
// }
//
// impl TaskQueue<ReadOnly> {
//     pub async fn read_one(&self) -> Option<()> {
//         todo!()
//     }
//
//     pub async fn read_many(&self, len: NonZeroUsize) -> Vec<()> {
//         todo!()
//     }
//
//     pub async fn notify(&self) {
//         todo!()
//     }
// }
//
// // TODO: Impl iterator for TaskQueue<ReadOnly>
//
// impl<T> fmt::Debug for TaskQueue<T>
// where
//     T: sealed::AccessType,
// {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("TaskQueue").finish_non_exhaustive()
//     }
// }
//
// #[async_trait::async_trait]
// impl<S> FromContextParts<S> for TaskQueue<WriteOnly>
// where
//     T: sealed::AccessType,
// {
//     type Rejection = Infallible;
//
//     async fn from_context_parts(cx: &HandlerContext, _state: &S) -> Result<Self, Self::Rejection> {
//         todo!()
//     }
// }
//
// #[async_trait::async_trait]
// impl<S> FromContextParts<S> for TaskQueue<ReadOnly>
// where
//     T: sealed::AccessType,
// {
//     type Rejection = Infallible;
//
//     async fn from_context_parts(cx: &HandlerContext, _state: &S) -> Result<Self, Self::Rejection> {
//         todo!()
//     }
// }

use std::marker::PhantomData;

mod sealed {
    pub trait AccessType {}

    impl AccessType for super::ReadOnly {}

    impl AccessType for super::WriteOnly {}
}

pub struct ReadOnly;
pub struct WriteOnly;

pub struct TaskQueue {}

impl TaskQueue {
    pub async fn add<T>(&self, task: T) where T: Into<()> {
        todo!()
    }
}

pub struct DataQueue<T, A = WriteOnly> {
    types: PhantomData<T>,
    marker: PhantomData<A>
}

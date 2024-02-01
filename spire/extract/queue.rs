use core::fmt;
use std::marker::PhantomData;

mod sealed {
    pub trait AccessType {}

    impl AccessType for super::ReadOnly {}

    impl AccessType for super::WriteOnly {}
}

pub struct ReadOnly;

pub struct WriteOnly;

/// Notice: Reading from the [`TaskQueue`] removes tasks.
pub struct TaskQueue<T = WriteOnly>
where
    T: sealed::AccessType,
{
    marker: PhantomData<T>,
}

impl<T> TaskQueue<T> where T: sealed::AccessType {}

impl TaskQueue<WriteOnly> {}

impl TaskQueue<ReadOnly> {
    // pub async fn read(&self) {}
    pub async fn notify(&self) {}
}

// TODO: Impl iterator for TaskQueue<ReadOnly>

impl<T> fmt::Debug for TaskQueue<T>
where
    T: sealed::AccessType,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskQueue").finish_non_exhaustive()
    }
}

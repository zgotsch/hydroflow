pub mod null;
pub mod tee;
pub mod vector;

pub use null::NullHandoff;
pub use tee::TeeingHandoff;
pub use vector::VecHandoff;

pub trait TryCanReceive<T> {
    fn try_give(&mut self, item: T) -> Result<T, T>;
}
pub trait CanReceive<T> {
    fn give(&mut self, item: T) -> T;
}

/**
 * A handle onto the metadata part of a [Handoff], with no element type.
 */
pub trait HandoffMeta {
    // TODO(justin): more fine-grained info here.
    fn is_bottom(&self) -> bool;
}

pub trait Handoff: Default + HandoffMeta {
    type Inner;

    fn take_inner(&mut self) -> Self::Inner;

    fn give<T>(&mut self, item: T) -> T
    where
        Self: CanReceive<T>,
    {
        <Self as CanReceive<T>>::give(self, item)
    }

    fn try_give<T>(&mut self, item: T) -> Result<T, T>
    where
        Self: TryCanReceive<T>,
    {
        <Self as TryCanReceive<T>>::try_give(self, item)
    }
}

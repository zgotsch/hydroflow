use hydroflow_macros::LatticeRepr;

use crate::lattice::{Lattice, LatticeRepr, Merge};

#[allow(dead_code)]
#[derive(LatticeRepr)]
pub struct MyLatRepr<L: 'static, Lr, Lr2>
where
    L: Lattice,
    Lr: LatticeRepr<Lattice = L> + Merge<Lr2>,
    Lr2: LatticeRepr<Lattice = L>,
{
    null: crate::lattice::null::NullRepr,
    generic: Lr,

    #[lr_ignore]
    _phantom: std::marker::PhantomData<Lr2>
}

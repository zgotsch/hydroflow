use hydroflow_macros::LatticeRepr;

use crate::lattice::{LatticeRepr, Merge};

#[allow(dead_code)]
#[derive(LatticeRepr)]
pub struct MyLatRepr<Lr, Lr2>
where
    Lr: LatticeRepr + Merge<Lr2>,
    Lr2: LatticeRepr<Lattice = Lr::Lattice>,
{
    null: crate::lattice::null::NullRepr,
    generic: Lr,
}

macro_rules! where_clause {
    ($ty:ty) => {
        pub struct WhereClause<T>
        where T: $ty {
            my_t: T,
        }
    }
}

where_clause!(Copy + Clone);

#[macro_export]
macro_rules! LatticeRepr {
    (
        $struct_lat_vis:vis  struct $struct_lat_name:ident;
        $struct_repr_vis:vis struct $struct_repr_name:ident;

        $struct_vis:vis struct $struct_name:ident $(<$($struct_generic:ident),*>)?
        $(
            where
            $(
                $generic:ident : $generic_constraint:ty,
            )*
        )?
        {
            $(
                $field_vis:vis $field:ident : $field_type:path,
            )*
        }
    ) => {
        $struct_vis struct $struct_name $(<$($struct_generic),*>)?
        $( where $($generic : $generic_constraint,)*)?
        {
            _phantom: std::marker::PhantomData<($($($struct_generic,)*)?)>,
        }

        $struct_lat_vis struct $struct_lat_name $(<$($struct_generic),*>)?
        $( where $($generic : $generic_constraint,)*)?
        {
            _phantom: std::marker::PhantomData<($($($struct_generic,)*)?)>,
        }
        impl $(<$($struct_generic),*>)? $crate::lattice::Lattice
            for $struct_lat_name $(<$($struct_generic),*>)?
        $( where $($generic : $generic_constraint,)*)?
        {}

        #[derive(Clone)]
        $struct_repr_vis struct $struct_repr_name $(<$($struct_generic),*>)?
        $( where $($generic : $generic_constraint,)*)?
        {
            $(
                $field_vis $field : <$field_type as $crate::lattice::LatticeRepr>::Repr,
            )*
        }

        impl $(<$($struct_generic),*>)? $crate::lattice::LatticeRepr
            for $struct_name $(<$($struct_generic),*>)?
        $( where $($generic : $generic_constraint,)*)?
        {
            type Lattice = $struct_lat_name  $(<$($struct_generic),*>)?;
            type Repr    = $struct_repr_name $(<$($struct_generic),*>)?;
        }
    }
}

LatticeRepr! {
    pub struct DebugOpLattice;
    pub struct DebugOp;

    pub struct DebugOpLatRepr<Lr>
    where
        Lr: crate::lattice::LatticeRepr + Clone,
    {
        pub thing_a: crate::lattice::null::NullRepr,
        pub thing_b: Lr,
    }
}

macro_rules! import_type_or_make_dummy {
    (
        types {$path:path} :: (
            $( // Instruction name, base path part and root repetition block
                $src_name:ident // The struct name to import
                $(as $dst_name:ident)? // The name to reexport as
                $(< $($lt:lifetime),+ $(,)? >)? // Matches, optionally, a separator comma, and a
                                                  // comma-separated lifetime list within angle
                                                  // brackets, with an optional trailing comma
            ),+
            , // Mandatory trailing comma for the repitition list, or else teh compiler complains
              // for some reason
        ),
        cfg($pred:meta) // A cfg(...) predicate
        $(,)? // Optional trailing comma for the whole macro
    ) => {$(
        import_type_or_make_dummy!(
            type
            {$path}::$src_name
            $(as $dst_name)?
            $(< $($lt),+ >)?,
            cfg($pred),
        );
    )+};
    (
        type // Instruction name
        {$path:path}::$src_name:ident // The path to import, and the struct name
        as $dst_name:ident // The name to reexport as
        $(< $($lt:lifetime),+ $(,)? >)?, // From this point onwards, the same stuff as above
        cfg($pred:meta)
        $(,)?
    ) => {
        #[cfg($pred)]
        pub use $path::{$src_name as $dst_name};
        #[cfg(not($pred))]
        #[derive(Copy, Clone, Debug, Default)]
        pub struct $dst_name // Expands the struct name
            $(<$($lt),+>)? // Expands the lifetimes as declaration of generic parameters
            ($($(::core::marker::PhantomData<& $lt ()>),+)?); // Creates a tuple of PhantomData,
                                                              // one per lifetime
    };
    (
        // Matches the same stuff, but without the `as`
        type
        {$path:path}::$name:ident
        $(< $($lt:lifetime),+ $(,)? >)?,
        cfg($pred:meta)
        $(,)?
    ) => {
        import_type_or_make_dummy!(type {$path}::$name as $name $(< $($lt),+ >)?, cfg($pred));
    };
}

macro_rules! import_type_alias_or_make_dummy {
    (
        types {$path:path} :: ( // Instruction name and base path part
            $( // Root repetition block
                $src_name:ident // The type alias name to import
                $(as $dst_name:ident)? // The type alias name to reexport as
                = $dummy:ty // The fallback re-definition
                $(,)? // Per-type-alias optional trailing comma
            ),+
            , // Mandatory trailing comma, because an optional one breaks everything for some reason
        ),
        cfg($pred:meta) // A cfg(...) predicate
        $(,)? // Optional trailing comma for whole macro
    ) => {$(
        import_type_alias_or_make_dummy!(
            type {$path}::$src_name $(as $dst_name)? = $dummy,
            cfg($pred),
        );
    )+};
    (
        type // Instruction name
        {$path:path}::$src_name:ident // Path and type alias name to export
        as $dst_name:ident // The same stuff as in the repeating case, basically
        = $dummy:ty,
        cfg($pred:meta)
        $(,)?
    ) => {
        #[cfg($pred)]
        pub(super) use $path::{$src_name as $dst_name};
        #[cfg(not($pred))]
        pub(super) type $dst_name = $dummy;
    };
    (
        type // Same as above, but without the `as` part
        {$path:path}::$name:ident
        = $dummy:ty,
        cfg($pred:meta)
        $(,)?
    ) => {
        import_type_alias_or_make_dummy!(type {$path}::$name as $name = $dummy, cfg($pred));
    };
}

macro_rules! import_const_or_make_dummy {
    (
        $ty:ty: consts {$path:path} :: ( // Instruction name, constant type and base path part
            $( // Root repetition block
                $src_name:ident // The constant name to import
                $(as $dst_name:ident)? // The constant name to reexport as
                = $dummy:expr // The fallback re-definition
                $(,)? // Per-constant optional trailing comma
            ),+
            , // Mandatory trailing comma, because an optional one breaks everything for some reason
        ),
        cfg($pred:meta) // A cfg(...) predicate
        $(,)? // Optional trailing comma for whole macro
    ) => {$(
        import_const_or_make_dummy!(
            $ty: const {$path}::$src_name $(as $dst_name)? = $dummy,
            cfg($pred),
        );
    )+};
    (
        $ty:ty: const // Instruction name and constant type
        {$path:path}::$src_name:ident // Path and constant name to export
        as $dst_name:ident // The same stuff as in the repeating case, basically
        = $dummy:expr,
        cfg($pred:meta)
        $(,)?
    ) => {
        #[cfg($pred)]
        pub(super) use $path::{$src_name as $dst_name};
        #[cfg(not($pred))]
        pub(super) const $dst_name: $ty = $dummy;
    };
    (
        $ty:ty: const // Same as above, but without the `as` part
        {$path:path}::$name:ident
        = $dummy:expr,
        cfg($pred:meta)
        $(,)?
    ) => {
        import_const_or_make_dummy!($ty: const {$path}::$name as $name = $dummy, cfg($pred));
    };
}

macro_rules! import_trait_or_make_dummy {
    (
        traits {$path:path} :: ( // Instruction name and base path part
            $( // Root repetition block
                $(($unsafety:tt))? $src_name:ident // The trait name to import
                $(as $dst_name:ident)? // The trait name to reexport as
            ),+
            , // Mandatory trailing comma for the repetition, the compiler complains otherwise
        ),
        cfg($pred:meta) // A cfg(...) predicate
        $(,)? // Optional trailing comma for the whole macro
    ) => {$(
        import_trait_or_make_dummy!(
            $(($unsafety))? trait {$path}::$src_name $(as $dst_name)?,
            cfg($pred),
        );
    )+};
    (
        $(($unsafety:tt))? trait // Instruction name, and whether the trait is unsafe
        {$path:path}::$src_name:ident // Path and type alias name to export
        as $dst_name:ident, // The same stuff as in the repeating case, basically
        cfg($pred:meta)
        $(,)?
    ) => {
        #[cfg($pred)]
        pub use $path::{$src_name as $dst_name};
        #[cfg(not($pred))]
        pub $($unsafety)? trait $dst_name {}
    };
    (
        $(($unsafety:tt))? trait // Same as above, but without the `as` part
        {$path:path}::$name:ident,
        cfg($pred:meta)
        $(,)?
    ) => {
        import_trait_or_make_dummy!($(($unsafety))? trait {$path}::$name as $name, cfg($pred));
    };
}

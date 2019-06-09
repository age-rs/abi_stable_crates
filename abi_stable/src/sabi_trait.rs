pub mod reexports{

    pub use std::{
        ops::{Deref as __DerefTrait,DerefMut as __DerefMutTrait},
    };

    pub use crate::marker_type::ErasedObject as __ErasedObject;


    pub mod __sabi_re{
        pub use abi_stable::{
            erased_types::{
                DynTrait,
                GetVtable,
                traits::InterfaceFor,
            },
            pointer_trait::{TransmuteElement,OwnedPointer},
            prefix_type::{PrefixTypeTrait,WithMetadata},
            traits::IntoInner,
            sabi_types::{StaticRef,MovePtr},
            sabi_trait::{
                robject::{
                    RObject,
                },
                vtable::{GetVTable,RObjectVtable,GetRObjectVTable},
                for_generated_code::{sabi_from_ref,sabi_from_mut},
            },
            std_types::RBox,
            utils::{transmute_reference,transmute_mut_reference,take_manuallydrop},
        };

        pub use core_extensions::{
            utils::transmute_ignore_size,
            TypeIdentity,
        };

        pub use std::{
            marker::PhantomData,
            mem::ManuallyDrop,
            ptr,
        };
    }
}

pub mod prelude{
    pub use crate::type_level::unerasability::{TU_Unerasable,TU_Opaque};
}

pub mod for_generated_code;
pub mod robject;
pub mod vtable;

use std::{
    fmt::Debug,
    marker::PhantomData,
};

use self::{
    reexports::{
        *,
        __sabi_re::*,
    },
    vtable::BaseVtable,
};

use crate::{
    abi_stability::Tag,
    erased_types::{c_functions,InterfaceType},
    marker_type::ErasedObject,
    type_level::bools::{True,False,Boolean},
    sabi_types::MaybeCmp,
    std_types::Tuple2,
};
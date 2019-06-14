/*!
Contains the `DynTrait` type,and related traits/type aliases.
*/

use std::{
    fmt::{self,Write as fmtWrite},
    io,
    ops::DerefMut,
    marker::PhantomData,
    mem::ManuallyDrop,
    ptr,
    rc::Rc,
};

use serde::{de, ser, Deserialize, Deserializer};

#[allow(unused_imports)]
use core_extensions::{prelude::*, ResultLike};

use crate::{
    abi_stability::SharedStableAbi,
    pointer_trait::{
        StableDeref, TransmuteElement,OwnedPointer,
        GetPointerKind,PK_SmartPointer,PK_Reference,
    },
    marker_type::{ErasedObject,UnsafeIgnoredType}, 
    sabi_types::{StaticRef,MovePtr},
    std_types::{RBox, RCow, RStr,RVec,RIoError},
    type_level::unerasability::{TU_Unerasable,TU_Opaque},
};

#[allow(unused_imports)]
use crate::std_types::Tuple2;

use super::*;
use super::{
    c_functions::adapt_std_fmt,
    trait_objects::*,
    vtable::{GetVtable, VTable},
    traits::InterfaceFor,
};


#[cfg(all(test,not(feature="only_new_tests")))]
mod tests;

mod priv_ {
    use super::*;


    /**

DynTrait implements ffi-safe trait objects,for a selection of traits.

# Passing opaque values around with `DynTrait<_>`

One can pass non-StableAbi types around by using type erasure,using this type.

It generally looks like `DynTrait<'borrow,Pointer<()>,Interface>`,where:

- `'borrow` is the borrow that the type that was erased had.

- `Pointer` is some `pointer_trait::StableDeref` pointer type.

- `Interface` is an `InterfaceType`,which describes what traits are 
    required when constructing the `DynTrait<_>` and which ones it implements.

`trait InterfaceType` allows describing which traits are required 
when constructing a `DynTrait<_>`,and which ones it implements.

<h3> Construction </h3>

To construct a `DynTrait<_>` one can use these associated functions:
    
- from_value:
    Can be constructed from the value directly.
    Requires a value that implements ImplType.
    
- from_ptr:
    Can be constructed from a pointer of a value.
    Requires a value that implements ImplType.
    
- from_any_value:
    Can be constructed from the value directly.Requires a `'static` value.
    
- from_any_ptr
    Can be constructed from a pointer of a value.Requires a `'static` value.

- from_borrowing_value:
    Can be constructed from the value directly.Cannot unerase the DynTrait afterwards.
    
- from_borrowing_ptr
    Can be constructed from a pointer of a value.Cannot unerase the DynTrait afterwards.

DynTrait uses the impls of the value in methods,
which means that the pointer itself does not have to implement those traits,

<h3> Trait object </h3>

`DynTrait<'borrow,Pointer<()>,Interface>` 
can be used as a trait object for any combination of 
the traits listed bellow.

These are the traits:

- Send

- Sync

- Iterator

- DoubleEndedIterator

- std::fmt::Write

- std::io::Write

- std::io::Seek

- std::io::Read

- std::io::BufRead

- Clone 

- Display 

- Debug 

- Default: Can be called as an inherent method.

- Eq 

- PartialEq 

- Ord 

- PartialOrd 

- Hash 

- serde::Deserialize:
    first deserializes from a string,and then calls the objects' Deserialize impl.

- serde::Serialize:
    first calls the objects' Deserialize impl,then serializes that as a string.

<h3> Deconstruction </h3>

`DynTrait<_>` can then be unwrapped into a concrete type,
within the same dynamic library/executable that constructed it,
using these (fallible) conversion methods:

- sabi_into_unerased:
    Unwraps into a pointer to `T`.
    Where `DynTrait<P<()>,Interface>`'s 
        Interface must equal `<T as ImplType>::Interface`

- sabi_as_unerased:
    Unwraps into a `&T`.
    Where `DynTrait<P<()>,Interface>`'s 
        Interface must equal `<T as ImplType>::Interface`

- sabi_as_unerased_mut:
    Unwraps into a `&mut T`.
    Where `DynTrait<P<()>,Interface>`'s 
        Interface must equal `<T as ImplType>::Interface`

- sabi_into_any_unerased:Unwraps into a pointer to `T`.Requires `T:'static`.

- sabi_as_any_unerased:Unwraps into a `&T`.Requires `T:'static`.

- sabi_as_any_unerased_mut:Unwraps into a `&mut T`.Requires `T:'static`.


`DynTrait` cannot be converted back if it was created 
using `DynTrait::from_borrowing_*`.

# Passing DynTrait between dynamic libraries

Passing DynTrait between dynamic libraries 
(as in between the dynamic libraries directly loaded by the same binary/dynamic library)
may cause the program to panic at runtime with an error message stating that 
the trait is not implemented for the specific interface.

This can only happen if you are passing DynTrait between dynamic libraries,
or if DynTrait was instantiated in the parent passed to a child,
a DynTrait instantiated in a child dynamic library passed to the parent
should not cause a panic,it would be a bug.

```text
        binary
  _________|___________
lib0      lib1      lib2
  |         |         |
lib00    lib10      lib20
```

In this diagram passing a DynTrait constructed in lib00 to anything other than 
the binary or lib0 will cause the panic to happen if:

- The InterfaceType requires extra traits in the version of the Interface
    that lib1 and lib2 know about (that the binary does not require).

- lib1 or lib2 attempt to call methods that require the traits that were added 
    to the InterfaceType,in versions of that interface that only they know about.






# Examples

<h3> In the Readme </h3>

The primary example using `DynTrait<_>` is in the readme.

Readme is in 
[the repository for this crate](https://github.com/rodrimati1992/abi_stable_crates),
[crates.io](https://crates.io/crates/abi_stable),
[lib.rs](https://lib.rs/crates/abi_stable).

<h3> Comparing DynTraits </h3>

This is only possible if the erased types don't contain borrows,
and they are not constructed using `DynTrait::from_borrowing_*` methods.

DynTraits wrapping different pointer types can be compared with each other,
it simply uses the values' implementation of PartialEq.

```
use abi_stable::{
    DynTrait,
    erased_types::interfaces::PartialEqInterface,
    std_types::RArc,
};

{
    let left:DynTrait<'static,&(),PartialEqInterface>=
        DynTrait::from_any_ptr(&100,PartialEqInterface);
    
    let mut n100=100;
    let right:DynTrait<'static,&mut (),PartialEqInterface>=
        DynTrait::from_any_ptr(&mut n100,PartialEqInterface);

    assert_eq!(left,right);
}
{
    let left=
        DynTrait::from_any_value(200,PartialEqInterface);

    let right=
        DynTrait::from_any_ptr(RArc::new(200),PartialEqInterface);

    assert_eq!(left,right);
}

```

<h3> Writing to a DynTrait </h3>

This is an example of using the `write!()` macro with DynTrait.

```
use abi_stable::{
    DynTrait,
    erased_types::interfaces::FmtWriteInterface,
};

use std::fmt::Write;

let mut buffer=String::new();

let mut wrapped:DynTrait<'static,&mut (),FmtWriteInterface>=
    DynTrait::from_any_ptr(&mut buffer,FmtWriteInterface);

write!(wrapped,"Foo").unwrap();
write!(wrapped,"Bar").unwrap();
write!(wrapped,"Baz").unwrap();

drop(wrapped);

assert_eq!(&buffer[..],"FooBarBaz");


```


<h3> Iteration </h3>

Using `DynTrait` as an `Iterator` and `DoubleEndedIterator`.

```
use abi_stable::{
    DynTrait,
    erased_types::interfaces::DEIteratorInterface,
};

let mut wrapped=DynTrait::from_any_value(0..=10,DEIteratorInterface::NEW);

assert_eq!(
    wrapped.by_ref().take(5).collect::<Vec<_>>(),
    vec![0,1,2,3,4]
);

assert_eq!(
    wrapped.rev().collect::<Vec<_>>(),
    vec![10,9,8,7,6,5]
);


```


# Making pointers compatible with DynTrait

To make pointers compatible with DynTrait,they must imlement the 
`abi_stable::pointer_trait::{GetPointerKind,StableDeref,TransmuteElement}` traits 
as shown in the example.

`GetPointerKind` should generally be implemented with `type Kind=PK_SmartPointer`.
The exception is in the case that it is a `#[repr(transparent)]`
wrapper around a `&` or a `&mut`,
in which case it should implement `GetPointerKind<Kind=PK_Reference>` 
or `GetPointerKind<Kind=PK_MutReference>` respectively.

<h3> Example </h3>

This is an example of a newtype wrapping an `RBox<T>`.

```rust 
    
use abi_stable::DynTrait;

fn main(){
    let lines="line0\nline1\nline2";
    let mut iter=NewtypeBox::new(lines.lines());

    // The type annotation here is just to show the type,it's not necessary.
    let mut wrapper:DynTrait<'_,NewtypeBox<()>,IteratorInterface>=
        DynTrait::from_borrowing_ptr(iter,IteratorInterface);

    // You can clone the DynTrait! 
    let clone=wrapper.clone();

    assert_eq!( wrapper.next(), Some("line0") );
    assert_eq!( wrapper.next(), Some("line1") );
    assert_eq!( wrapper.next(), Some("line2") );
    assert_eq!( wrapper.next(), None );

    assert_eq!(
        clone.rev().collect::<Vec<_>>(),
        vec!["line2","line1","line0"],
    )

}


/////////////////////////////////////////

use std::ops::{Deref, DerefMut};

use abi_stable::{
    StableAbi,
    InterfaceType,
    impl_InterfaceType,
    std_types::RBox,
    erased_types::IteratorItem,
    pointer_trait::{
        PK_SmartPointer,GetPointerKind,StableDeref,TransmuteElement
    },
    type_level::bools::True,
};

#[repr(transparent)]
#[derive(Default,Clone,StableAbi)]
pub struct NewtypeBox<T>{
    box_:RBox<T>,
}

impl<T> NewtypeBox<T>{
    pub fn new(value:T)->Self{
        Self{
            box_:RBox::new(value)
        }
    }
}

impl<T> Deref for NewtypeBox<T>{
    type Target=T;

    fn deref(&self)->&T{
        &*self.box_
    }
}

impl<T> DerefMut for NewtypeBox<T>{
    fn deref_mut(&mut self)->&mut T{
        &mut *self.box_
    }
}

unsafe impl<T> GetPointerKind for NewtypeBox<T>{
    type Kind=PK_SmartPointer;
}

unsafe impl<T> StableDeref for NewtypeBox<T> {}

unsafe impl<T,O> TransmuteElement<O> for NewtypeBox<T>
where 
    // Using this to ensure that the pointer is safe to wrap,
    // while this is not necessary for `RBox<T>`,
    // it might be for some other pointer type.
    RBox<T>:TransmuteElement<O,Kind=Self::Kind>
{
    type TransmutedPtr = NewtypeBox<O>;
}

/////////////////////////////////////////

#[repr(C)]
#[derive(StableAbi)]
pub struct IteratorInterface;

impl_InterfaceType!{
    impl InterfaceType for IteratorInterface {
        type Iterator = True;
        type DoubleEndedIterator = True;
        type Clone = True;
        type Debug = True;
    }
}

impl<'a> IteratorItem<'a> for IteratorInterface{
    type Item=&'a str;
}

```

    
    */
    #[repr(C)]
    #[derive(StableAbi)]
    #[sabi(
        // prefix_bound="I:InterfaceBound<'borr>",
        // bound="<I as SharedStableAbi>::StaticEquivalent:InterfaceBound<'static>",
        bound="VTable<'borr,P,I>:SharedStableAbi",
        tag="<I as InterfaceBound<'borr>>::TAG",
    )]
    pub struct DynTrait<'borr,P,I,EV=()> 
    where I:InterfaceBound<'borr>
    {
        pub(super) object: ManuallyDrop<P>,
        vtable: *const VTable<'borr,P,I>,
        extra_vtable:EV,
        _marker:PhantomData<extern fn()->Tuple2<I,RStr<'borr>>>,
        _marker2:UnsafeIgnoredType<Rc<()>>,

    }

    impl DynTrait<'static,&'static (),()> {
        /// Constructs the `DynTrait<_>` from a `T:ImplType`.
        ///
        /// Use this whenever possible instead of `from_any_value`,
        /// because it produces better error messages when unerasing the `DynTrait<_>`
        pub fn from_value<T>(object: T) -> DynTrait<'static,RBox<()>,T::Interface>
        where
            T: ImplType,
            T::Interface:InterfaceBound<'static>,
            T: GetVtable<'static,T,RBox<()>,RBox<T>,<T as ImplType>::Interface>,
        {
            let object = RBox::new(object);
            DynTrait::from_ptr(object)
        }

        /// Constructs the `DynTrait<_>` from a pointer to a `T:ImplType`.
        ///
        /// Use this whenever possible instead of `from_any_ptr`,
        /// because it produces better error messages when unerasing the `DynTrait<_>`
        pub fn from_ptr<P, T>(object: P) -> DynTrait<'static,P::TransmutedPtr,T::Interface>
        where
            T: ImplType,
            T::Interface:InterfaceBound<'static>,
            T: GetVtable<'static,T,P::TransmutedPtr,P,<T as ImplType>::Interface>,
            P: StableDeref<Target = T>+TransmuteElement<()>,
        {
            DynTrait {
                object: unsafe{
                    ManuallyDrop::new(object.transmute_element(<()>::T))
                },
                vtable: T::get_vtable(),
                extra_vtable:(),
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }

        /// Constructs the `DynTrait<_>` from a type that doesn't borrow anything.
        pub fn from_any_value<T,I>(object: T,interface:I) -> DynTrait<'static,RBox<()>,I>
        where
            T:'static,
            I:InterfaceBound<'static>,
            InterfaceFor<T,I,TU_Unerasable> : GetVtable<'static,T,RBox<()>,RBox<T>,I>,
        {
            let object = RBox::new(object);
            DynTrait::from_any_ptr(object,interface)
        }

        /// Constructs the `DynTrait<_>` from a pointer to a 
        /// type that doesn't borrow anything.
        pub fn from_any_ptr<P, T,I>(
            object: P,
            _interface:I
        ) -> DynTrait<'static,P::TransmutedPtr,I>
        where
            I:InterfaceBound<'static>,
            T:'static,
            InterfaceFor<T,I,TU_Unerasable>: GetVtable<'static,T,P::TransmutedPtr,P,I>,
            P: StableDeref<Target = T>+TransmuteElement<()>,
        {
            DynTrait {
                object: unsafe{
                    ManuallyDrop::new(object.transmute_element(<()>::T))
                },
                vtable: <InterfaceFor<T,I,TU_Unerasable>>::get_vtable(),
                extra_vtable:(),
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }
        
        /// Constructs the `DynTrait<_>` from a value with a `'borr` borrow.
        ///
        /// Cannot unerase the DynTrait afterwards.
        pub fn from_borrowing_value<'borr,T,I>(
            object: T,
            interface:I,
        ) -> DynTrait<'borr,RBox<()>,I>
        where
            T:'borr,
            I:InterfaceBound<'borr>,
            InterfaceFor<T,I,TU_Opaque> : GetVtable<'borr,T,RBox<()>,RBox<T>,I>,
        {
            let object = RBox::new(object);
            DynTrait::from_borrowing_ptr(object,interface)
        }

        /// Constructs the `DynTrait<_>` from a pointer to the erased type
        /// with a `'borr` borrow.
        ///
        /// Cannot unerase the DynTrait afterwards.
        pub fn from_borrowing_ptr<'borr,P, T,I>(
            object: P,
            _interface:I
        ) -> DynTrait<'borr,P::TransmutedPtr,I>
        where
            T:'borr,
            I:InterfaceBound<'borr>,
            InterfaceFor<T,I,TU_Opaque>: GetVtable<'borr,T,P::TransmutedPtr,P,I>,
            P: StableDeref<Target = T>+TransmuteElement<()>,
        {
            DynTrait {
                object: unsafe{
                    ManuallyDrop::new(object.transmute_element(<()>::T))
                },
                vtable: <InterfaceFor<T,I,TU_Opaque>>::get_vtable(),
                extra_vtable:(),
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }
    }



    impl<'borr,P,I,EV> DynTrait<'borr,P,I,EV>
    where 
        I: InterfaceBound<'borr>
    {
    /**

Constructs an DynTrait from an erased pointer and an extra vtable.

# Safety

These are the requirements for the caller:

- `P` must be a pointer to the type that `extra_vtable` functions 
    take as the first parameter.

- The vtable must not come from a reborrowed DynTrait
    (created using DynTrait::reborrow or DynTrait::reborrow_mut).

- The vtable must be the `<SomeVTableName>` of a struct declared with 
    `#[derive(StableAbi)]``#[sabi(kind(Prefix(prefix_struct="<SomeVTableName>")))]`.

- The vtable must have `StaticRef<RObjectVtable<..>>` 
    as its first declared field

    */
        pub unsafe fn with_vtable<OrigPtr,Erasability>(
            ptr:OrigPtr,
            extra_vtable:EV,
        )-> DynTrait<'borr,P,I,EV>
        where
            OrigPtr::Target:Sized+'borr,
            I:InterfaceBound<'borr>,
            InterfaceFor<OrigPtr::Target,I,Erasability>: 
                GetVtable<'borr,OrigPtr::Target,P,OrigPtr,I>,
            OrigPtr: TransmuteElement<(),TransmutedPtr=P>+'borr,
            P:StableDeref<Target=()>,
        {
            DynTrait {
                object: unsafe{
                    ManuallyDrop::new(ptr.transmute_element(<()>::T))
                },
                vtable: <InterfaceFor<OrigPtr::Target,I,Erasability>>::get_vtable(),
                extra_vtable,
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }
    }



    impl<P,I,EV> DynTrait<'static,P,I,EV> 
    where 
        I: InterfaceBound<'static>
    {
        /// Allows checking whether 2 `DynTrait<_>`s have a value of the same type.
        ///
        /// Notes:
        ///
        /// - Types from different dynamic libraries/executables are 
        /// never considered equal.
        ///
        /// - `DynTrait`s constructed using `DynTrait::from_borrowing_*`
        /// are never considered to wrap the same type.
        pub fn sabi_is_same_type<Other,I2,EV2>(&self,other:&DynTrait<'static,Other,I2,EV2>)->bool
        where I2:InterfaceBound<'static>
        {
            self.sabi_vtable_address()==other.sabi_vtable_address()||
            self.sabi_vtable().type_info().is_compatible(other.sabi_vtable().type_info())
        }
    }

    impl<'borr,P,I,EV> DynTrait<'borr,P,I,StaticRef<EV>>
    where 
        I: InterfaceBound<'borr>
    {
        /// A vtable used by `#[sabi_trait]` derived trait objects.
        #[inline]
        pub fn sabi_et_vtable<'a>(&self)->&'a EV{
            self.extra_vtable.get()
        }
    }
        
    impl<'borr,P,I,EV> DynTrait<'borr,P,I,EV>
    where 
        I: InterfaceBound<'borr>
    {
        #[inline]
        pub(super) fn sabi_extra_vtable(&self)->EV
        where
            EV:Copy,
        {
            self.extra_vtable
        }

        #[inline]
        pub(super) fn sabi_vtable<'a>(&self) -> &'a VTable<'borr,P,I>{
            unsafe {
                &*(((self.vtable as usize)&PTR_MASK) as *const VTable<'borr,P,I>)
            }
        }

        #[inline]
        pub(super)fn sabi_vtable_address(&self) -> usize {
            (self.vtable as usize)&PTR_MASK
        }

        pub(super)fn sabi_vtable_ptr_flags(&self) -> usize {
            (self.vtable as usize)&PTR_FLAGS
        }

        /// Returns the address of the wrapped object.
        ///
        /// This will not change between calls for the same `DynTrait<_>`.
        pub fn sabi_object_address(&self) -> usize
        where
            P: Deref,
        {
            self.sabi_erased_ref() as *const ErasedObject as usize
        }

        unsafe fn sabi_object_as<T>(&self) -> &T
        where
            P: Deref,
        {
            &*((&**self.object) as *const P::Target as *const T)
        }
        unsafe fn sabi_object_as_mut<T>(&mut self) -> &mut T
        where
            P: DerefMut,
        {
            &mut *((&mut **self.object) as *mut P::Target as *mut T)
        }
        

        pub fn sabi_erased_ref(&self) -> &ErasedObject
        where
            P: Deref,
        {
            unsafe { self.sabi_object_as() }
        }

        #[inline]
        pub fn sabi_erased_mut(&mut self) -> &mut ErasedObject
        where
            P: DerefMut,
        {
            unsafe { self.sabi_object_as_mut() }
        }


        #[inline]
        fn sabi_into_erased_ptr(self)->ManuallyDrop<P>{
            let mut this= ManuallyDrop::new(self);
            unsafe{ ptr::read(&mut this.object) }
        }


        #[inline]
        pub fn sabi_with_value<F,R>(self,f:F)->R
        where 
            P: OwnedPointer<Target=()>,
            F:FnOnce(MovePtr<'_,()>)->R,
        {
            OwnedPointer::with_moved_ptr(self.sabi_into_erased_ptr(),f)
        }


    }


    impl<'borr,P,I,EV> DynTrait<'borr,P,I,EV> 
    where 
        I: InterfaceBound<'borr>
    {
        /// The uid in the vtable has to be the same as the one for T,
        /// otherwise it was not created from that T in the library that declared the opaque type.
        pub(super) fn sabi_check_same_destructor<A,T>(&self) -> Result<(), UneraseError<()>>
        where
            P: TransmuteElement<T>,
            A: GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            let t_vtable:&VTable<'borr,P,I> = A::get_vtable();
            if self.sabi_vtable_address() == t_vtable as *const _ as usize
                || self.sabi_vtable().type_info().is_compatible(t_vtable.type_info())
            {
                Ok(())
            } else {
                Err(UneraseError {
                    dyn_trait:(),
                    expected_vtable_address: t_vtable as *const _ as usize,
                    expected_type_info:t_vtable.type_info(),
                    found_vtable_address: self.vtable as usize,
                    found_type_info:self.sabi_vtable().type_info(),
                })
            }
        }

        /// Unwraps the `DynTrait<_>` into a pointer of 
        /// the concrete type that it was constructed with.
        ///
        /// T is required to implement ImplType.
        ///
        /// # Errors
        ///
        /// This will return an error in any of these conditions:
        ///
        /// - It is called in a dynamic library/binary outside
        /// the one from which this `DynTrait<_>` was constructed.
        ///
        /// - The DynTrait was constructed using a `from_borrowing_*` method
        ///
        /// - `T` is not the concrete type this `DynTrait<_>` was constructed with.
        ///
        pub fn sabi_into_unerased<T>(self) -> Result<P::TransmutedPtr, UneraseError<Self>>
        where
            P: TransmuteElement<T>,
            P::Target:Sized,
            T: ImplType + GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            check_unerased!(self,self.sabi_check_same_destructor::<T,T>());
            unsafe { 
                let this=ManuallyDrop::new(self);
                Ok(ptr::read(&*this.object).transmute_element(T::T)) 
            }
        }

        /// Unwraps the `DynTrait<_>` into a reference of 
        /// the concrete type that it was constructed with.
        ///
        /// T is required to implement ImplType.
        ///
        /// # Errors
        ///
        /// This will return an error in any of these conditions:
        ///
        /// - It is called in a dynamic library/binary outside
        /// the one from which this `DynTrait<_>` was constructed.
        ///
        /// - The DynTrait was constructed using a `from_borrowing_*` method
        ///
        /// - `T` is not the concrete type this `DynTrait<_>` was constructed with.
        ///
        pub fn sabi_as_unerased<T>(&self) -> Result<&T, UneraseError<&Self>>
        where
            P: Deref + TransmuteElement<T>,
            T: ImplType + GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            check_unerased!(self,self.sabi_check_same_destructor::<T,T>());
            unsafe { Ok(self.sabi_object_as()) }
        }

        /// Unwraps the `DynTrait<_>` into a mutable reference of 
        /// the concrete type that it was constructed with.
        ///
        /// T is required to implement ImplType.
        ///
        /// # Errors
        ///
        /// This will return an error in any of these conditions:
        ///
        /// - It is called in a dynamic library/binary outside
        /// the one from which this `DynTrait<_>` was constructed.
        ///
        /// - The DynTrait was constructed using a `from_borrowing_*` method
        ///
        /// - `T` is not the concrete type this `DynTrait<_>` was constructed with.
        ///
        pub fn sabi_as_unerased_mut<T>(&mut self) -> Result<&mut T, UneraseError<&mut Self>>
        where
            P: DerefMut + TransmuteElement<T>,
            T: ImplType + GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            check_unerased!(self,self.sabi_check_same_destructor::<T,T>());
            unsafe { Ok(self.sabi_object_as_mut()) }
        }


        /// Unwraps the `DynTrait<_>` into a pointer of 
        /// the concrete type that it was constructed with.
        ///
        /// T is required to not borrow anything.
        ///
        /// # Errors
        ///
        /// This will return an error in any of these conditions:
        ///
        /// - It is called in a dynamic library/binary outside
        /// the one from which this `DynTrait<_>` was constructed.
        ///
        /// - The DynTrait was constructed using a `from_borrowing_*` method
        ///
        /// - `T` is not the concrete type this `DynTrait<_>` was constructed with.
        ///
        pub fn sabi_into_any_unerased<T>(self) -> Result<P::TransmutedPtr, UneraseError<Self>>
        where
            T:'static,
            P: TransmuteElement<T>,
            P::Target:Sized,
            Self:DynTraitBound<'borr>,
            InterfaceFor<T,I,TU_Unerasable>: GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            check_unerased!(
                self,
                self.sabi_check_same_destructor::<InterfaceFor<T,I,TU_Unerasable>,T>()
            );
            unsafe {
                unsafe { 
                    let this=ManuallyDrop::new(self);
                    Ok(ptr::read(&*this.object).transmute_element(T::T)) 
                }
            }
        }

        /// Unwraps the `DynTrait<_>` into a reference of 
        /// the concrete type that it was constructed with.
        ///
        /// T is required to not borrow anything.
        ///
        /// # Errors
        ///
        /// This will return an error in any of these conditions:
        ///
        /// - It is called in a dynamic library/binary outside
        /// the one from which this `DynTrait<_>` was constructed.
        ///
        /// - The DynTrait was constructed using a `from_borrowing_*` method
        ///
        /// - `T` is not the concrete type this `DynTrait<_>` was constructed with.
        ///
        pub fn sabi_as_any_unerased<T>(&self) -> Result<&T, UneraseError<&Self>>
        where
            T:'static,
            P: Deref + TransmuteElement<T>,
            Self:DynTraitBound<'borr>,
            InterfaceFor<T,I,TU_Unerasable>: GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            check_unerased!(
                self,
                self.sabi_check_same_destructor::<InterfaceFor<T,I,TU_Unerasable>,T>()
            );
            unsafe { Ok(self.sabi_object_as()) }
        }

        /// Unwraps the `DynTrait<_>` into a mutable reference of 
        /// the concrete type that it was constructed with.
        ///
        /// T is required to not borrow anything.
        ///
        /// # Errors
        ///
        /// This will return an error in any of these conditions:
        ///
        /// - It is called in a dynamic library/binary outside
        /// the one from which this `DynTrait<_>` was constructed.
        ///
        /// - The DynTrait was constructed using a `from_borrowing_*` method
        ///
        /// - `T` is not the concrete type this `DynTrait<_>` was constructed with.
        ///
        pub fn sabi_as_any_unerased_mut<T>(&mut self) -> Result<&mut T, UneraseError<&mut Self>>
        where
            P: DerefMut + TransmuteElement<T>,
            Self:DynTraitBound<'borr>,
            InterfaceFor<T,I,TU_Unerasable>: GetVtable<'borr,T,P,P::TransmutedPtr,I>,
        {
            check_unerased!(
                self,
                self.sabi_check_same_destructor::<InterfaceFor<T,I,TU_Unerasable>,T>()
            );
            unsafe { Ok(self.sabi_object_as_mut()) }
        }

    }


    mod private_struct {
        pub struct PrivStruct;
    }
    use self::private_struct::PrivStruct;

    
    /// This is used to make sure that reborrowing does not change 
    /// the Send-ness or Sync-ness of the pointer.
    pub trait ReborrowBounds<SendNess,SyncNess>{}

    // If it's reborrowing,it must have either both Sync+Send or neither.
    impl ReborrowBounds<False,False> for PrivStruct {}
    impl ReborrowBounds<True ,True > for PrivStruct {}

    
    impl<'borr,P,I,EV> DynTrait<'borr,P,I,EV> 
    where 
        I:InterfaceBound<'borr>
    {
        /// Creates a shared reborrow of this DynTrait.
        ///
        /// The reborrowed DynTrait cannot use these methods:
        /// 
        /// - DynTrait::default
        /// 
        pub fn reborrow<'re>(&'re self)->DynTrait<'borr,&'re (),I,EV> 
        where
            P:Deref<Target=()>,
            PrivStruct:ReborrowBounds<I::Send,I::Sync>,
            EV:Copy,
        {
            // Reborrowing will break if I add extra functions that operate on `P`.
            DynTrait {
                object: ManuallyDrop::new(&**self.object),
                vtable: ((self.vtable as usize) | PTR_FLAG_IS_BORROWED)as *const _,
                extra_vtable:self.sabi_extra_vtable(),
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }

        /// Creates a mutable reborrow of this DynTrait.
        ///
        /// The reborrowed DynTrait cannot use these methods:
        /// 
        /// - DynTrait::default
        /// 
        /// - DynTrait::clone
        /// 
        pub fn reborrow_mut<'re>(&'re mut self)->DynTrait<'borr,&'re mut (),I,EV> 
        where
            P:DerefMut<Target=()>,
            PrivStruct:ReborrowBounds<I::Send,I::Sync>,
            EV:Copy,
        {
            let extra_vtable=self.sabi_extra_vtable();
            // Reborrowing will break if I add extra functions that operate on `P`.
            DynTrait {
                object: ManuallyDrop::new(&mut **self.object),
                vtable: ((self.vtable as usize) | PTR_FLAG_IS_BORROWED)as *const _,
                extra_vtable,
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }
    }


    impl<'borr,P,I,EV> DynTrait<'borr,P,I,EV> 
    where 
        I:InterfaceBound<'borr>+'borr,
        EV:'borr,
    {
        /// Constructs a DynTrait<P,I> with a `P`,using the same vtable.
        /// `P` must come from a function in the vtable,
        /// or come from a copy of `P:Copy+GetPointerKind<Kind=PK_Reference>`,
        /// to ensure that it is compatible with the functions in it.
        pub(super) fn from_new_ptr(&self, object: P,extra_vtable:EV) -> Self {
            Self {
                object:ManuallyDrop::new(object),
                vtable: self.vtable,
                extra_vtable,
                _marker:PhantomData,
                _marker2:UnsafeIgnoredType::DEFAULT,
            }
        }

/**
Constructs a `DynTrait<P,I>` with the default value for `P`.

# Reborrowing

This cannot be called with a reborrowed DynTrait:

```compile_fail
# use abi_stable::{
#     DynTrait,
#     erased_types::interfaces::DefaultInterface,
# };
let object=DynTrait::from_any_value((),DefaultInterface);
let borrow=object.reborrow();
let _=borrow.default();

```

```compile_fail
# use abi_stable::{
#     DynTrait,
#     erased_types::interfaces::DefaultInterface,
# };
let object=DynTrait::from_any_value((),DefaultInterface);
let borrow=object.reborrow_mut();
let _=borrow.default();

```
 */
        pub fn default(&self) -> Self
        where
            P: Deref + GetPointerKind<Kind=PK_SmartPointer>,
            I: InterfaceType<Default = True>,
            EV:Copy,
        {
            let new = self.sabi_vtable().default_ptr()();
            self.from_new_ptr(new,self.sabi_extra_vtable())
        }

        /// It serializes a `DynTrait<_>` into a string by using 
        /// `<ConcreteType as SerializeImplType>::serialize_impl`.
        pub fn serialized<'a>(&'a self) -> Result<RCow<'a, str>, RBoxError>
        where
            P: Deref,
            I: InterfaceType<Serialize = True>,
        {
            self.sabi_vtable().serialize()(self.sabi_erased_ref()).into_result()
        }

        /// Deserializes a string into a `DynTrait<_>`,by using 
        /// `<I as DeserializeOwnedInterface>::deserialize_impl`.
        pub fn deserialize_owned_from_str(s: &str) -> Result<Self, RBoxError>
        where
            P: 'borr+Deref,
            I: DeserializeOwnedInterface<'borr,Deserialize = True, Deserialized = Self>,
        {
            s.piped(RStr::from).piped(I::deserialize_impl)
        }

        /// Deserializes a `&'borr str` into a `DynTrait<'borr,_>`,by using 
        /// `<I as DeserializeBorrowedInterface<'borr>>::deserialize_impl`.
        pub fn deserialize_borrowing_from_str(s: &'borr str) -> Result<Self, RBoxError>
        where
            P: 'borr+Deref,
            I: DeserializeBorrowedInterface<'borr,Deserialize = True, Deserialized = Self>,
        {
            s.piped(RStr::from).piped(I::deserialize_impl)
        }
    }

    impl<'borr,P,I,EV> Drop for DynTrait<'borr,P,I,EV>
    where I:InterfaceBound<'borr>
    {
        fn drop(&mut self){
            unsafe{
                let vtable=self.sabi_vtable();

                if (self.sabi_vtable_ptr_flags()&PTR_FLAG_IS_BORROWED)==PTR_FLAG_IS_BORROWED {
                    // Do nothing
                }else{
                    vtable.drop_ptr()(&mut *self.object);
                }
            }
        }
    }

}


const PTR_FLAGS:usize=0b1111;
const PTR_MASK:usize=!PTR_FLAGS;
const PTR_FLAG_IS_BORROWED:usize=0b_0001;


pub use self::priv_::DynTrait;

//////////////////////



mod clone_impl{
    pub trait CloneImpl<PtrKind>{
        fn clone_impl(&self) -> Self;
    }
}
use self::clone_impl::CloneImpl;


/// This impl is for smart pointers.
impl<'borr,P, I,EV> CloneImpl<PK_SmartPointer> for DynTrait<'borr,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'borr,Clone = True>+'borr,
    EV:Copy+'borr,
{
    fn clone_impl(&self) -> Self {
        let vtable = self.sabi_vtable();
        let new = vtable.clone_ptr()(&*self.object);
        self.from_new_ptr(new,self.sabi_extra_vtable())
    }
}

/// This impl is for references.
impl<'borr,P, I,EV> CloneImpl<PK_Reference> for DynTrait<'borr,P,I,EV>
where
    P: Deref+Copy,
    I: InterfaceBound<'borr,Clone = True>+'borr,
    EV:Copy+'borr,
{
    fn clone_impl(&self) -> Self {
        self.from_new_ptr(*self.object,self.sabi_extra_vtable())
    }
}


/**
Clone is implemented for references and smart pointers,
using `GetPointerKind` to decide whether `P` is a smart pointer or a reference.

DynTrait does not implement Clone if P==`&mut ()` :

```compile_fail
# use abi_stable::{
#     DynTrait,
#     erased_types::interfaces::CloneInterface,
# };

let mut object=DynTrait::from_any_value((),());
let borrow=object.reborrow_mut();
let _=borrow.clone();

```

*/
impl<'borr,P, I,EV> Clone for DynTrait<'borr,P,I,EV>
where
    P: Deref+GetPointerKind,
    I: InterfaceBound<'borr>,
    Self:CloneImpl<<P as GetPointerKind>::Kind>,
{
    fn clone(&self) -> Self {
        self.clone_impl()
    }
}

//////////////////////


impl<'borr,P, I,EV> Display for DynTrait<'borr,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'borr,Display = True>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        adapt_std_fmt::<ErasedObject>(self.sabi_erased_ref(), self.sabi_vtable().display(), f)
    }
}

impl<'borr,P, I,EV> Debug for DynTrait<'borr,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'borr,Debug = True>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        adapt_std_fmt::<ErasedObject>(self.sabi_erased_ref(), self.sabi_vtable().debug(), f)
    }
}

/**
First it serializes a `DynTrait<_>` into a string by using 
<ConcreteType as SerializeImplType>::serialize_impl,
then it serializes the string.

*/
/// ,then it .
impl<'borr,P, I,EV> Serialize for DynTrait<'borr,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'borr,Serialize = True>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.sabi_vtable().serialize()(self.sabi_erased_ref())
            .into_result()
            .map_err(ser::Error::custom)?
            .serialize(serializer)
    }
}

/// First it Deserializes a string,then it deserializes into a 
/// `DynTrait<_>`,by using `<I as DeserializeOwnedInterface>::deserialize_impl`.
impl<'de,'borr:'de, P, I,EV> Deserialize<'de> for DynTrait<'borr,P,I,EV>
where
    EV: 'borr,
    P: Deref+'borr,
    I: InterfaceBound<'borr>+'borr,
    I: DeserializeOwnedInterface<'borr,Deserialize = True, Deserialized = Self>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        I::deserialize_impl(RStr::from(&*s)).map_err(de::Error::custom)
    }
}

impl<P, I,EV> Eq for DynTrait<'static,P,I,EV>
where
    Self: PartialEq,
    P: Deref,
    I: InterfaceBound<'static,Eq = True>,
{
}

impl<P, P2, I,EV,EV2> PartialEq<DynTrait<'static,P2,I,EV2>> for DynTrait<'static,P,I,EV>
where
    P: Deref,
    P2: Deref,
    I: InterfaceBound<'static,PartialEq = True>,
{
    fn eq(&self, other: &DynTrait<'static,P2,I,EV2>) -> bool {
        // unsafe: must check that the vtable is the same,otherwise return a sensible value.
        if !self.sabi_is_same_type(other) {
            return false;
        }

        self.sabi_vtable().partial_eq()(self.sabi_erased_ref(), other.sabi_erased_ref())
    }
}

impl<P, I,EV> Ord for DynTrait<'static,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'static,Ord = True>,
    Self: PartialOrd + Eq,
{
    fn cmp(&self, other: &Self) -> Ordering {
        // unsafe: must check that the vtable is the same,otherwise return a sensible value.
        if !self.sabi_is_same_type(other) {
            return self.sabi_vtable_address().cmp(&other.sabi_vtable_address());
        }

        self.sabi_vtable().cmp()(self.sabi_erased_ref(), other.sabi_erased_ref()).into()
    }
}

impl<P, P2, I,EV,EV2> PartialOrd<DynTrait<'static,P2,I,EV2>> for DynTrait<'static,P,I,EV>
where
    P: Deref,
    P2: Deref,
    I: InterfaceBound<'static,PartialOrd = True>,
    Self: PartialEq<DynTrait<'static,P2,I,EV2>>,
{
    fn partial_cmp(&self, other: &DynTrait<'static,P2,I,EV2>) -> Option<Ordering> {
        // unsafe: must check that the vtable is the same,otherwise return a sensible value.
        if !self.sabi_is_same_type(other) {
            return Some(self.sabi_vtable_address().cmp(&other.sabi_vtable_address()));
        }

        self.sabi_vtable().partial_cmp()(self.sabi_erased_ref(), other.sabi_erased_ref())
            .map(IntoReprRust::into_rust)
            .into()
    }
}

impl<'borr,P, I,EV> Hash for DynTrait<'borr,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'borr,Hash = True>,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.sabi_vtable().hash()(self.sabi_erased_ref(), HasherObject::new(state))
    }
}


//////////////////////////////////////////////////////////////////


impl<'borr,P, I,Item,EV> Iterator for DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,Iterator = True,IteratorItem=Item>,
{
    type Item=Item;

    fn next(&mut self)->Option<Item>{
        let vtable=self.sabi_vtable();
        (vtable.iter().next)(self.sabi_erased_mut()).into_rust()
    }

    fn nth(&mut self,nth:usize)->Option<Item>{
        let vtable=self.sabi_vtable();
        (vtable.iter().nth)(self.sabi_erased_mut(),nth).into_rust()
    }

    fn size_hint(&self)->(usize,Option<usize>){
        let vtable=self.sabi_vtable();
        let tuple=(vtable.iter().size_hint)(self.sabi_erased_ref()).into_rust();
        (tuple.0,tuple.1.into_rust())
    }

    fn count(mut self)->usize{
        let vtable=self.sabi_vtable();
        (vtable.iter().count)(self.sabi_erased_mut())
    }

    fn last(mut self)->Option<Item>{
        let vtable=self.sabi_vtable();
        (vtable.iter().last)(self.sabi_erased_mut()).into_rust()
    }
}


impl<'borr,P, I,Item,EV> DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,Iterator = True,IteratorItem=Item>,
{
/**
Eagerly skips n elements from the iterator.

This method is faster than using `Iterator::skip`.

# Example

```
# use abi_stable::{
#     DynTrait,
#     erased_types::interfaces::IteratorInterface,
#     std_types::RVec,
#     traits::IntoReprC,
# };

let mut iter=0..20;
let mut wrapped=DynTrait::from_any_ptr(&mut iter,IteratorInterface::NEW);

assert_eq!(wrapped.next(),Some(0));

wrapped.skip_eager(2);

assert_eq!(wrapped.next(),Some(3));
assert_eq!(wrapped.next(),Some(4));
assert_eq!(wrapped.next(),Some(5));

wrapped.skip_eager(2);

assert_eq!(wrapped.next(),Some(8));
assert_eq!(wrapped.next(),Some(9));

wrapped.skip_eager(9);

assert_eq!(wrapped.next(),Some(19));
assert_eq!(wrapped.next(),None    );



```


*/
    pub fn skip_eager(&mut self, n: usize){
        let vtable=self.sabi_vtable();
        (vtable.iter().skip_eager)(self.sabi_erased_mut(),n);
    }


/**
Extends the `RVec<Item>` with the `self` Iterator.

Extends `buffer` with as many elements of the iterator as `taking` specifies:

- RNone: Yields all elements.Use this with care,since Iterators can be infinite.

- RSome(n): Yields n elements.

<h3> Example </h3>

```
# use abi_stable::{
#     DynTrait,
#     erased_types::interfaces::IteratorInterface,
#     std_types::{RVec,RSome},
#     traits::IntoReprC,
# };

let mut wrapped=DynTrait::from_any_value(0.. ,IteratorInterface::NEW);

let mut buffer=vec![ 101,102,103 ].into_c();
wrapped.extending_rvec(&mut buffer,RSome(5));
assert_eq!(
    &buffer[..],
    &*vec![101,102,103,0,1,2,3,4]
);

assert_eq!( wrapped.next(),Some(5));
assert_eq!( wrapped.next(),Some(6));
assert_eq!( wrapped.next(),Some(7));

```
*/
    pub fn extending_rvec(&mut self,buffer:&mut RVec<Item>,taking:ROption<usize>){
        let vtable=self.sabi_vtable();
        (vtable.iter().extending_rvec)(self.sabi_erased_mut(),buffer,taking);
    }
}


//////////////////////////////////////////////////////////////////


impl<'borr,P, I,Item,EV> DoubleEndedIterator for DynTrait<'borr,P,I,EV>
where
    Self:Iterator<Item=Item>,
    P: DerefMut,
    I: InterfaceBound<'borr,DoubleEndedIterator = True,IteratorItem=Item>,
{

    fn next_back(&mut self)->Option<Item>{
        let vtable=self.sabi_vtable();
        (vtable.back_iter().next_back)(self.sabi_erased_mut()).into_rust()
    }
}


impl<'borr,P, I,Item,EV> DynTrait<'borr,P,I,EV>
where
    Self:Iterator<Item=Item>,
    P: DerefMut,
    I: InterfaceBound<'borr,DoubleEndedIterator = True,IteratorItem=Item>,
{
    pub fn nth_back_(&mut self,nth:usize)->Option<Item>{
        let vtable=self.sabi_vtable();
        (vtable.back_iter().nth_back)(self.sabi_erased_mut(),nth).into_rust()
    }

/**
Extends the `RVec<Item>` with the back of the `self` DoubleEndedIterator.

Extends `buffer` with as many elements of the iterator as `taking` specifies:

- RNone: Yields all elements.Use this with care,since Iterators can be infinite.

- RSome(n): Yields n elements.

<h3> Example </h3>

```
# use abi_stable::{
#     DynTrait,
#     erased_types::interfaces::DEIteratorInterface,
#     std_types::{RVec,RNone},
#     traits::IntoReprC,
# };

let mut wrapped=DynTrait::from_any_value(0..=3 ,DEIteratorInterface::NEW);

let mut buffer=vec![ 101,102,103 ].into_c();
wrapped.extending_rvec_back(&mut buffer,RNone);
assert_eq!(
    &buffer[..],
    &*vec![101,102,103,3,2,1,0]
)

```

*/
    pub fn extending_rvec_back(&mut self,buffer:&mut RVec<Item>,taking:ROption<usize>){
        let vtable=self.sabi_vtable();
        (vtable.back_iter().extending_rvec_back)(self.sabi_erased_mut(),buffer,taking);
    }
}


//////////////////////////////////////////////////////////////////


impl<'borr,P,I,EV> fmtWrite for DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,FmtWrite = True>,
{
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error>{
        let vtable = self.sabi_vtable();
        match vtable.fmt_write_str()(self.sabi_erased_mut(),s.into()) {
            ROk(_)=>Ok(()),
            RErr(_)=>Err(fmt::Error),
        }
    }
}



//////////////////////////////////////////////////////////////////


#[inline]
fn to_io_result<T,U>(res:RResult<T,RIoError>)->io::Result<U>
where
    T:Into<U>
{
    match res {
        ROk(v)=>Ok(v.into()),
        RErr(e)=>Err(e.into()),
    }
}


/////////////


impl<'borr,P,I,EV> io::Write for DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,IoWrite = True>,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>{
        let vtable = self.sabi_vtable().io_write();

        to_io_result((vtable.write)(self.sabi_erased_mut(),buf.into()))
    }
    fn flush(&mut self) -> io::Result<()>{
        let vtable = self.sabi_vtable().io_write();

        to_io_result((vtable.flush)(self.sabi_erased_mut()))
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let vtable = self.sabi_vtable().io_write();

        to_io_result((vtable.write_all)(self.sabi_erased_mut(),buf.into()))
    }
}


/////////////


impl<'borr,P,I,EV> io::Read for DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,IoRead = True>,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>{
        let vtable = self.sabi_vtable().io_read();

        to_io_result((vtable.read)(self.sabi_erased_mut(),buf.into()))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let vtable = self.sabi_vtable().io_read();

        to_io_result((vtable.read_exact)(self.sabi_erased_mut(),buf.into()))
    }

}


/////////////


impl<'borr,P,I,EV> io::BufRead for DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,IoRead = True,IoBufRead = True>,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]>{
        let vtable = self.sabi_vtable().io_bufread();

        to_io_result((vtable.fill_buf)(self.sabi_erased_mut()))
    }

    fn consume(&mut self, ammount:usize ){
        let vtable = self.sabi_vtable().io_bufread();

        (vtable.consume)(self.sabi_erased_mut(),ammount)
    }

}

/////////////


impl<'borr,P,I,EV> io::Seek for DynTrait<'borr,P,I,EV>
where
    P: DerefMut,
    I: InterfaceBound<'borr,IoSeek = True>,
{
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64>{
        let vtable = self.sabi_vtable();

        to_io_result(vtable.io_seek()(self.sabi_erased_mut(),pos.into()))
    }
}


//////////////////////////////////////////////////////////////////

unsafe impl<'borr,P,I,EV> Send for DynTrait<'borr,P,I,EV>
where
    P: Send,
    I: InterfaceBound<'borr,Send = True>,
{}


unsafe impl<'borr,P,I,EV> Sync for DynTrait<'borr,P,I,EV>
where
    P: Sync,
    I: InterfaceBound<'borr,Sync = True>,
{}


//////////////////////////////////////////////////////////////////

mod sealed {
    use super::*;
    pub trait Sealed {}
    impl<'borr,P,I,EV> Sealed for DynTrait<'borr,P,I,EV> 
    where I:InterfaceBound<'borr>
    {}
}
use self::sealed::Sealed;

/// For accessing the Interface of a `DynTrait<Pointer<()>,Interface>`.
pub trait DynTraitBound<'borr>: Sealed {
    type Interface: InterfaceType;
}

impl<'borr,P, I,EV> DynTraitBound<'borr> for DynTrait<'borr,P,I,EV>
where
    P: Deref,
    I: InterfaceBound<'borr>,
{
    type Interface = I;
}


/// For accessing the `Interface` in a `DynTrait<Pointer<()>,Interface>`.
pub type GetVWInterface<'borr,This>=
    <This as DynTraitBound<'borr>>::Interface;


//////////////////////////////////////////////////////////////////

/// Error for `DynTrait<_>` being unerased into the wrong type
/// with one of the `*unerased*` methods.
#[derive(Copy, Clone)]
pub struct UneraseError<T> {
    dyn_trait:T,
    expected_vtable_address: usize,
    expected_type_info:&'static TypeInfo,
    found_vtable_address: usize,
    found_type_info:&'static TypeInfo,
}


impl<T> UneraseError<T>{
    fn map<F,U>(self,f:F)->UneraseError<U>
    where F:FnOnce(T)->U
    {
        UneraseError{
            dyn_trait              :f(self.dyn_trait),
            expected_vtable_address:self.expected_vtable_address,
            expected_type_info     :self.expected_type_info,
            found_vtable_address   :self.found_vtable_address,
            found_type_info        :self.found_type_info,
        }
    }

    /// Extracts the DynTrait,to handle the failure to unerase it.
    #[must_use]
    pub fn into_inner(self)->T{
        self.dyn_trait
    }
}


impl<D> fmt::Debug for UneraseError<D>{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UneraseError")
            .field("dyn_trait",&"<not shown>")
            .field("expected_vtable_address",&self.expected_vtable_address)
            .field("expected_type_info",&self.expected_type_info)
            .field("found_vtable_address",&self.found_vtable_address)
            .field("found_type_info",&self.found_type_info)
            .finish()
    }
}

impl<D> fmt::Display for UneraseError<D>{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl<D> ::std::error::Error for UneraseError<D> {}

//////////////////////////////////////////////////////////////////
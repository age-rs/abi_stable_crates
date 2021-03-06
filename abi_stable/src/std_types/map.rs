/*!
Contains the ffi-safe equivalent of `std::collections::HashMap`,and related items.
*/

use std::{
    borrow::Borrow,
    collections::{HashMap,hash_map::RandomState},
    cmp::{Eq,PartialEq},
    fmt::{self,Debug},
    hash::{Hash,Hasher,BuildHasher},
    ops::{Index,IndexMut},
    iter::FromIterator,
    ptr::NonNull,
    marker::PhantomData,
    mem,
};

#[allow(unused_imports)]
use core_extensions::prelude::*;

use crate::{
    DynTrait,
    StableAbi,
    marker_type::{ErasedObject,NotCopyNotClone,UnsafeIgnoredType},
    erased_types::trait_objects::HasherObject,
    prefix_type::{PrefixTypeTrait,WithMetadata},
    sabi_types::StaticRef,
    std_types::*,
    traits::{IntoReprRust,ErasedType},
    utils::{transmute_reference,transmute_mut_reference},
};


mod entry;
mod extern_fns;
mod iterator_stuff;
mod map_query;
mod map_key;

#[cfg(all(test,not(feature="only_new_tests")))]
mod test;

use self::{
    map_query::MapQuery,
    map_key::MapKey,
    entry::{BoxedREntry},
};

pub use self::{
    iterator_stuff::{
        RefIterInterface,MutIterInterface,ValIterInterface,
        IntoIter,
    },
    entry::{REntry,ROccupiedEntry,RVacantEntry},
};


/**

An ffi-safe hashmap,which wraps `std::collections::HashMap<K,V,S>`,
only requiring the `K:Eq+Hash` bounds when constructing it.

Most of the API in `HashMap` is implemented here,including the Entry API.


# Example

This example demonstrates how one can use the RHashMap as a dictionary.

```
use abi_stable::std_types::{RHashMap,Tuple2,RString,RSome};

let mut map=RHashMap::new();

map.insert("dictionary","A book/document containing definitions of words");
map.insert("bibliophile","Someone who loves books.");
map.insert("pictograph","A picture representating of a word.");

assert_eq!(
    map["dictionary"],
    "A book/document containing definitions of words",
);

assert_eq!(
    map.remove("bibliophile"),
    RSome("Someone who loves books."),
);

assert_eq!(
    map.get("pictograph"),
    Some(&"A picture representating of a word."),
);

for Tuple2(k,v) in map {
    assert!( k=="dictionary" || k=="pictograph" );

    assert!(
        v=="A book/document containing definitions of words" ||
        v=="A picture representating of a word.",
        "{}=>{}",
        k,v
    );
}


```


*/
#[derive(StableAbi)]
#[repr(C)]
#[sabi(
    // The hasher doesn't matter
    unsafe_unconstrained(S),
)]
pub struct RHashMap<K,V,S=RandomState>{
    map:RBox<ErasedMap<K,V,S>>,
    vtable:StaticRef<VTable<K,V,S>>,
}


///////////////////////////////////////////////////////////////////////////////


struct BoxedHashMap<'a,K,V,S>{
    map:HashMap<MapKey<K>,V,S>,
    entry:Option<BoxedREntry<'a,K,V>>,
}

/// An RHashMap iterator,
/// implementing `Iterator<Item= Tuple2< &K, &V > >+!Send+!Sync+Clone`
pub type Iter<'a,K,V>=
    DynTrait<'a,RBox<()>,RefIterInterface<K,V>>;

/// An RHashMap iterator,
/// implementing `Iterator<Item= Tuple2< &K, &mut V > >+!Send+!Sync`
pub type IterMut<'a,K,V>=
    DynTrait<'a,RBox<()>,MutIterInterface<K,V>>;

/// An RHashMap iterator,
/// implementing `Iterator<Item= Tuple2< K, V > >+!Send+!Sync`
pub type Drain<'a,K,V>=
    DynTrait<'a,RBox<()>,ValIterInterface<K,V>>;


/// Used as the erased type of the RHashMap type.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(
    // The hasher doesn't matter
    unsafe_unconstrained(S),
)]
struct ErasedMap<K,V,S>(
    PhantomData<Tuple2<K,V>>,
    UnsafeIgnoredType<S>
);

unsafe impl<'a,K:'a,V:'a,S:'a> ErasedType<'a> for ErasedMap<K,V,S> {
    type Unerased=BoxedHashMap<'a,K,V,S>;
}


///////////////////////////////////////////////////////////////////////////////


impl<K,V> RHashMap<K,V,RandomState>{
    /// Constructs an empty RHashMap.
    /// 
    /// # Example
    /// 
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    ///
    /// let mut map=RHashMap::<RString,u32>::new();
    /// assert!(map.is_empty());
    /// map.insert("Hello".into(),10);
    /// assert_eq!(map.is_empty(),false);
    /// 
    /// ```
    #[inline]
    pub fn new()->RHashMap<K,V>
    where 
        Self:Default
    {
        Self::default()
    }

    /// Constructs an empty RHashMap with at least the passed capacity.
    /// 
    /// # Example
    /// 
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    ///
    /// let mut map=RHashMap::<RString,u32>::with_capacity(10);
    /// assert!(map.capacity()>=10);
    /// 
    /// ```
    #[inline]
    pub fn with_capacity(capacity:usize)->RHashMap<K,V>
    where 
        Self:Default
    {
        let mut this=Self::default();
        this.reserve(capacity);
        this
    }
}


impl<K,V,S> RHashMap<K,V,S>{
    /// Constructs an empty RHashMap with the passed `hash_builder` to hash the keys.
    /// 
    /// # Example
    /// 
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map=RHashMap::<RString,u32,_>::with_hasher(s);
    /// assert!(map.is_empty());
    /// map.insert("Hello".into(),10);
    /// assert_eq!(map.is_empty(),false);
    /// 
    /// ```
    #[inline]
    pub fn with_hasher(hash_builder: S) -> RHashMap<K, V, S> 
    where
        K:Eq+Hash,
        S:BuildHasher+Default,
    {
        Self::with_capacity_and_hasher(0,hash_builder)
    }
    /// Constructs an empty RHashMap with at least the passed capacity,
    /// and the passed `hash_builder` to hash the keys.
    /// 
    /// # Example
    /// 
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map=RHashMap::<RString,u32,_>::with_capacity_and_hasher(10,s);
    /// assert!(map.capacity()>=10);
    /// 
    /// ```
    pub fn with_capacity_and_hasher(
        capacity: usize,
        hash_builder: S
    ) -> RHashMap<K, V, S> 
    where
        K:Eq+Hash,
        S:BuildHasher+Default,
    {
        let mut map=VTable::<K,V,S>::erased_map(hash_builder);
        map.reserve(capacity);
        RHashMap{
            map,
            vtable:WithMetadata::as_prefix(VTable::VTABLE_REF),
        }
    }
}


impl<K,V,S> RHashMap<K,V,S>{

    fn vtable<'a>(&self)->&'a VTable<K,V,S>{
        self.vtable.get()
    }

}


impl<K,V,S> RHashMap<K,V,S>{
    /// Returns whether the map associates a value with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    ///
    /// let mut map=RHashMap::<RString,u32>::new();
    /// assert_eq!(map.contains_key("boo"),false);
    /// map.insert("boo".into(),0);
    /// assert_eq!(map.contains_key("boo"),true);
    ///
    /// ```
    pub fn contains_key<Q>(&self,query:&Q)->bool
    where
        K:Borrow<Q>,
        Q:Hash+Eq+?Sized
    {
        self.get(query).is_some()
    }

    /// Returns a reference to the value associated with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    ///
    /// let mut map=RHashMap::<RString,u32>::new();
    /// assert_eq!(map.get("boo"), None);
    /// map.insert("boo".into(),0);
    /// assert_eq!(map.get("boo"), Some(&0));
    ///
    /// ```
    pub fn get<Q>(&self,query:&Q)->Option<&V>
    where
        K:Borrow<Q>,
        Q:Hash+Eq+?Sized
    {
        let vtable=self.vtable();
        unsafe{
            vtable.get_elem()(&*self.map,MapQuery::new(&query))
        }
    }

    /// Returns a mutable reference to the value associated with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RString};
    ///
    /// let mut map=RHashMap::<RString,u32>::new();
    /// assert_eq!(map.get_mut("boo"), None);
    /// map.insert("boo".into(),0);
    /// assert_eq!(map.get_mut("boo"), Some(&mut 0));
    ///
    /// ```
    pub fn get_mut<Q>(&mut self,query:&Q)->Option<&mut V>
    where
        K:Borrow<Q>,
        Q:Hash+Eq+?Sized
    {
        let vtable=self.vtable();
        unsafe{
            vtable.get_mut_elem()(&mut *self.map,MapQuery::new(&query))
        }
    }

    /// Removes the value associated with the key.
    /// Returns a mutable reference to the value associated with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RSome,RNone};
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.remove(&0), RSome(1));
    /// assert_eq!(map.remove(&0), RNone);
    ///
    /// assert_eq!(map.remove(&3), RSome(4));
    /// assert_eq!(map.remove(&3), RNone);
    ///
    /// ```
    pub fn remove<Q>(&mut self,query:&Q)->ROption<V>
    where
        K:Borrow<Q>,
        Q:Hash+Eq+?Sized
    {
        self.remove_entry(query).map(|x| x.1 )
    }

    /// Removes the entry for the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RSome,RNone,Tuple2};
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.remove_entry(&0), RSome(Tuple2(0,1)));
    /// assert_eq!(map.remove_entry(&0), RNone);
    ///
    /// assert_eq!(map.remove_entry(&3), RSome(Tuple2(3,4)));
    /// assert_eq!(map.remove_entry(&3), RNone);
    ///
    /// ```
    pub fn remove_entry<Q>(&mut self,query:&Q)->ROption<Tuple2<K,V>>
    where
        K:Borrow<Q>,
        Q:Hash+Eq+?Sized
    {
        let vtable=self.vtable();
        vtable.remove_entry()(&mut *self.map,MapQuery::new(&query))
    }
}


impl<K,V,S> RHashMap<K,V,S>{
    /// Returns whether the map associates a value with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    /// assert_eq!(map.contains_key(&11),false);
    /// map.insert(11,0);
    /// assert_eq!(map.contains_key(&11),true);
    ///
    /// ```
    pub fn contains_key_p(&self,key:&K)->bool{
        self.get_p(key).is_some()
    }

    /// Returns a reference to the value associated with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    /// assert_eq!(map.get(&12), None);
    /// map.insert(12,0);
    /// assert_eq!(map.get(&12), Some(&0));
    ///
    /// ```
    pub fn get_p(&self,key:&K)->Option<&V>{
        let vtable=self.vtable();
        unsafe{
            vtable.get_elem_p()(&*self.map,&key)
        }
    }

    /// Returns a mutable reference to the value associated with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    /// assert_eq!(map.get_mut(&12), None);
    /// map.insert(12,0);
    /// assert_eq!(map.get_mut(&12), Some(&mut 0));
    ///
    /// ```
    pub fn get_mut_p(&mut self,key:&K)->Option<&mut V>{
        let vtable=self.vtable();
        unsafe{
            vtable.get_mut_elem_p()(&mut *self.map,&key)
        }
    }

    /// Removes the value associated with the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RSome,RNone};
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.remove_p(&0), RSome(1));
    /// assert_eq!(map.remove_p(&0), RNone);
    ///
    /// assert_eq!(map.remove_p(&3), RSome(4));
    /// assert_eq!(map.remove_p(&3), RNone);
    ///
    /// ```
    pub fn remove_p(&mut self,key:&K)->ROption<V>{
        self.remove_entry_p(key).map(|x| x.1 )
    }

    /// Removes the entry for the key.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,RSome,RNone,Tuple2};
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.remove_entry_p(&0), RSome(Tuple2(0,1)));
    /// assert_eq!(map.remove_entry_p(&0), RNone);
    ///
    /// assert_eq!(map.remove_entry_p(&3), RSome(Tuple2(3,4)));
    /// assert_eq!(map.remove_entry_p(&3), RNone);
    ///
    /// ```
    pub fn remove_entry_p(&mut self,key:&K)->ROption<Tuple2<K,V>>{
        let vtable=self.vtable();
        vtable.remove_entry_p()(&mut *self.map,&key)
    }

    /// Returns a reference to the value associated with the key.
    ///
    /// # Panics
    ///
    /// Panics if the key is not associated with a value.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.index_p(&0),&1);
    /// assert_eq!(map.index_p(&3),&4);
    ///
    /// ```
    ///
    /// ```should_panic
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// assert_eq!(map.index_p(&0),&1);
    ///
    /// ```
    pub fn index_p(&self,key:&K)->&V{
        self.get_p(key).expect("no entry in RHashMap<_,_> found for key")
    }

    /// Returns a mutable reference to the value associated with the key.
    ///
    /// # Panics
    ///
    /// Panics if the key is not associated with a value.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.index_mut_p(&0),&mut 1);
    /// assert_eq!(map.index_mut_p(&3),&mut 4);
    ///
    /// ```
    ///
    /// ```should_panic
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// assert_eq!(map.index_mut_p(&0),&mut 1);
    ///
    /// ```
    pub fn index_mut_p(&mut self,key:&K)->&mut V{
        self.get_mut_p(key).expect("no entry in RHashMap<_,_> found for key")
    }

    //////////////////////////////////

    /// Inserts a value into the map,associating it with a key,returning the previous value.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// map.insert(0,1);
    /// map.insert(2,3);
    ///
    /// assert_eq!(map[&0],1);
    /// assert_eq!(map[&2],3);
    ///
    /// ```
    pub fn insert(&mut self,key:K,value:V)->ROption<V>{
        let vtable=self.vtable();
        unsafe{
            vtable.insert_elem()(&mut *self.map,key,value)
        }
    }

    /// Reserves enough space to insert `reserved` extra elements without reallocating.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    /// map.reserve(10);
    ///
    /// ```
    pub fn reserve(&mut self,reserved:usize){
        let vtable=self.vtable();

        vtable.reserve()(&mut *self.map,reserved);
    }

    /// Removes all the entries in the map.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=vec![(0,1),(3,4)].into_iter().collect::<RHashMap<u32,u32>>();
    ///
    /// assert_eq!(map.contains_key(&0),true);
    /// assert_eq!(map.contains_key(&3),true);
    ///
    /// map.clear();
    ///
    /// assert_eq!(map.contains_key(&0),false);
    /// assert_eq!(map.contains_key(&3),false);
    ///
    /// ```
    pub fn clear(&mut self){
        let vtable=self.vtable();
        vtable.clear_map()(&mut *self.map);
    }

    /// Returns the ammount of entries in the map.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// assert_eq!(map.len(),0);
    /// map.insert(0,1);
    /// assert_eq!(map.len(),1);
    /// map.insert(2,3);
    /// assert_eq!(map.len(),2);
    ///
    /// ```
    pub fn len(&self)->usize{
        let vtable=self.vtable();
        vtable.len()(&*self.map)
    }

    /// Returns the capacity of the map,the ammount of elements it can store without reallocating.
    ///
    /// Note that this is a lower bound,since hash maps don't necessarily have an exact capacity.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::with_capacity(4);
    ///
    /// assert!(map.capacity()>=4);
    ///
    /// ```
    pub fn capacity(&self)->usize{
        let vtable=self.vtable();
        vtable.capacity()(&*self.map)
    }

    /// Returns whether the map contains any entries.
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::RHashMap;
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// assert_eq!(map.is_empty(),true);
    /// map.insert(0,1);
    /// assert_eq!(map.is_empty(),false);
    ///
    /// ```
    pub fn is_empty(&self)->bool{
        self.len()==0
    }

    /// Iterates over the entries in the map,with references to the values in the map.
    ///
    /// This returns an `Iterator<Item= Tuple2< &K, &V > >+!Send+!Sync+Clone`
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,Tuple2};
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// map.insert(0,1);
    /// map.insert(3,4);
    ///
    /// let mut list=map.iter().collect::<Vec<_>>();
    /// list.sort();
    /// assert_eq!( list, vec![Tuple2(&0,&1),Tuple2(&3,&4)] );
    ///
    /// ```
     pub fn iter    (&self)->Iter<'_,K,V>{
        let vtable=self.vtable();

        vtable.iter()(&*self.map)
    }
    
    /// Iterates over the entries in the map,with mutable references to the values in the map.
    ///
    /// This returns an `Iterator<Item= Tuple2< &K, &mut V > >+!Send+!Sync`
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,Tuple2};
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// map.insert(0,1);
    /// map.insert(3,4);
    ///
    /// let mut list=map.iter_mut().collect::<Vec<_>>();
    /// list.sort();
    /// assert_eq!( list, vec![Tuple2(&0,&mut 1),Tuple2(&3,&mut  4)] );
    ///
    /// ```
    pub fn iter_mut(&mut self)->IterMut<'_,K,V>{
        let vtable=self.vtable();

        vtable.iter_mut()(&mut *self.map)
    }

    /// Clears the map,returning an iterator over all the entries that were removed.
    /// 
    /// This returns an `Iterator<Item= Tuple2< K, V > >+!Send+!Sync`
    ///
    /// # Example
    ///
    /// ```
    /// use abi_stable::std_types::{RHashMap,Tuple2};
    ///
    /// let mut map=RHashMap::<u32,u32>::new();
    ///
    /// map.insert(0,1);
    /// map.insert(3,4);
    ///
    /// let mut list=map.drain().collect::<Vec<_>>();
    /// list.sort();
    /// assert_eq!( list, vec![Tuple2(0,1),Tuple2(3,4)] );
    ///
    /// assert!(map.is_empty());
    ///
    /// ```
    pub fn drain   (&mut self)->Drain<'_,K,V>{
        let vtable=self.vtable();

        vtable.drain()(&mut *self.map)
    }

/**
Gets a handle into the entry in the map for the key,
that allows operating directly on the entry.

# Example

```
use abi_stable::std_types::RHashMap;

let mut map=RHashMap::<u32,u32>::new();

// Inserting an entry that wasn't there before.
{
    let mut entry=map.entry(0);
    assert_eq!(entry.get(),None);
    assert_eq!(entry.or_insert(3),&mut 3);
    assert_eq!(map.get(&0),Some(&3));
}


```
*/
    pub fn entry(&mut self,key:K)->REntry<'_,K,V>{
        let vtable=self.vtable();

        vtable.entry()(&mut *self.map,key)
    }
}


/// This returns an `Iterator<Item= Tuple2< K, V > >+!Send+!Sync`
impl<K,V,S> IntoIterator for RHashMap<K,V,S>{
    type Item=Tuple2<K,V>;
    type IntoIter=IntoIter<K,V>;
    
    fn into_iter(self)->IntoIter<K,V>{
        let vtable=self.vtable();

        vtable.iter_val()(self.map)
    }
}


/// This returns an `Iterator<Item= Tuple2< &K, &V > >+!Send+!Sync+Clone`
impl<'a,K,V,S> IntoIterator for &'a RHashMap<K,V,S>{
    type Item=Tuple2<&'a K,&'a V>;
    type IntoIter=Iter<'a,K,V>;
    
    fn into_iter(self)->Self::IntoIter{
        self.iter()
    }
}


/// This returns an `Iterator<Item= Tuple2< &K, &mut V > >+!Send+!Sync`
impl<'a,K,V,S> IntoIterator for &'a mut RHashMap<K,V,S>{
    type Item=Tuple2<&'a K,&'a mut V>;
    type IntoIter=IterMut<'a,K,V>;
    
    fn into_iter(self)->Self::IntoIter{
        self.iter_mut()
    }
}


impl<K,V,S> From<HashMap<K,V,S>> for RHashMap<K,V,S>
where
    Self:Default
{
    fn from(map:HashMap<K,V,S>)->Self{
        map.into_iter().collect()
    }
}

impl<K,V,S> Into<HashMap<K,V,S>> for RHashMap<K,V,S>
where
    K:Eq+Hash,
    S:BuildHasher+Default,
{
    fn into(self)->HashMap<K,V,S>{
        self.into_iter().map(IntoReprRust::into_rust).collect()
    }
}


impl<K,V,S> FromIterator<(K,V)> for RHashMap<K,V,S>
where
    Self:Default,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K,V)>
    {
        let mut map=Self::default();
        map.extend(iter);
        map
    }
}


impl<K,V,S> FromIterator<Tuple2<K,V>> for RHashMap<K,V,S>
where
    Self:Default
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Tuple2<K,V>>
    {
        let mut map=Self::default();
        map.extend(iter);
        map
    }
}


impl<K,V,S> Extend<(K,V)> for RHashMap<K,V,S>{
    fn extend<I>(&mut self,iter: I)
    where
        I: IntoIterator<Item = (K,V)>
    {
        let iter=iter.into_iter();
        self.reserve(iter.size_hint().0);
        for (k,v) in iter {
            self.insert(k,v);
        }
    }
}


impl<K,V,S> Extend<Tuple2<K,V>> for RHashMap<K,V,S>{
    #[inline]
    fn extend<I>(&mut self,iter: I)
    where
        I: IntoIterator<Item = Tuple2<K,V>>
    {
        self.extend( iter.into_iter().map(Tuple2::into_rust) );
    }
}

impl<K,V,S> Default for RHashMap<K,V,S>
where
    K:Eq+Hash,
    S:BuildHasher+Default,
{
    fn default()->Self{
        Self::with_hasher(S::default())
    }
}


impl<K,V,S> Clone for RHashMap<K,V,S>
where 
    K:Clone,
    V:Clone,
    Self:Default
{
    fn clone(&self)->Self{
        self.iter().map(|Tuple2(k,v)| (k.clone(),v.clone()) ).collect()
    }
}


impl<K,V,S> Debug for RHashMap<K,V,S>
where 
    K:Debug,
    V:Debug,
{
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{
        f.debug_map()
         .entries(self.iter().map(Tuple2::into_rust))
         .finish()
    }
}


impl<K,V,S> Eq for RHashMap<K,V,S>
where 
    K:Eq,
    V:Eq,
{}


impl<K,V,S> PartialEq for RHashMap<K,V,S>
where 
    K:PartialEq,
    V:PartialEq,
{
    fn eq(&self,other:&Self)->bool{
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .all(|Tuple2(k, vl)|{
                other.get_p(k)
                     .map_or(false, |vr| *vr == *vl)
            })
    }
}


unsafe impl<K, V, S> Send for RHashMap<K, V, S> 
where
    HashMap<K, V, S>: Send,
{}

unsafe impl<K, V, S> Sync for RHashMap<K, V, S> 
where
    HashMap<K, V, S>: Sync,
{}


impl<K,Q,V,S> Index<&Q> for RHashMap<K,V,S>
where
    K:Borrow<Q>,
    Q:Eq+Hash+?Sized
{
    type Output=V;

    fn index(&self,query:&Q)->&V{
        self.get(query).expect("no entry in RHashMap<_,_> found for key")
    }
}

impl<K,Q,V,S> IndexMut<&Q> for RHashMap<K,V,S>
where
    K:Borrow<Q>,
    Q:Eq+Hash+?Sized
{
    fn index_mut(&mut self,query:&Q)->&mut V{
        self.get_mut(query).expect("no entry in RHashMap<_,_> found for key")
    }
}


mod serde{
    use super::*;

    use ::serde::{
        de::{Visitor, MapAccess},
        ser::SerializeMap,
        Deserialize,Serialize,Deserializer,Serializer,
    };


    struct RHashMapVisitor<K,V,S> {
        marker: PhantomData<fn() -> RHashMap<K,V,S>>
    }

    impl<K,V,S> RHashMapVisitor<K,V,S> {
        fn new() -> Self {
            RHashMapVisitor {
                marker: PhantomData
            }
        }
    }

    impl<'de,K,V,S> Visitor<'de> for RHashMapVisitor<K,V,S>
    where
        K: Deserialize<'de>,
        V: Deserialize<'de>,
        RHashMap<K,V,S>:Default,
    {
        type Value = RHashMap<K,V,S>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an RHashMap")
        }

        fn visit_map<M>(self, mut map_access: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let capacity=map_access.size_hint().unwrap_or(0);
            let mut map = RHashMap::default();
            map.reserve(capacity);

            while let Some((k, v)) = map_access.next_entry()? {
                map.insert(k, v);
            }

            Ok(map)
        }
    }

    impl<'de,K,V,S> Deserialize<'de> for RHashMap<K,V,S>
    where
        K: Deserialize<'de>,
        V: Deserialize<'de>,
        Self:Default,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_map(RHashMapVisitor::new())
        }
    }

    

    impl<K,V,S> Serialize for RHashMap<K,V,S>
    where
        K:Serialize,
        V:Serialize,
    {
        fn serialize<Z>(&self, serializer: Z) -> Result<Z::Ok, Z::Error>
        where
            Z: Serializer
        {
            let mut map = serializer.serialize_map(Some(self.len()))?;
            for Tuple2(k, v) in self.iter() {
                map.serialize_entry(k, v)?;
            }
            map.end()
        }
    }


}


///////////////////////////////////////////////////////////////////////////////


#[derive(StableAbi)]
#[repr(C)]
#[sabi(
    kind(Prefix(prefix_struct="VTable")),
    missing_field(panic),
    // The hasher doesn't matter
    unsafe_unconstrained(S),
    //debug_print,
)]
struct VTableVal<K,V,S>{
    ///
    insert_elem:extern "C" fn(&mut ErasedMap<K,V,S>,K,V)->ROption<V>,
    
    get_elem:for<'a> extern "C" fn(&'a ErasedMap<K,V,S>,MapQuery<'_,K>)->Option<&'a V>,
    get_mut_elem:for<'a> extern "C" fn(&'a mut ErasedMap<K,V,S>,MapQuery<'_,K>)->Option<&'a mut V>,
    remove_entry:extern "C" fn(&mut ErasedMap<K,V,S>,MapQuery<'_,K>)->ROption<Tuple2<K,V>>,
    
    get_elem_p:for<'a> extern "C" fn(&'a ErasedMap<K,V,S>,&K)->Option<&'a V>,
    get_mut_elem_p:for<'a> extern "C" fn(&'a mut ErasedMap<K,V,S>,&K)->Option<&'a mut V>,
    remove_entry_p:extern "C" fn(&mut ErasedMap<K,V,S>,&K)->ROption<Tuple2<K,V>>,
    
    reserve:extern "C" fn(&mut ErasedMap<K,V,S>,usize),
    clear_map:extern "C" fn(&mut ErasedMap<K,V,S>),
    len:extern "C" fn(&ErasedMap<K,V,S>)->usize,
    capacity:extern "C" fn(&ErasedMap<K,V,S>)->usize,
    iter    :extern "C" fn(&ErasedMap<K,V,S>     )->Iter<'_,K,V>,
    iter_mut:extern "C" fn(&mut ErasedMap<K,V,S> )->IterMut<'_,K,V>,
    drain   :extern "C" fn(&mut ErasedMap<K,V,S> )->Drain<'_,K,V>,
    iter_val:extern "C" fn(RBox<ErasedMap<K,V,S>>)->IntoIter<K,V>,
    #[sabi(last_prefix_field)]
    entry:extern "C" fn(&mut ErasedMap<K,V,S>,K)->REntry<'_,K,V>,
}



impl<K,V,S> VTable<K,V,S>
where
    K:Eq+Hash,
    S:BuildHasher,
{
    const VTABLE_REF: StaticRef<WithMetadata<VTableVal<K,V,S>>> = unsafe{
        StaticRef::from_raw(&WithMetadata::new(
            PrefixTypeTrait::METADATA,
            Self::VTABLE,
        ))
    };

    fn erased_map(hash_builder:S)->RBox<ErasedMap<K,V,S>>{
        unsafe{
            let map=HashMap::<MapKey<K>,V,S>::with_hasher(hash_builder);
            let boxed=BoxedHashMap{
                map,
                entry:None,
            };
            let boxed=RBox::new(boxed);
            let boxed=mem::transmute::<RBox<_>,RBox<ErasedMap<K,V,S>>>(boxed);
            boxed
        }
    }


    const VTABLE:VTableVal<K,V,S>=VTableVal{
        insert_elem :ErasedMap::insert_elem,

        get_elem    :ErasedMap::get_elem,
        get_mut_elem:ErasedMap::get_mut_elem,
        remove_entry:ErasedMap::remove_entry,

        get_elem_p    :ErasedMap::get_elem_p,
        get_mut_elem_p:ErasedMap::get_mut_elem_p,
        remove_entry_p:ErasedMap::remove_entry_p,

        reserve     :ErasedMap::reserve,
        clear_map   :ErasedMap::clear_map,
        len         :ErasedMap::len,
        capacity    :ErasedMap::capacity,
        iter        :ErasedMap::iter,
        iter_mut    :ErasedMap::iter_mut,
        drain       :ErasedMap::drain,
        iter_val    :ErasedMap::iter_val,
        entry       :ErasedMap::entry,
    };

}



///////////////////////////////////////////////////////////////////////////////

/*!
Types,traits,and functions used by prefix-types.

*/

use crate::{
    abi_stability::type_layout::{TypeLayout,TLField,TLData},
    std_types::StaticSlice,
};



/// A trait implemented by all prefix-types,providing some metadata about them.
pub trait PrefixTypeTrait{
    fn layout()->&'static TypeLayout;
    fn metadata()->PrefixTypeMetadata{
        PrefixTypeMetadata::new(Self::layout())
    }
}


#[derive(Debug,Copy,Clone)]
pub struct PrefixTypeMetadata{
    /// This is the ammount of fields on the prefix of the struct,
    /// which is always the same for the same type,regardless of which library it comes from.
    pub prefix_field_count:usize,

    pub fields:StaticSlice<TLField>,

    /// The layout of the struct,for error messages.
    pub layout:&'static TypeLayout,
}


impl PrefixTypeMetadata{
    pub fn new(layout:&'static TypeLayout)->Self{
        let (first_suffix_field,fields)=match layout.data {
            TLData::PrefixType{first_suffix_field,fields}=>
                (first_suffix_field,fields),
            _=>panic!(
                "Attempting to construct a PrefixTypeMetadata from a \
                 TypeLayout of a non-prefix-type.\n\
                 Type:{}\nDataVariant:{:?}\nPackage:{}",
                 layout.full_type,
                 layout.data.discriminant(),
                 layout.package,
            ),
        };
        Self{
            fields:fields,
            prefix_field_count:first_suffix_field,
            layout,
        }
    }

    /// Returns the maximum prefix.Does not check that they are compatible.
    /// 
    /// # Preconditions
    /// 
    /// The prefixes must already have been checked for compatibility.
    pub fn max(self,other:Self)->Self{
        if self.fields.len() < other.fields.len() {
            other
        }else{
            self
        }
    }
    /// Returns the minimum and maximum prefix.Does not check that they are compatible.
    /// 
    /// # Preconditions
    /// 
    /// The prefixes must already have been checked for compatibility.
    pub fn min_max(self,other:Self)->(Self,Self){
        if self.fields.len() < other.fields.len() {
            (self,other)
        }else{
            (other,self)
        }
    }
}


/// Used to panic with an error message informing the user that a field 
/// is expected to be on the `T` type when it's not.
#[cold]
#[inline(never)]
pub fn panic_on_missing_field_ty<T>(field_index:usize,actual_layout:&'static TypeLayout)->!
where T:PrefixTypeTrait
{
    panic_on_missing_field_val(field_index,T::layout(),actual_layout)
}


/// Used to panic with an error message informing the user that a field 
/// is expected to be on `expected` when it's not.
#[cold]
#[inline(never)]
pub fn panic_on_missing_field_val(
    field_index:usize,
    expected:&'static TypeLayout,
    actual:&'static TypeLayout,
)->! {
    let expected=PrefixTypeMetadata::new(expected);
    let actual=PrefixTypeMetadata::new(actual);

    let field=expected.fields[field_index];

    panic!("\n
Attempting to access nonexistent field:
    index:{index} 
    named:{field_named}
    type:{field_type}

Type:{struct_type}

Package:'{package}' 

Expected:
    Version(expected compatible):{expected_package_version}
    Field count:{expected_field_count}

Found:
    Version:{actual_package_version}
    Field count:{actual_field_count}

\n",
        index=field_index,
        field_named=field.name.as_str(),
        field_type=field.abi_info.get().layout.full_type,
        struct_type=expected.layout.full_type,
        package=expected.layout.package,
        
        expected_package_version =expected.layout.package_version ,
        expected_field_count=expected.fields.len(),
        
        actual_package_version =actual.layout.package_version ,
        actual_field_count=actual.fields.len(),
    );
}
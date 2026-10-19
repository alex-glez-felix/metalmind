/// Un `enum` de acciones con su nombre al lado de cada variante, para que
/// agregar una sea una línea.
macro_rules! named_actions {
    (
        $(#[doc = $doc:literal])*
        pub enum $name:ident {
            $($(#[doc = $variant_doc:literal])* $variant:ident = $text:literal,)*
        }
    ) => {
        $(#[doc = $doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[doc = $variant_doc])* $variant,)*
        }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant,)*];

            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)*
                }
            }

            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|action| action.name() == name)
            }
        }
    };
}

pub(crate) use named_actions;

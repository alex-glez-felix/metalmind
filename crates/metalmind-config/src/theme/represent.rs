//! En qué tipos lleva cada capa los valores de un tema. Los grupos
//! (`SyntaxColors`, `Layout`…) son genéricos en esto: con [`Plain`] traen las
//! unidades de config, y quien los usa los pasa a las suyas con `convert`,
//! sin volver a escribir sus campos.

use std::fmt::Debug;

use super::color::Color;
use super::units::{Count, Factor, Fraction, Length, MarkColumns, Px, TextStyle, Weight};

/// Cómo lleva `Self` un valor de la unidad `T`.
pub trait Represent<T> {
    type Value: Copy + Debug + PartialEq;
    fn represent(value: T) -> Self::Value;
}

/// Todas las unidades de un tema: lo que implementa esto lleva cualquier grupo.
pub trait Units:
    Represent<Color>
    + Represent<Px>
    + Represent<Length>
    + Represent<Factor>
    + Represent<Fraction>
    + Represent<Count>
    + Represent<Weight>
    + Represent<MarkColumns>
    + Represent<TextStyle>
{
}

/// Las unidades de config, sin convertir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plain;

impl<T: Copy + Debug + PartialEq> Represent<T> for Plain {
    type Value = T;

    fn represent(value: T) -> T {
        value
    }
}

impl Units for Plain {}

#[cfg(test)]
#[path = "represent_tests.rs"]
mod tests;

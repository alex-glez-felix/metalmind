/// Una clave del primer nivel del frontmatter y su valor, sea cual sea:
/// el frontmatter entero, solo para leer. Los escalares son su texto, como
/// en los campos que Metalmind entiende.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub key: String,
    pub value: PropertyValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    Null,
    Text(String),
    List(Vec<PropertyValue>),
    /// Un mapa anidado, en orden; las claves que no son un escalar se saltan.
    Map(Vec<Property>),
}

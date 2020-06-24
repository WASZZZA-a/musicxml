pub trait OptionExt<T> { fn try_map<E, U, F:FnOnce(T)->Result<U, E>>(self, f: F) -> Result<Option<U>, E>; }
impl<T> OptionExt<T> for Option<T> {
	fn try_map<E, U, F:FnOnce(T) ->Result<U, E>>(self, f: F) -> Result<Option<U>, E> { self.map(f).transpose() }
}

#[macro_use] mod serde;

#[derive(Debug)] pub struct Error(anyhow::Error);
impl std::fmt::Display for Error { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { self.0.fmt(f) } }
impl std::error::Error for Error {}
impl ::serde::de::Error for Error { fn custom<T: std::fmt::Display>(msg: T) -> Self { Error(anyhow::Error::msg(msg.to_string())) } }
impl From<de::value::Error> for Error { fn from(t: de::value::Error) -> Self { ::serde::de::Error::custom(t) } }
impl From<anyhow::Error> for Error { fn from(t: anyhow::Error) -> Self { Error(t) } }
impl From<std::num::ParseIntError> for Error { fn from(t: std::num::ParseIntError) -> Self { ::serde::de::Error::custom(t) } }
impl From<std::num::ParseFloatError> for Error { fn from(t: std::num::ParseFloatError) -> Self { ::serde::de::Error::custom(t) } }
impl From<std::str::ParseBoolError> for Error { fn from(t: std::str::ParseBoolError) -> Self { ::serde::de::Error::custom(t) } }
macro_rules! bail { ($($arg:tt)*) => { throw!(<Error as ::serde::de::Error>::custom(format!($($arg)*))) } }
macro_rules! ensure { ($cond:expr, $($arg:tt)*) => { if !$cond { bail!($($arg)*) } } }

mod content; use content::ContentDeserializer;
mod seq; use seq::SeqDeserializer;

use {fehler::*, ::serde::de::{self, Visitor, Deserializer}};

///

#[throws] fn from_yes_no(s: &str) -> bool {
	match s {
		"yes" => true,
		"no" => false,
		_ => bail!("provided string was not `yes` or `no`, got {}", s),
	}
}

struct TextDeserializer<'de>(&'de str);
impl<'de> Deserializer<'de> for TextDeserializer<'de> {
	type Error = Error;
	#[throws] fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> V::Value {
		if !self.0.trim().is_empty() { println!("text any->str {}", &visitor as &dyn de::Expected); }
		visitor.visit_str::<Error>(self.0)?
	}
	#[throws] fn deserialize_option<V:Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_some(self)? }
	#[throws] fn deserialize_str<V:Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_str::<Error>(self.0)? }
	#[throws] fn deserialize_string<V:Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_string::<Error>(self.0.to_owned())? }
	#[throws] fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_u8::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_u16::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_u32::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_i8::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_i16::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_i32::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_f32::<Error>(self.0.parse()?)? }
	#[throws] fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_bool::<Error>(/*self.0.parse()*/from_yes_no(self.0)?)? }
	#[throws] fn deserialize_enum<V: Visitor<'de>>(self, _name: &'static str, _variants: &'static [&'static str], visitor: V) -> V::Value {
		visitor.visit_enum(<&str as ::serde::de::IntoDeserializer<Error>>::into_deserializer(self.0))?
		//visitor.visit_enum::<Error>(self.0.into_deserializer()?)?
	}

	::serde::forward_to_deserialize_any!{
		char bytes byte_buf identifier u64 u128 i64 i128 f64 unit unit_struct newtype_struct tuple tuple_struct struct seq map ignored_any}
}

pub struct ElementDeserializer<'de> {
	name: &'de str,
	attributes: std::iter::Peekable<std::slice::Iter<'de, roxmltree::Attribute<'de>>>,
	children: std::iter::Peekable<roxmltree::Children<'de, 'de>>,
}

impl std::fmt::Debug for ElementDeserializer<'_> {
    #[throws(std::fmt::Error)] fn fmt(&self, f: &mut std::fmt::Formatter) {
        use itertools::Itertools;
        write!(f, "{} [{:?}] {{{:?}}}", self.name, self.attributes.clone().format(" "), self.children.clone().format(" "))?
    }
}

impl<'de> ElementDeserializer<'de> {
    fn new(node: roxmltree::Node<'de, 'de>) -> Self {
		assert!(node.is_element() || node.is_root(), "{:?}", node);
		Self{name: node.tag_name().name(), attributes: node.attributes().iter().peekable(), children: node.children().peekable()}
	}
	#[throws] fn simple_content(&mut self) -> &'de str {
		let text = self.children.next().ok_or_else(|| anyhow::Error::msg("Expected simple content"))?;
		ensure!(text.is_text() && self.children.next().is_none() && self.attributes.next().is_none(), "Expected simple content");
		text.text().unwrap()
    }
}

impl<'de> ::serde::de::IntoDeserializer<'de, Error> for TextDeserializer<'de> { type Deserializer = Self; fn into_deserializer(self) -> Self::Deserializer { self } }
impl<'de> ::serde::de::IntoDeserializer<'de, Error> for ElementDeserializer<'de> { type Deserializer = Self; fn into_deserializer(self) -> Self::Deserializer { self } }
impl<'de> ::serde::de::IntoDeserializer<'de, Error> for &mut ElementDeserializer<'de> { type Deserializer = Self; fn into_deserializer(self) -> Self::Deserializer { self } }
impl<'t, 'de> ::serde::de::IntoDeserializer<'de, Error> for ContentDeserializer<'t, 'de> { type Deserializer = Self; fn into_deserializer(self) -> Self::Deserializer { self } }
impl<'t, 'de> ::serde::de::IntoDeserializer<'de, Error> for SeqDeserializer<'t, 'de> { type Deserializer = Self; fn into_deserializer(self) -> Self::Deserializer { self } }

enum Value<'t, 'de> { Text(TextDeserializer<'de>), Element(ElementDeserializer<'de>), Content(ContentDeserializer<'t, 'de>), Seq(SeqDeserializer<'t, 'de>) }
delegatable_trait!{Value}
impl<'t, 'de> ::serde::de::IntoDeserializer<'de, Error> for Value<'t, 'de> { type Deserializer = Self; fn into_deserializer(self) -> Self::Deserializer { self } }

impl<'de> Deserializer<'de> for &mut ElementDeserializer<'de> {
	type Error = Error;
	#[throws] fn deserialize_unit<V:Visitor<'de>>(self, visitor: V) -> V::Value {
		assert!(self.attributes.next().is_none() && self.children.next().is_none());
		visitor.visit_unit::<Error>()?
	}
	#[throws] fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_str(visitor)? }
	#[throws] fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_string(visitor)? }
	#[throws] fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_u8(visitor)? }
	#[throws] fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_u16(visitor)? }
	#[throws] fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_u32(visitor)? }
	#[throws] fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_i8(visitor)? }
	#[throws] fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_i16(visitor)? }
	#[throws] fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_i32(visitor)? }
	#[throws] fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_f32(visitor)? }
	#[throws] fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> V::Value { TextDeserializer(self.simple_content()?).deserialize_bool(visitor)? }

	#[throws] fn deserialize_option<V:Visitor<'de>>(self, visitor: V) -> V::Value { visitor.visit_some(self)? }

	#[throws] fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> V::Value {
		println!("seq {:?}", self.name);
		visitor.visit_seq(::serde::de::value::SeqDeserializer::new(self.children.by_ref().filter(|child| child.is_element()).map(|child| {
			println!("item {:?}", child.tag_name().name());
			// /*Item*/ContentDeserializer(&mut NodeDeserializer::new(child)) // Item flatten => tag enum
			ElementDeserializer::new(child)
		})))?
	}

	#[throws] fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> V::Value {
		self.deserialize_struct("", &[], visitor)?
	}

	#[throws] fn deserialize_struct<V: Visitor<'de>>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> V::Value {
		println!("struct '{}' {:?} '{}'", name, fields, self.name);
		let cell = std::cell::RefCell::new(self);
		let mut index = 0;
		visitor.visit_map(::serde::de::value::MapDeserializer::new(std::iter::from_fn(|| {
			let mut node = cell.borrow_mut();
			//println!("back to struct '{}' {:?} '{}' {}", name, fields, node.name, index);
			if let Some(a) = node.attributes.peek() {
				println!("attribute {}", a.name());
				if fields.contains(&a.name()) {
					let a = node.attributes.next().unwrap();
					Some((a.name(), Value::Text(TextDeserializer(a.value()))))
				} else if let Some(field) = fields.iter().find(|field| field.is_empty() || field.parse() == Ok(index) ) {
					println!("no field '{}' in {:?}, deserializing attribute to '{}'", a.name(), fields, field);
					index += 1;
					//("", Value::Text(TextDeserializer(a.value()))) // Flatten
					Some((field, Value::Content(ContentDeserializer(node)))) // Flatten
				} else {
					println!("no field '{}' in {:?}'", a.name(), fields);
					None //TODO: panic if not in content context: panic!("Unknown {}='{}' in {:?} {:?}", a.name(), a.value(), name, fields);
				}
			} else {
				loop {
					if let Some(child) = node.children.peek() {
						let name = child.tag_name().name();
						let fields_iter = fields.iter().map(|field| (field, field.split_at(field.find(|c| "$*+{".contains(c)).unwrap_or(field.len()))));
						if !name.is_empty() /*&&*/{
							if let Some((field,(tag,def))) = fields_iter.clone().find(|(_,(id,_))| id == &name) {
								if !def.is_empty() {
									println!("external sequence '{}' {:?}", field, child);
									break Some((field, Value::Seq(SeqDeserializer{node, tag}))); // External sequence
								} else {
									println!("field '{}' {:?}", field, child);
									use roxmltree::NodeType::*; match child.node_type() {
										Text => break Some((field, Value::Text(TextDeserializer(node.children.next().unwrap().text().unwrap())))),
										Element => break Some((field, Value::Element(ElementDeserializer::new(node.children.next().unwrap())))),
										_ => todo!(),
									}
								}
							}
						}/*else*/ if child.is_element() /*&&*/{ if let Some((field,_)) = fields_iter.clone().find(|(_,(id,_))| id.is_empty() || id.parse()==Ok(index)) {
							println!("no field '{}' in {:?}, deserializing child to '{}'.'{}'", name, fields, node.name, field);
							index += 1;
							break Some((field, Value::Content(ContentDeserializer(node)))); // External enum tag
						} } /*else*/ if child.is_text() /*/*&&*/{ if let Some((field,_)) = fields_iter.clone().find(|(_,(_,def))| def==&"$")*/ {
							//println!("deserializing remaining content to {}", field);
							//break Some((field, Value::Content(ContentDeserializer(node)))); // External enum tag
							if !child.text().unwrap().trim().is_empty() { println!("deserializing text {:?} to {}", child, "$"); }
							break Some(("$", Value::Text(TextDeserializer(node.children.next().unwrap().text().unwrap())))); // External enum tag
						} /*else*/ {
							if child.is_text() && child.text().unwrap().trim().is_empty() {
								println!("skip whitespace {:?}", child);
								node.children.next();
							} else {
								println!("Unknown {:?}", child);
								break None;
								// todo: panic if not in content context
								//use itertools::Itertools; assert!(, "Ignored {:?} {:?}", child, fields_iter.format(" ")); // Helps complete format
							}
						}
					} else { break None; }
				}
			}
		})))?
	}

	#[throws] fn deserialize_enum<V: Visitor<'de>>(self, name: &'static str, variants: &'static [&'static str], visitor: V) -> V::Value {
		println!("enum '{}' {:?} {:?}", name, variants, self.name);
		if name ==  self.name {
			TextDeserializer(self.simple_content()?).deserialize_enum(name, variants, visitor)?
		} else {
			visitor.visit_enum(::serde::de::value::MapAccessDeserializer::new(::serde::de::value::MapDeserializer::new(std::iter::once((self.name, self)))))?
		}
    }

    #[throws] fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> V::Value {
		println!("any {}", &visitor as &dyn de::Expected);
		self.deserialize_map(visitor)?
	}
	::serde::forward_to_deserialize_any!{char bytes byte_buf identifier u64 u128 i64 i128 f64 unit_struct newtype_struct tuple tuple_struct ignored_any}
}

impl<'de> Deserializer<'de> for ElementDeserializer<'de> {
	type Error = Error;
	#[throws] fn deserialize_any<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_any(&mut self, visitor)? }
    #[throws] fn deserialize_unit<V:Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_unit(&mut self, visitor)? }
	#[throws] fn deserialize_str<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_str(&mut self, visitor)? }
	#[throws] fn deserialize_string<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_string(&mut self, visitor)? }
	#[throws] fn deserialize_u8<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_u8(&mut self, visitor)? }
	#[throws] fn deserialize_u16<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_u16(&mut self, visitor)? }
	#[throws] fn deserialize_u32<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_u32(&mut self, visitor)? }
	#[throws] fn deserialize_i8<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_i8(&mut self, visitor)? }
	#[throws] fn deserialize_i16<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_i16(&mut self, visitor)? }
	#[throws] fn deserialize_i32<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_i32(&mut self, visitor)? }
	#[throws] fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_f32(&mut self, visitor)? }
	#[throws] fn deserialize_option<V:Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_option(&mut self, visitor)? }
	#[throws] fn deserialize_seq<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_seq(&mut self, visitor)? }
	#[throws] fn deserialize_map<V: Visitor<'de>>(mut self, visitor: V) -> V::Value { <&mut Self as Deserializer>::deserialize_map(&mut self, visitor)? }
	#[throws] fn deserialize_struct<V: Visitor<'de>>(mut self, name: &'static str, fields: &'static [&'static str], visitor: V) -> V::Value {
		<&mut Self as Deserializer>::deserialize_struct(&mut self, name, fields, visitor)?
	}
	#[throws] fn deserialize_enum<V: Visitor<'de>>(mut self, name: &'static str, variants: &'static [&'static str], visitor: V) -> V::Value {
		<&mut Self as Deserializer>::deserialize_enum(&mut self, name, variants, visitor)?
    }
	::serde::forward_to_deserialize_any!{char bytes byte_buf identifier bool u64 u128 i64 i128 f64 unit_struct newtype_struct tuple tuple_struct ignored_any}
}

#[throws(anyhow::Error)] pub fn from_node<'input: 'de, 't: 'de, 'de, T: ::serde::Deserialize<'de>>(node: roxmltree::Node<'t, 'input>) -> T {
	T::deserialize(ElementDeserializer::new(node))?
}
#[throws(anyhow::Error)] pub fn from_document<'input: 'de, 'de, T: ::serde::Deserialize<'de>>(document: &'de roxmltree::Document<'input>) -> T {
	from_node(document.root())?
}
#[throws(anyhow::Error)] pub fn parse(bytes: &[u8]) -> roxmltree::Document { roxmltree::Document::parse(std::str::from_utf8(bytes)?)? }

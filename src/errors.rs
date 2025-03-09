#![allow(unused)]

use std::fmt::Display;
use thiserror::Error;
use zip::result::ZipError;

#[derive(Debug, Error)]
pub enum ConverterError {
    IOErr(#[from] std::io::Error),
    XMLError(String),
    Other(String),
}

impl Display for ConverterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl From<ZipError> for ConverterError {
    fn from(value: ZipError) -> Self {
        io_err!(std::io::ErrorKind::InvalidData, "{}", value.to_string())
    }
}

impl From<xmltree::ParseError> for ConverterError {
    fn from(value: xmltree::ParseError) -> Self {
        match value {
            xmltree::ParseError::CannotParse => xml_err!("Cannot parse xml file"),
            xmltree::ParseError::MalformedXml(e) => ConverterError::XMLError(e.to_string()),
        }
    }
}

impl From<xmltree::Error> for ConverterError {
    fn from(value: xmltree::Error) -> Self {
        match value {
            xmltree::Error::Io(error) => ConverterError::IOErr(error),
            xmltree::Error::DocumentStartAlreadyEmitted => xml_err!("Document start already written"),
            xmltree::Error::LastElementNameNotAvailable => xml_err!("Last element name not available"),
            xmltree::Error::EndElementNameIsNotEqualToLastStartElementName => {
                xml_err!("End element name is not equal to last start element name")
            }
            xmltree::Error::EndElementNameIsNotSpecified => xml_err!("End element name is not specified"),
        }
    }
}

impl ConverterError {}

macro_rules! io_err {
    ($kind:expr, $($arg:tt)*) => {
       $crate::errors::ConverterError::IOErr(std::io::Error::new($kind, format!($($arg)*)))
    };
}
pub(crate) use io_err;

macro_rules! xml_err {
    ($($arg:tt)*) => {
        $crate::errors::ConverterError::XMLError(format!($($arg)*))
    };
}
pub(crate) use xml_err;

// @generated
// This file is @generated by prost-build.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Calls {
    #[prost(message, optional, tag="1")]
    pub clock: ::core::option::Option<super::super::v1::Clock>,
    #[prost(message, repeated, tag="3")]
    pub calls: ::prost::alloc::vec::Vec<Call>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Call {
    #[prost(message, optional, tag="1")]
    pub call: ::core::option::Option<super::super::super::ethereum::r#type::v2::Call>,
    #[prost(string, tag="2")]
    pub tx_hash: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Events {
    #[prost(message, optional, tag="1")]
    pub clock: ::core::option::Option<super::super::v1::Clock>,
    #[prost(message, repeated, tag="2")]
    pub events: ::prost::alloc::vec::Vec<Event>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Event {
    #[prost(message, optional, tag="1")]
    pub log: ::core::option::Option<super::super::super::ethereum::r#type::v2::Log>,
    #[prost(string, tag="2")]
    pub tx_hash: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EventsAndCalls {
    #[prost(message, optional, tag="1")]
    pub clock: ::core::option::Option<super::super::v1::Clock>,
    #[prost(message, repeated, tag="2")]
    pub events: ::prost::alloc::vec::Vec<Event>,
    #[prost(message, repeated, tag="3")]
    pub calls: ::prost::alloc::vec::Vec<Call>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Transactions {
    #[prost(message, repeated, tag="1")]
    pub transactions: ::prost::alloc::vec::Vec<Transaction>,
    #[prost(message, optional, tag="2")]
    pub clock: ::core::option::Option<super::super::v1::Clock>,
    #[prost(enumeration="DetailLevel", tag="3")]
    pub detail_level: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Transaction {
    #[prost(message, optional, tag="1")]
    pub trace: ::core::option::Option<super::super::super::ethereum::r#type::v2::TransactionTrace>,
    #[prost(string, tag="2")]
    pub tx_hash: ::prost::alloc::string::String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DetailLevel {
    DetaillevelExtended = 0,
    /// DETAILLEVEL_TRACE = 1; // TBD
    DetaillevelBase = 2,
}
impl DetailLevel {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DetailLevel::DetaillevelExtended => "DETAILLEVEL_EXTENDED",
            DetailLevel::DetaillevelBase => "DETAILLEVEL_BASE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "DETAILLEVEL_EXTENDED" => Some(Self::DetaillevelExtended),
            "DETAILLEVEL_BASE" => Some(Self::DetaillevelBase),
            _ => None,
        }
    }
}
// @@protoc_insertion_point(module)

pub trait Multipart {
    fn clone(&self) -> Self;
    fn name(&self) -> String;
    fn data(&self) -> Bytes;
    fn content_type(&self) -> String;
}

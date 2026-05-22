pub trait Consume {
    type Item;

    /// Push a single item to the internal buffer
    fn push(&mut self, item: Self::Item);
    /// Push multiple items to the internal buffer
    fn extend(&mut self, item: &[Self::Item]);
    /// Get the capacity of the internal buffer
    fn capacity(&self) -> usize;
    /// Get the length of the internal buffer
    fn len(&self) -> usize;
    /// Indicate if the internal buffer is empty
    fn is_empty(&self) -> bool;
}

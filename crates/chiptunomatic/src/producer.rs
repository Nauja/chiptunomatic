use alloc::vec::Vec;

pub trait Producer {
    type Item;

    fn poll(&mut self) -> Option<Vec<Self::Item>>;
}

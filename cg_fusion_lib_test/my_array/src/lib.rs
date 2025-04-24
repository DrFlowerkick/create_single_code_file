use std::iter::FromIterator;

#[derive(Copy, Clone, PartialEq)]
pub struct MyArray<T, const N: usize> {
    items: [T; N],
    n_items: usize,
}

impl<T: Copy + Clone + Default, const N: usize> MyArray<T, N> {
    pub fn new() -> Self {
        Self {
            items: [T::default(); N],
            n_items: 0,
        }
    }
    pub fn init(init_item: T, n_init_items: usize) -> Self {
        Self {
            items: [init_item; N],
            n_items: n_init_items,
        }
    }
    pub fn push(&mut self, item: T) {
        if self.n_items == N {
            panic!("line {}", line!());
        }
        self.items[self.n_items] = item;
        self.n_items += 1;
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.n_items == 0 {
            return None;
        }
        self.n_items -= 1;
        Some(self.items[self.n_items])
    }
    pub fn insert(&mut self, index: usize, item: T) {
        if index > self.n_items || self.n_items == N {
            panic!("line {}", line!());
        }
        self.items[index..].rotate_right(1);
        self.items[index] = item;
        self.n_items += 1;
    }
    pub fn replace(&mut self, index: usize, item: T) -> Option<T> {
        if index >= self.n_items {
            return None;
        }
        let result = self.items[index];
        self.items[index] = item;
        Some(result)
    }
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.n_items {
            return None;
        }
        let result = self.items[index];
        self.items[index..].rotate_left(1);
        self.n_items -= 1;
        Some(result)
    }
    pub fn flush(&mut self) {
        self.n_items = 0;
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.n_items {
            return None;
        }
        Some(&self.items[index])
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.n_items {
            return None;
        }
        Some(&mut self.items[index])
    }
    pub fn get_last(&mut self) -> Option<&T> {
        if self.n_items == 0 {
            return None;
        }
        Some(&self.items[self.n_items])
    }
    pub fn get_slice(&self, index: usize, len: usize) -> &[T] {
        if len == 0 {
            panic!("line {}", line!());
        }
        if index + len > self.n_items {
            panic!("line {}", line!());
        }
        &self.items[index..index + len]
    }
    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.n_items]
    }
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        &mut self.items[..self.n_items]
    }
    pub fn append_slice(&mut self, slice: &[T]) {
        if self.n_items + slice.len() > N {
            panic!("line {}", line!());
        }
        for (i, item) in slice.iter().enumerate() {
            self.items[self.n_items + i] = *item;
        }
        self.n_items += slice.len();
    }
    pub fn set(&mut self, index: usize, item: T) -> Option<&T> {
        if index >= self.n_items {
            return None;
        }
        self.items[index] = item;
        Some(&self.items[index])
    }
    pub fn len(&self) -> usize {
        self.n_items
    }
    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }
    pub fn remaining_len(&self) -> usize {
        N - self.n_items
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().take(self.n_items)
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut().take(self.n_items)
    }
}

impl<T, const N: usize> FromIterator<T> for MyArray<T, N>
where
    T: Copy + Clone + Default,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut my_array: MyArray<T, N> = MyArray::new();

        for i in iter {
            my_array.push(i);
        }
        my_array
    }
}

impl<T: Copy + Clone + Default, const N: usize> Default for MyArray<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect() {
        let array = [0, 1, 2, 3, 4];
        let my_array: MyArray<i32, 10> = array.iter().map(|i| *i).collect();
        assert_eq!(my_array.as_slice(), &array[..]);
    }

    #[test]
    fn test_collect_empty_collection() {
        let array: [i32; 0] = [];
        let my_array: MyArray<i32, 10> = array.iter().map(|i| *i).collect();
        assert_eq!(my_array.len(), 0);
    }

    #[test]
    #[should_panic]
    fn test_collect_to_large_collection() {
        let array = [0, 1, 2, 3, 4];
        let _my_array: MyArray<i32, 4> = array.iter().map(|i| *i).collect();
    }
}

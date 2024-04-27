use crate::Script;
use smallvec::SmallVec;
use std::io::{stdin, stdout, Read, Write};
use std::ops::{Add, Sub};

pub struct RuntimeContext<T>
where
    T: CellType,
{
    pub data: Vec<T>,
    pub data_pointer: usize,

    pub min_cell_value: T,
    pub max_cell_value: T,

    pub refresh_fn: Option<Box<dyn Fn(&Script, &Self)>>,
    pub read_fn: Box<dyn FnMut() -> T>,
    pub write_fn: Box<dyn FnMut(T)>,
}

impl<T> RuntimeContext<T>
where
    T: CellType,
{
    pub fn new(read: impl FnMut() -> T + 'static, write: impl FnMut(T) + 'static) -> Self {
        Self {
            data: Vec::with_capacity(30000), // Minimum capacity according to Wikipedia
            data_pointer: 0,
            min_cell_value: T::min_value(),
            max_cell_value: T::max_value(),
            refresh_fn: None,
            read_fn: Box::new(read),
            write_fn: Box::new(write),
        }
    }
    pub fn new_stdio() -> Self {
        Self::new(
            || {
                let mut value = [0u8];
                stdin().read_exact(&mut value).expect("Could not read");
                T::from_u8(value[0])
            },
            |value| {
                stdout()
                    .write(&value.as_u8_array())
                    .expect("Could not write");
            },
        )
    }

    pub fn get_cell(&mut self, i: usize) -> &mut T {
        if self.data.len() <= i {
            self.data.resize(i + 1, T::zero());
        }
        let ptr = &mut self.data[i];

        ptr
    }
    pub fn fix_cell(&mut self, i: usize) {
        let max = self.max_cell_value;
        let min = self.min_cell_value;
        let cell = self.get_cell(i);
        let diff = max - min;
        while *cell > max {
            *cell = *cell - diff;
        }
        while *cell < min {
            *cell = *cell + diff;
        }
    }
    pub fn read_cell(&self, i: usize) -> T {
        if self.data.len() <= i {
            return T::zero();
        }
        self.data[i]
    }
    pub fn increment_cell(&mut self, i: usize) {
        let max = self.max_cell_value;
        let min = self.min_cell_value;
        let cell = self.get_cell(i);
        if *cell >= max {
            *cell = min;
        } else {
            *cell = *cell + T::one();
        }
    }
    pub fn decrement_cell(&mut self, i: usize) {
        let max = self.max_cell_value;
        let min = self.min_cell_value;
        let cell = self.get_cell(i);
        if *cell <= min {
            *cell = max;
        } else {
            *cell = *cell - T::one();
        }
    }

    pub fn refresh(&self, script: &Script) {
        if let Some(refresh_fn) = self.refresh_fn.as_ref() {
            refresh_fn(script, self);
        }
    }
    pub fn read(&mut self) -> T {
        (self.read_fn)()
    }
    pub fn write(&mut self, value: T) {
        (self.write_fn)(value)
    }
}

pub type RuntimeContextU8 = RuntimeContext<u8>;
pub type RuntimeContextU64 = RuntimeContext<u64>;

pub trait CellType:
    Copy + Clone + Ord + Eq + Sub<Output = Self> + Add<Output = Self> + 'static
{
    fn min_value() -> Self;
    fn zero() -> Self;
    fn one() -> Self;
    fn max_value() -> Self;

    fn from_u8(value: u8) -> Self;
    fn from_str_radix(str: &str, radix: u32) -> Result<Self, ()>;

    fn as_u8_array(&self) -> SmallVec<[u8; 8]>;
}

macro_rules! cell_type_impl {
    ($ty:ty) => {
        impl CellType for $ty {
            fn min_value() -> Self {
                Self::MIN
            }
            fn zero() -> Self {
                0
            }
            fn one() -> Self {
                1
            }
            fn max_value() -> Self {
                Self::MAX
            }
            fn from_u8(value: u8) -> Self {
                value as Self
            }
            fn from_str_radix(str: &str, radix: u32) -> Result<Self, ()> {
                <$ty>::from_str_radix(str, radix).map_err(|_| ())
            }

            fn as_u8_array(&self) -> SmallVec<[u8; 8]> {
                let mut vec = SmallVec::new_const();
                for b in self.to_be_bytes() {
                    vec.push(b)
                }
                vec
            }
        }
    };
}

cell_type_impl!(u8);
cell_type_impl!(u16);
cell_type_impl!(u32);
cell_type_impl!(u64);
cell_type_impl!(usize);

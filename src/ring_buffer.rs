use std::mem::MaybeUninit;

/// A fixed sized ring buffer that is [`Copy`].
///
/// This is a domain specific data structure for implementing [`TrailBuffer`](crate::trail::TrailBuffer).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RingBuffer<T: Copy, const N: usize> {
    len: usize,
    start: usize,
    buffer: [MaybeUninit<T>; N],
}

impl<T: Copy, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! pop {
    ($this: expr) => {
        match $this.len {
            0 => (),
            1 => {
                $this.len = 0;
                $this.start = 0;
            }
            _ => {
                $this.len -= 1;
                $this.start = ($this.start + 1) % N;
            }
        }
    };
}

impl<T: Copy, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Self {
        RingBuffer {
            buffer: [MaybeUninit::uninit(); N],
            len: 0,
            start: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, item: T) {
        self.buffer[(self.start + self.len) % N] = MaybeUninit::new(item);
        if self.len == N {
            self.start = (self.start + 1) % N;
        } else {
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        fn assume_init<A>(x: &MaybeUninit<A>) -> &A {
            unsafe { x.assume_init_ref() }
        }
        if self.start + self.len > N {
            self.buffer[self.start..N]
                .iter()
                .chain(self.buffer[0..self.start + self.len - N].iter())
                .map(assume_init)
        } else {
            self.buffer[self.start..self.start + self.len]
                .iter()
                .chain([].iter())
                .map(assume_init)
        }
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> + '_ {
        fn assume_init<A>(x: &mut MaybeUninit<A>) -> &mut A {
            unsafe { x.assume_init_mut() }
        }
        if self.start + self.len > N {
            let (start, end) = self.buffer.split_at_mut(self.start);
            end.iter_mut()
                .chain(start[0..self.start + self.len - N].iter_mut())
                .map(assume_init)
        } else {
            self.buffer[self.start..self.start + self.len]
                .iter_mut()
                .chain([].iter_mut())
                .map(assume_init)
        }
    }

    /// Iterates and pops an item whenever the function returns false.
    /// Should only be used if items are ordered.
    pub fn retain_mut_ordered(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        fn assume_init<A>(x: &mut MaybeUninit<A>) -> &mut A {
            unsafe { x.assume_init_mut() }
        }
        let len = self.start + self.len;
        if len > N {
            for item in &mut self.buffer[self.start..N] {
                if !f(unsafe { item.assume_init_mut() }) {
                    pop!(self)
                }
            }
            for item in &mut self.buffer[0..len - N] {
                if !f(unsafe { item.assume_init_mut() }) {
                    pop!(self)
                }
            }
        } else {
            for item in &mut self.buffer[self.start..len] {
                if !f(unsafe { item.assume_init_mut() }) {
                    pop!(self)
                }
            }
        }
    }

    pub fn pop_front(&mut self) {
        pop!(self)
    }
}

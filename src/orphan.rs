#![allow(dead_code)]

#[repr(transparent)]
pub struct Local<T>(pub T);

impl<T> Local<T> {
    #[inline]
    pub fn from<V: Into<Self>>(value: V) -> Self {
        value.into()
    }

    #[inline]
    pub fn into<V: Into<Local<T>>>(value: V) -> T {
        value.into().0
    }
}

#[inline]
pub fn into<T, V: Into<Local<T>>>(value: V) -> T {
    value.into().0
}

impl<T> std::ops::Deref for Local<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Local<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

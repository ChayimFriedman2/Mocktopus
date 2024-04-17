use std::cell::{Cell, UnsafeCell};
use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

/// **This function is deprecated.** Using it invokes immediate undefined behavior, *even if the resulting reference is not used*.
/// If you need to convert `&` to `&mut`, use [`OnceMutCell`] or [`UnsafeCell`] instead.
///
/// For example:
///
/// ```
/// #[mockable]
/// fn get_string(context: &mut Context) -> &mut String {
///     context.get_mut_string()
/// }
///
/// #[test]
/// fn get_string_test() {
///     let mocked = OnceMutCell::new("mocked".to_string());
///     // MockResult::Return(&mut string) would fail
///     get_string.mock_raw(|_| MockResult::Return(mocked.borrow()));
///
///     assert_eq!("mocked", get_string(&mut Context::default()));
/// }
/// ```
///
/// -----------------
///
/// Converts non-mutable reference to a mutable one
///
/// Allows creating multiple mutable references to a single item breaking Rust's safety policy.
/// # Safety
/// Use with extreme caution, may cause all sorts of mutability related undefined behaviors!
///
/// One safe use case is when mocking function, which gets called only once during whole test execution, for example:
///
/// ```
/// #[mockable]
/// fn get_string(context: &mut Context) -> &mut String {
///     context.get_mut_string()
/// }
///
/// #[test]
/// fn get_string_test() {
///     let mocked = "mocked".to_string();
///     unsafe {
///         // MockResult::Return(&mut string) would fail
///         get_string.mock_raw(|_| MockResult::Return(as_mut(&mocked)));
///     }
///
///     assert_eq!("mocked", get_string(&mut Context::default()));
/// }
/// ```
///
/// [`UnsafeCell`]: std::cell::UnsafeCell
#[deprecated = "this function invokes immediate undefined behavior and cannot be used correctly"]
#[allow(invalid_reference_casting)]
pub unsafe fn as_mut<T>(t_ref: &T) -> &mut T {
    &mut *(t_ref as *const T as *mut T)
}

/// An error that is raised when you try to borrow a [`OnceMutCell`] that is already borrowed.
///
/// If you have mutable access to the cell, call [`OnceMutCell::get_mut()`] instead.
///
/// If you don't have mutable access to the cell at the borrow time but there is a time between the borrows
/// when you have mutable access to it, call [`OnceMutCell::reset()`] at that time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct OnceMutCellBorrowedError;

impl fmt::Display for OnceMutCellBorrowedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(deprecated)]
        f.write_str(self.description())
    }
}

impl std::error::Error for OnceMutCellBorrowedError {
    #[inline]
    fn description(&self) -> &str {
        "`OnceMutCell` already borrowed"
    }
}

/// A cell that can be mutably borrowed, but only once.
///
/// The cell can be borrowed more than once if you have a mutable access to it by [resetting] it.
///
/// [resetting]: OnceMutCell::reset
///
/// # Example
///
/// ```
/// # use mocktopus::mocking_utils::OnceMutCell;
/// let mut cell = OnceMutCell::new(123_i32);
///
/// let v1: &mut i32 = cell.borrow();
/// *v1 = 456;
///
/// cell.reset();
///
/// let v2 = cell.borrow();
/// assert_eq!(*v2, 456);
/// ```
pub struct OnceMutCell<T: ?Sized> {
    borrowed: Cell<bool>,
    value: UnsafeCell<T>,
}

impl<T> OnceMutCell<T> {
    /// Creates a new `OnceMutCell` with the specified initial value.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self {
            borrowed: Cell::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// Consumes the cell, returning its value.
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> OnceMutCell<T> {
    /// Gives an access to the cell's contents *when you have a mutable reference*.
    ///
    /// If you only have a shared reference, call [`borrow()`] instead.
    ///
    /// **Note:** Even though this takes a mutable reference (that serves as a proof there are no borrows of the cell),
    /// this *does not* allow further `borrow()`s if the cell was borrowed already. If you need that, also call [`reset()`].
    ///
    /// [`borrow()`]: OnceMutCell::borrow
    /// [`reset()`]: OnceMutCell::reset
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// Allows further borrows of the cell.
    ///
    /// This can be done safely since this method takes a mutable reference, which serves as a proof there are no
    /// outstanding borrows.
    #[inline]
    pub fn reset(&mut self) {
        self.borrowed.set(false);
    }

    /// Tries to borrow the cell, returning an error if it is already borrowed.
    ///
    /// For a panicking version see [`borrow()`].
    ///
    /// [`borrow()`]: OnceMutCell::borrow
    #[inline]
    pub fn try_borrow(&self) -> Result<&mut T, OnceMutCellBorrowedError> {
        if self.borrowed.get() {
            return Err(OnceMutCellBorrowedError);
        }

        self.borrowed.set(true);
        // SAFETY: We only allow one borrow (`self.borrowed` ensures that), and we can only get more borrows
        // if we `reset()`, which requires a mutable reference to ensure there are no references to our value.
        Ok(unsafe { &mut *self.value.get() })
    }

    /// Tries to borrow the cell, panicking if it is already borrowed.
    ///
    /// For a fallible version see [`try_borrow()`].
    ///
    /// # Panics
    ///
    /// Panics if the cell is already borrowed.
    ///
    /// [`try_borrow()`]: OnceMutCell::try_borrow
    #[inline]
    #[track_caller]
    pub fn borrow(&self) -> &mut T {
        match self.try_borrow() {
            Ok(value) => value,
            Err(_) => panic_already_borrowed(),
        }
    }

    /// Tries to borrow the cell. If it succeeds, calls the callback and returns its return value. If it fails, returns an error.
    /// After the callback has finished, resets the cell.
    ///
    /// This enables pattern that are impossible to express with [`try_borrow()`], since this essentially resets the cell with a shared
    /// reference (but this is safe, since we set the cell as borrowed and we finished borrowing it).
    ///
    /// On the other hand, the closure's return value cannot borrow from the cell.
    ///
    /// For a panicking version see [`with()`].
    ///
    /// [`try_borrow()`]: OnceMutCell::try_borrow
    /// [`with()`]: OnceMutCell::with
    #[inline]
    pub fn try_with<R, F: FnOnce(&mut T) -> R>(
        &self,
        callback: F,
    ) -> Result<R, OnceMutCellBorrowedError> {
        struct Guard<'a, T: ?Sized>(&'a OnceMutCell<T>);
        impl<T: ?Sized> Drop for Guard<'_, T> {
            #[inline]
            fn drop(&mut self) {
                self.0.borrowed.set(false);
            }
        }

        if self.borrowed.get() {
            return Err(OnceMutCellBorrowedError);
        }

        let guard = Guard(self);
        guard.0.borrowed.set(true);
        // SAFETY: We only allow one borrow (`self.borrowed` ensures that), and we can only get more borrows
        // if we `reset()`, which requires a mutable reference to ensure there are no references to our value.
        Ok(callback(unsafe { &mut *guard.0.value.get() }))
    }

    /// Tries to borrow the cell. If it succeeds, calls the callback and returns its return value. If it fails, panics.
    /// After the callback has finished, resets the cell.
    ///
    /// This enables pattern that are impossible to express with [`borrow()`], since this essentially resets the cell with a shared
    /// reference (but this is safe, since we set the cell as borrowed and we finished borrowing it).
    ///
    /// On the other hand, the closure's return value cannot borrow from the cell.
    ///
    /// For a fallible version see [`try_with()`].
    ///
    /// # Panics
    ///
    /// Panics if the cell is already borrowed.
    ///
    /// [`borrow()`]: OnceMutCell::borrow
    /// [`try_with()`]: OnceMutCell::try_with
    #[inline]
    #[track_caller]
    pub fn with<R, F: FnOnce(&mut T) -> R>(&self, callback: F) -> R {
        match self.try_with(callback) {
            Ok(result) => result,
            Err(_) => panic_already_borrowed(),
        }
    }
}

#[cold]
#[track_caller]
fn panic_already_borrowed() -> ! {
    panic!("`OnceMutCell` already borrowed")
}

impl<T: Clone> Clone for OnceMutCell<T> {
    /// # Panics
    ///
    /// Panics if the cell is already borrowed.
    #[inline]
    #[track_caller]
    fn clone(&self) -> Self {
        Self::new(self.with(|v| v.clone()))
    }
}

impl<T: Default> Default for OnceMutCell<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for OnceMutCell<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: PartialEq + ?Sized> PartialEq for OnceMutCell<T> {
    /// # Panics
    ///
    /// Panics if the cell is already borrowed.
    #[inline]
    #[track_caller]
    fn eq(&self, other: &Self) -> bool {
        self.with(|this| other.with(|other| this == other))
    }
}

impl<T: Eq + ?Sized> Eq for OnceMutCell<T> {}

impl<T: PartialOrd + ?Sized> PartialOrd for OnceMutCell<T> {
    /// # Panics
    ///
    /// Panics if the cell is already borrowed.
    #[inline]
    #[track_caller]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.with(|this| other.with(|other| (*this).partial_cmp(&*other)))
    }
}

impl<T: Ord + ?Sized> Ord for OnceMutCell<T> {
    /// # Panics
    ///
    /// Panics if the cell is already borrowed.
    #[inline]
    #[track_caller]
    fn cmp(&self, other: &Self) -> Ordering {
        self.with(|this| other.with(|other| (*this).cmp(&*other)))
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for OnceMutCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_with(|value| f.debug_tuple("OnceMutCell").field(&value).finish()) {
            Ok(result) => result,
            Err(_) => f.pad("OnceMutCell(<borrowed>)"),
        }
    }
}

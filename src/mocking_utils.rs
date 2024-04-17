/// **This function is deprecated.** Using it invokes immediate undefined behavior, *even if the resulting reference is not used*.
/// If you need to convert `&` to `&mut`, use [`UnsafeCell`] instead.
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
///     let mocked = std::cell::UnsafeCell::new("mocked".to_string());
///     // MockResult::Return(&mut string) would fail
///     // SAFETY: We only call this function once, so there are no aliasing violations.
///     get_string.mock_raw(|_| MockResult::Return(unsafe { &mut *mocked.get() }));
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

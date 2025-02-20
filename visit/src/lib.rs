//! Visitor generator for the rust language.
//!
//!
//! There are three variants of visitor in swc. Those are `Fold`, `VisitMut`,
//! `Visit`.
//!
//! # Comparisons
//!
//! ## `Fold` vs `VisitMut`
//!
//! `Fold` and `VisitMut` do almost identical tasks, but `Fold` is easier to use
//! while being slower and weak to stack overflow for very deep asts. `Fold` is
//! fast enough for almost all cases so it would be better to start with `Fold`.
//!
//! By very deep asts, I meant code like thousands of `a + a + a + a + ...`.
//!
//!
//! # `Fold`
//!
//! `Fold` takes ownership of value, which means you have to return the new
//! value. Returning new value means returning ownership of the value. But you
//! don't have to care about ownership or about managing memories while using
//! such visitors. `rustc` handles them automatically and all allocations will
//! be freed when it goes out of the scope.
//!
//! You can invoke your `Fold` implementation like `node.fold_with(&mut
//! visitor)` where `visitor` is your visitor. Note that as it takes ownership
//! of value, you have to call `node.fold_children_with(self)` in e.g. `fn
//! fold_module(&mut self, m: Module) -> Module` if you override the default
//! behavior. Also you have to store return value from `fold_children_with`,
//! like `let node = node.fold_children_with(self)`. Order of execution can be
//! controlled using this. If there is some logic that should be applied to the
//! parent first, you can call `fold_children_with` after such logic.
//!
//! # `VisitMut`
//!
//! `VisitMut` uses a mutable reference to AST nodes (e.g. `&mut Expr`). You can
//! use `MapWithMut` from `swc_ecma_transforms_base` to get owned value from a
//! mutable reference.
//!
//! You will typically use code like
//!
//! ```ignore
//! *e = return_value.take();
//! ```
//!
//! where `e = &mut Expr` and `return_value` is also `&mut Expr`. `take()` is an
//! extension method defined on `MapWithMut`.  It's almost identical to `Fold`,
//! so I'll skip memory management.
//!
//! You can invoke your `VisitMut` implementation like `node.visit_mut_with(&mut
//! visitor)` where `visitor` is your visitor. Again, you need to call
//! `node.visit_mut_children_with(self)` in visitor implementation if you want
//! to modify children nodes. You don't need to store the return value in this
//! case.
//!
//!
//! # `Visit`
//!
//!`Visit` uses non-mutable references to AST nodes. It can be used to see if
//! an AST node contains a specific node nested deeply in the AST. This is
//! useful for checking if AST node contains `this`. This is useful for lots of
//! cases - `this` in arrow expressions are special and we need to generate
//! different code if a `this` expression is used.
//!
//! You can use your `Visit` implementation like  `node.visit_with(&Invalid{
//! span: DUMY_SP, }, &mut visitor`. I think API is misdesigned, but it works
//! and there are really lots of code using `Visit` already.

pub use either::Either;
pub use swc_visit_macros::define;

pub mod util;

/// Visit all children nodes. This converts `VisitAll` to `Visit`. The type
/// parameter `V` should implement `VisitAll` and `All<V>` implements `Visit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct All<V> {
    pub visitor: V,
}

/// A visitor which visits node only if `enabled` is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Optional<V> {
    pub enabled: bool,
    pub visitor: V,
}

impl<V> Optional<V> {
    pub fn new(visitor: V, enabled: bool) -> Self {
        Self { enabled, visitor }
    }
}

/// Trait for a pass which is designed to invoked multiple time to same input.
///
/// See [Repeat].
pub trait Repeated {
    /// Should run again?
    fn changed(&self) -> bool;

    /// Reset.
    fn reset(&mut self);
}

/// A visitor which applies `A` and then `B`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AndThen<A, B> {
    pub first: A,
    pub second: B,
}

/// Chains multiple visitor.
#[macro_export]
macro_rules! chain {
    ($a:expr, $b:expr) => {{
        use $crate::AndThen;

        AndThen {
            first: $a,
            second: $b,
        }
    }};

    ($a:expr, $b:expr,) => {
        chain!($a, $b)
    };

    ($a:expr, $b:expr,  $($rest:tt)+) => {{
        use $crate::AndThen;

        AndThen{
            first: $a,
            second: chain!($b, $($rest)*),
        }
    }};
}

/// A visitor which applies `V` again and again if `V` modifies the node.
///
/// # Note
/// `V` should return `true` from `changed()` to make the pass run multiple
/// time.
///
/// See: [Repeated]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Repeat<V>
where
    V: Repeated,
{
    pub pass: V,
}

impl<V> Repeat<V>
where
    V: Repeated,
{
    pub fn new(pass: V) -> Self {
        Self { pass }
    }
}

impl<V> Repeated for Repeat<V>
where
    V: Repeated,
{
    fn changed(&self) -> bool {
        self.pass.changed()
    }

    fn reset(&mut self) {
        self.pass.reset()
    }
}

impl<A, B> Repeated for AndThen<A, B>
where
    A: Repeated,
    B: Repeated,
{
    fn changed(&self) -> bool {
        self.first.changed() || self.second.changed()
    }

    fn reset(&mut self) {
        self.first.reset();
        self.second.reset();
    }
}

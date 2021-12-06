use super::*;

use std::rc::{Rc, Weak};

// TODO: Remove when `OnceCell` is stable
struct OnceCell<T>(std::cell::RefCell<Option<T>>);
impl<T> OnceCell<T> {
    pub fn new() -> Self {
        Self(std::cell::RefCell::new(None))
    }
    pub fn set(&self, x: T) -> Result<(), ()> {
        *self.0.try_borrow_mut().map_err(|_| ())? = Some(x);
        Ok(())
    }
    pub fn get(&self) -> Option<std::cell::Ref<T>> {
        Some(std::cell::Ref::map(self.0.borrow(), |x| {
            x.as_ref().unwrap()
        }))
    }
}

enum RecursiveInner<T> {
    Owned(Rc<T>),
    Unowned(Weak<T>),
}

/// A parser that can be defined in terms of itself by separating its [declaration](Recursive::declare) from its
/// [definition](Recursive::define).
///
/// Prefer to use [`recursive()`], which exists as a convenient wrapper around both operations, if possible.
pub struct Recursive<'a, I, O, E: Error<I>, S = ()>(
    RecursiveInner<OnceCell<Box<dyn Parser<I, O, S, Error = E> + 'a>>>,
);

impl<'a, I: Clone, O, S, E: Error<I>> Recursive<'a, I, O, E, S> {
    fn cell(&self) -> Rc<OnceCell<Box<dyn Parser<I, O, S, Error = E> + 'a>>> {
        match &self.0 {
            RecursiveInner::Owned(x) => x.clone(),
            RecursiveInner::Unowned(x) => x
                .upgrade()
                .expect("Recursive parser used before being defined"),
        }
    }

    /// Declare the existence of a recursive parser, allowing it to be used to construct parser combinators before
    /// being fulled defined.
    ///
    /// Declaring a parser before defining it is required for a parser to reference itself.
    ///
    /// This should be followed by **exactly one** call to the [`Recursive::define`] method prior to using the parser
    /// for parsing (i.e: via the [`Parser::parse`] method or similar).
    ///
    /// Prefer to use [`recursive()`], which is a convenient wrapper around this method and [`Recursive::define`], if
    /// possible.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::prelude::*;
    /// #[derive(Debug, PartialEq)]
    /// enum Chain {
    ///     End,
    ///     Link(char, Box<Chain>),
    /// }
    ///
    /// // Declare the existence of the parser before defining it so that it can reference itself
    /// let mut chain = Recursive::<_, _, Simple<char>>::declare();
    ///
    /// // Define the parser in terms of itself.
    /// // In this case, the parser parses a right-recursive list of '+' into a singly linked list
    /// chain.define(just('+')
    ///     .then(chain.clone())
    ///     .map(|(c, chain)| Chain::Link(c, Box::new(chain)))
    ///     .or_not()
    ///     .map(|chain| chain.unwrap_or(Chain::End)));
    ///
    /// assert_eq!(chain.parse(""), Ok(Chain::End));
    /// assert_eq!(
    ///     chain.parse("++"),
    ///     Ok(Chain::Link('+', Box::new(Chain::Link('+', Box::new(Chain::End))))),
    /// );
    /// ```
    pub fn declare() -> Self {
        Recursive(RecursiveInner::Owned(Rc::new(OnceCell::new())))
    }

    /// Defines the parser after declaring it, allowing it to be used for parsing.
    pub fn define<P: Parser<I, O, S, Error = E> + 'a>(&mut self, parser: P) {
        self.cell()
            .set(Box::new(parser))
            .unwrap_or_else(|_| panic!("Parser defined more than once"));
    }
}

impl<'a, I: Clone, O, S, E: Error<I>> Clone for Recursive<'a, I, O, E, S> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            RecursiveInner::Owned(x) => RecursiveInner::Owned(x.clone()),
            RecursiveInner::Unowned(x) => RecursiveInner::Unowned(x.clone()),
        })
    }
}

impl<'a, I: Clone, O, S, E: Error<I>> Parser<I, O, S> for Recursive<'a, I, O, E, S> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        debugger: &mut D,
        stream: &mut StreamOf<I, Self::Error>,
    ) -> PResult<I, O, Self::Error> {
        #[allow(deprecated)]
        debugger.invoke(
            self.cell()
                .get()
                .expect("Recursive parser used before being defined")
                .as_ref(),
            stream,
        )
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, O, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, O, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// Construct a recursive parser (i.e: a parser that may contain itself as part of its pattern).
///
/// The given function must create the parser. The parser must not be used to parse input before this function returns.
///
/// This is a wrapper around [`Recursive::declare`] and [`Recursive::define`].
///
/// The output type of this parser is `O`, the same as the inner parser.
///
/// # Examples
///
/// ```
/// # use chumsky::prelude::*;
/// #[derive(Debug, PartialEq)]
/// enum Tree {
///     Leaf(String),
///     Branch(Vec<Tree>),
/// }
///
/// // Parser that recursively parses nested lists
/// let tree = recursive::<_, _, _, _, Simple<char>>(|tree| tree
///     .separated_by(just(','))
///     .delimited_by('[', ']')
///     .map(Tree::Branch)
///     .or(text::ident().map(Tree::Leaf))
///     .padded());
///
/// assert_eq!(tree.parse("hello"), Ok(Tree::Leaf("hello".to_string())));
/// assert_eq!(tree.parse("[a, b, c]"), Ok(Tree::Branch(vec![
///     Tree::Leaf("a".to_string()),
///     Tree::Leaf("b".to_string()),
///     Tree::Leaf("c".to_string()),
/// ])));
/// // The parser can deal with arbitrarily complex nested lists
/// assert_eq!(tree.parse("[[a, b], c, [d, [e, f]]]"), Ok(Tree::Branch(vec![
///     Tree::Branch(vec![
///         Tree::Leaf("a".to_string()),
///         Tree::Leaf("b".to_string()),
///     ]),
///     Tree::Leaf("c".to_string()),
///     Tree::Branch(vec![
///         Tree::Leaf("d".to_string()),
///         Tree::Branch(vec![
///             Tree::Leaf("e".to_string()),
///             Tree::Leaf("f".to_string()),
///         ]),
///     ]),
/// ])));
/// ```
pub fn recursive<
    'a,
    I: Clone,
    O,
    S,
    P: Parser<I, O, S, Error = E> + 'a,
    F: FnOnce(Recursive<'a, I, O, E, S>) -> P,
    E: Error<I>,
>(
    f: F,
) -> Recursive<'a, I, O, E, S> {
    let mut parser = Recursive::declare();
    parser.define(f(Recursive(match &parser.0 {
        RecursiveInner::Owned(x) => RecursiveInner::Unowned(Rc::downgrade(x)),
        RecursiveInner::Unowned(_) => unreachable!(),
    })));
    parser
}

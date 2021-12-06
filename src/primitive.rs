use super::*;

/// See [`custom`].
pub struct Custom<I, O, F, E>(F, PhantomData<(I, O, E)>);

impl<I, O, F: Copy, E> Copy for Custom<I, O, F, E> {}
impl<I, O, F: Clone, E> Clone for Custom<I, O, F, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone, O, S, F: Fn(&mut StreamOf<I, E>) -> PResult<I, O, E>, E: Error<I>> Parser<I, O, S>
    for Custom<I, O, F, E>
{
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, O, E> {
        (self.0)(stream)
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

/// A parser primitive that allows you to define your own custom parsers.
///
/// This is a last resort: only use this function if none of the other parser combinators suit your needs. If you find
/// yourself needing to us it, it's likely because there's an API hole that needs filling: please report your use-case
/// [on the main repository](https://github.com/zesterer/chumsky/issues/new).
///
/// That said, using this function is preferable to implementing [`Parser`] by hand because it has some quirky and
/// implementation-specific API features that are undocumented, unstable, and difficult to use correctly.
///
/// The output type of this parser is determined by the parse result of the function.
pub fn custom<I, O, F, E>(f: F) -> Custom<I, O, F, E>
where
    I: Clone,
    F: Fn(&mut StreamOf<I, E>) -> PResult<I, O, E>,
    E: Error<I>,
{
    Custom(f, PhantomData)
}

/// See [`end`].
pub struct End<E>(PhantomData<E>);

impl<E> Clone for End<E> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<I: Clone, S, E: Error<I>> Parser<I, (), S> for End<E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, (), E> {
        match stream.next() {
            (_, _, None) => (Vec::new(), Ok(((), None))),
            (at, span, found) => (
                Vec::new(),
                Err(Located::at(
                    at,
                    E::expected_input_found(span, Vec::new(), found),
                )),
            ),
        }
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, (), E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, (), E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts only the end of input.
///
/// This parser is very useful when you wish to force a parser to consume *all* of the input. It is typically combined
/// with [`Parser::then_ignore`].
///
/// The output type of this parser is `()`.
///
/// # Examples
///
/// ```
/// # use chumsky::prelude::*;
/// assert_eq!(end::<Simple<char>>().parse(""), Ok(()));
/// assert!(end::<Simple<char>>().parse("hello").is_err());
/// ```
///
/// ```
/// # use chumsky::prelude::*;
/// let digits = text::digits::<_, Simple<char>>(10);
///
/// // This parser parses digits!
/// assert_eq!(digits.parse("1234"), Ok("1234".to_string()));
///
/// // However, parsers are lazy and do not consume trailing input.
/// // This can be inconvenient if we want to validate all of the input.
/// assert_eq!(digits.parse("1234AhasjADSJAlaDJKSDAK"), Ok("1234".to_string()));
///
/// // To fix this problem, we require that the end of input follows any successfully parsed input
/// let only_digits = digits.then_ignore(end());
///
/// // Now our parser correctly produces an error if any trailing input is found...
/// assert!(only_digits.parse("1234AhasjADSJAlaDJKSDAK").is_err());
/// // ...while still behaving correctly for inputs that only consist of valid patterns
/// assert_eq!(only_digits.parse("1234"), Ok("1234".to_string()));
/// ```
pub fn end<E>() -> End<E> {
    End(PhantomData)
}

/// A utility trait to abstract over linear container-like things.
///
/// This trait is likely to change in future versions of the crate, so avoid implementing it yourself.
pub trait Container<T> {
    /// An iterator over the items within this container, by value.
    type Iter: Iterator<Item = T>;
    /// Iterate over the elements of the container (using internal iteration because GATs are unstable).
    fn get_iter(&self) -> Self::Iter;
}

impl<T: Clone> Container<T> for T {
    type Iter = std::iter::Once<T>;
    fn get_iter(&self) -> Self::Iter {
        std::iter::once(self.clone())
    }
}

impl Container<char> for String {
    type Iter = std::vec::IntoIter<char>;
    fn get_iter(&self) -> Self::Iter {
        self.chars().collect::<Vec<_>>().into_iter()
    }
}

impl<'a> Container<char> for &'a str {
    type Iter = std::str::Chars<'a>;
    fn get_iter(&self) -> Self::Iter {
        self.chars()
    }
}

impl<'a, T: Clone> Container<T> for &'a [T] {
    type Iter = std::iter::Cloned<std::slice::Iter<'a, T>>;
    fn get_iter(&self) -> Self::Iter {
        self.iter().cloned()
    }
}

impl<'a, T: Clone, const N: usize> Container<T> for &'a [T; N] {
    type Iter = std::iter::Cloned<std::slice::Iter<'a, T>>;
    fn get_iter(&self) -> Self::Iter {
        self.iter().cloned()
    }
}

impl<T: Clone, const N: usize> Container<T> for [T; N] {
    type Iter = std::array::IntoIter<T, N>;
    fn get_iter(&self) -> Self::Iter {
        std::array::IntoIter::new(self.clone())
    }
}

impl<T: Clone> Container<T> for Vec<T> {
    type Iter = std::vec::IntoIter<T>;
    fn get_iter(&self) -> Self::Iter {
        self.clone().into_iter()
    }
}

impl<T: Clone> Container<T> for std::collections::LinkedList<T> {
    type Iter = std::collections::linked_list::IntoIter<T>;
    fn get_iter(&self) -> Self::Iter {
        self.clone().into_iter()
    }
}

impl<T: Clone> Container<T> for std::collections::VecDeque<T> {
    type Iter = std::collections::vec_deque::IntoIter<T>;
    fn get_iter(&self) -> Self::Iter {
        self.clone().into_iter()
    }
}

impl<T: Clone> Container<T> for std::collections::HashSet<T> {
    type Iter = std::collections::hash_set::IntoIter<T>;
    fn get_iter(&self) -> Self::Iter {
        self.clone().into_iter()
    }
}

impl<T: Clone> Container<T> for std::collections::BTreeSet<T> {
    type Iter = std::collections::btree_set::IntoIter<T>;
    fn get_iter(&self) -> Self::Iter {
        self.clone().into_iter()
    }
}

impl<T: Clone> Container<T> for std::collections::BinaryHeap<T> {
    type Iter = std::collections::binary_heap::IntoIter<T>;
    fn get_iter(&self) -> Self::Iter {
        self.clone().into_iter()
    }
}

/// See [`just`].
pub struct Just<I, C, E>(C, PhantomData<(I, E)>);

impl<I, C: Copy, E> Copy for Just<I, C, E> {}
impl<I, C: Clone, E> Clone for Just<I, C, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone + PartialEq, S, C: Container<I> + Clone, E: Error<I>> Parser<I, C, S> for Just<I, C, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, C, E> {
        for expected in self.0.get_iter() {
            match stream.next() {
                (_, _, Some(tok)) if tok == expected => {}
                (at, span, found) => {
                    return (
                        Vec::new(),
                        Err(Located::at(
                            at,
                            E::expected_input_found(span, Some(expected.clone()), found),
                        )),
                    )
                }
            }
        }

        (Vec::new(), Ok((self.0.clone(), None)))
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, C, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, C, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts only the given input.
///
/// The output type of this parser is `C`, the input or sequence that was provided.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let question = just::<_, _, Cheap<char>>('?');
///
/// assert_eq!(question.parse("?"), Ok('?'));
/// assert!(question.parse("!").is_err());
/// // This works because parsers do not eagerly consume input, so the '!' is not parsed
/// assert_eq!(question.parse("?!"), Ok('?'));
/// // This fails because the parser expects an end to the input after the '?'
/// assert!(question.then(end()).parse("?!").is_err());
/// ```
pub fn just<I, C: Container<I>, E: Error<I>>(inputs: C) -> Just<I, C, E> {
    Just(inputs, PhantomData)
}

/// See [`seq`].
pub struct Seq<I, E>(Vec<I>, PhantomData<E>);

impl<I: Clone, E> Clone for Seq<I, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone + PartialEq, S, E: Error<I>> Parser<I, (), S> for Seq<I, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, (), E> {
        for expected in &self.0 {
            match stream.next() {
                (_, _, Some(tok)) if &tok == expected => {}
                (at, span, found) => {
                    return (
                        Vec::new(),
                        Err(Located::at(
                            at,
                            E::expected_input_found(span, Some(expected.clone()), found),
                        )),
                    )
                }
            }
        }

        (Vec::new(), Ok(((), None)))
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, (), E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, (), E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts only a sequence of specific inputs.
///
/// The output type of this parser is `()`.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let hello = seq::<_, _, Cheap<char>>("Hello".chars());
///
/// assert_eq!(hello.parse("Hello"), Ok(()));
/// assert_eq!(hello.parse("Hello, world!"), Ok(()));
/// assert!(hello.parse("Goodbye").is_err());
///
/// let onetwothree = seq::<_, _, Cheap<i32>>([1, 2, 3]);
///
/// assert_eq!(onetwothree.parse([1, 2, 3]), Ok(()));
/// assert_eq!(onetwothree.parse([1, 2, 3, 4, 5]), Ok(()));
/// assert!(onetwothree.parse([2, 1, 3]).is_err());
/// ```
#[deprecated(
    since = "0.7",
    note = "Use `just` instead: it now works for many sequence-like types!"
)]
pub fn seq<I: Clone + PartialEq, Iter: IntoIterator<Item = I>, E>(xs: Iter) -> Seq<I, E> {
    Seq(xs.into_iter().collect(), PhantomData)
}

/// See [`one_of`].
pub struct OneOf<I, C, E>(C, PhantomData<(I, E)>);

impl<I, C: Clone, E> Clone for OneOf<I, C, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone + PartialEq, S, C: Container<I>, E: Error<I>> Parser<I, I, S> for OneOf<I, C, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, I, E> {
        match stream.next() {
            (_, _, Some(tok)) if self.0.get_iter().any(|not| not == tok) => {
                (Vec::new(), Ok((tok.clone(), None)))
            }
            (at, span, found) => {
                return (
                    Vec::new(),
                    Err(Located::at(
                        at,
                        E::expected_input_found(span, self.0.get_iter(), found),
                    )),
                )
            }
        }
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, I, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, I, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts one of a sequence of specific inputs.
///
/// The output type of this parser is `I`, the input that was found.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let digits = one_of::<_, _, Cheap<char>>("0123456789")
///     .repeated().at_least(1)
///     .then_ignore(end())
///     .collect::<String>();
///
/// assert_eq!(digits.parse("48791"), Ok("48791".to_string()));
/// assert!(digits.parse("421!53").is_err());
/// ```
pub fn one_of<I, C: Container<I>, E: Error<I>>(inputs: C) -> OneOf<I, C, E> {
    OneOf(inputs, PhantomData)
}

/// See [`empty`].
pub struct Empty<E>(PhantomData<E>);

impl<E> Clone for Empty<E> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<I: Clone, E: Error<I>> Parser<I, ()> for Empty<E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        _: &mut StreamOf<I, E>,
    ) -> PResult<I, (), E> {
        (Vec::new(), Ok(((), None)))
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, (), E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, (), E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that parses no inputs.
///
/// The output type of this parser is `()`.
pub fn empty<E>() -> Empty<E> {
    Empty(PhantomData)
}

/// See [`none_of`].
pub struct NoneOf<I, C, E>(C, PhantomData<(I, E)>);

impl<I, C: Clone, E> Clone for NoneOf<I, C, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone + PartialEq, S, C: Container<I>, E: Error<I>> Parser<I, I, S> for NoneOf<I, C, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, I, E> {
        match stream.next() {
            (_, _, Some(tok)) if self.0.get_iter().all(|not| not != tok) => {
                (Vec::new(), Ok((tok.clone(), None)))
            }
            (at, span, found) => {
                return (
                    Vec::new(),
                    Err(Located::at(
                        at,
                        E::expected_input_found(span, Vec::new(), found),
                    )),
                )
            }
        }
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, I, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, I, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts any input that is *not* in a sequence of specific inputs.
///
/// The output type of this parser is `I`, the input that was found.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let string = one_of::<_, _, Cheap<char>>("\"'")
///     .ignore_then(none_of("\"'").repeated())
///     .then_ignore(one_of("\"'"))
///     .then_ignore(end())
///     .collect::<String>();
///
/// assert_eq!(string.parse("'hello'"), Ok("hello".to_string()));
/// assert_eq!(string.parse("\"world\""), Ok("world".to_string()));
/// assert!(string.parse("\"421!53").is_err());
/// ```
pub fn none_of<I, C: Container<I>, E: Error<I>>(inputs: C) -> NoneOf<I, C, E> {
    NoneOf(inputs, PhantomData)
}

/// See [`take_until`].
#[derive(Copy, Clone)]
pub struct TakeUntil<A>(A);

impl<I: Clone, O, S, A: Parser<I, O>> Parser<I, (Vec<I>, O), S> for TakeUntil<A> {
    type Error = A::Error;

    fn parse_inner<D: Debugger>(
        &self,
        debugger: &mut D,
        stream: &mut StreamOf<I, A::Error>,
    ) -> PResult<I, (Vec<I>, O), A::Error> {
        let mut outputs = Vec::new();
        let mut alt = None;

        loop {
            let (errors, err) = match stream.try_parse(|stream| {
                #[allow(deprecated)]
                self.0.parse_inner(debugger, stream)
            }) {
                (errors, Ok((out, a_alt))) => {
                    break (errors, Ok(((outputs, out), merge_alts(alt, a_alt))))
                }
                (errors, Err(err)) => (errors, err),
            };

            match stream.next() {
                (_, _, Some(tok)) => outputs.push(tok),
                (_, _, None) => break (errors, Err(err)),
            }

            alt = merge_alts(alt.take(), Some(err));
        }
    }

    fn parse_inner_verbose(
        &self,
        d: &mut Verbose,
        s: &mut StreamOf<I, A::Error>,
    ) -> PResult<I, (Vec<I>, O), A::Error> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(
        &self,
        d: &mut Silent,
        s: &mut StreamOf<I, A::Error>,
    ) -> PResult<I, (Vec<I>, O), A::Error> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts any number of inputs until a terminating pattern is reached.
///
/// The output type of this parser is `(Vec<I>, O)`, a combination of the preceding inputs and the output of the
/// final patterns.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let single_line = just::<_, _, Simple<char>>("//")
///     .then(take_until(text::newline()))
///     .ignored();
///
/// let multi_line = just::<_, _, Simple<char>>("/*")
///     .then(take_until(just("*/")))
///     .ignored();
///
/// let comment = single_line.or(multi_line);
///
/// let tokens = text::ident()
///     .padded()
///     .padded_by(comment
///         .padded()
///         .repeated())
///     .repeated();
///
/// assert_eq!(tokens.parse(r#"
///     // These tokens...
///     these are
///     /*
///         ...have some
///         multi-line...
///     */
///     // ...and single-line...
///     tokens
///     // ...comments between them
/// "#), Ok(vec!["these".to_string(), "are".to_string(), "tokens".to_string()]));
/// ```
pub fn take_until<A>(until: A) -> TakeUntil<A> {
    TakeUntil(until)
}

/// See [`filter`].
pub struct Filter<F, E>(F, PhantomData<E>);

impl<F: Copy, E> Copy for Filter<F, E> {}
impl<F: Clone, E> Clone for Filter<F, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone, S, F: Fn(&I) -> bool, E: Error<I>> Parser<I, I, S> for Filter<F, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, I, E> {
        match stream.next() {
            (_, _, Some(tok)) if (self.0)(&tok) => (Vec::new(), Ok((tok, None))),
            (at, span, found) => (
                Vec::new(),
                Err(Located::at(
                    at,
                    E::expected_input_found(span, Vec::new(), found),
                )),
            ),
        }
    }

    fn parse_inner_verbose(&self, d: &mut Verbose, s: &mut StreamOf<I, E>) -> PResult<I, I, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(&self, d: &mut Silent, s: &mut StreamOf<I, E>) -> PResult<I, I, E> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

/// A parser that accepts only inputs that match the given predicate.
///
/// The output type of this parser is `I`, the input that was found.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let lowercase = filter::<_, _, Cheap<char>>(char::is_ascii_lowercase)
///     .repeated().at_least(1)
///     .then_ignore(end())
///     .collect::<String>();
///
/// assert_eq!(lowercase.parse("hello"), Ok("hello".to_string()));
/// assert!(lowercase.parse("Hello").is_err());
/// ```
pub fn filter<I, F: Fn(&I) -> bool, E>(f: F) -> Filter<F, E> {
    Filter(f, PhantomData)
}

/// See [`filter_map`].
pub struct FilterMap<F, E>(F, PhantomData<E>);

impl<F: Copy, E> Copy for FilterMap<F, E> {}
impl<F: Clone, E> Clone for FilterMap<F, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<I: Clone, O, S, F: Fn(E::Span, I) -> Result<O, E>, E: Error<I>> Parser<I, O, S> for FilterMap<F, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        stream: &mut StreamOf<I, E>,
    ) -> PResult<I, O, E> {
        let (at, span, tok) = stream.next();
        match tok.map(|tok| (self.0)(span.clone(), tok)) {
            Some(Ok(tok)) => (Vec::new(), Ok((tok, None))),
            Some(Err(err)) => (Vec::new(), Err(Located::at(at, err))),
            None => (
                Vec::new(),
                Err(Located::at(
                    at,
                    E::expected_input_found(span, Vec::new(), None),
                )),
            ),
        }
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

/// A parser that accepts a input and tests it against the given fallible function.
///
/// This function allows integration with custom error types to allow for custom parser errors.
///
/// The output type of this parser is `I`, the input that was found.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let numeral = filter_map(|span, c: char| match c.to_digit(10) {
///     Some(x) => Ok(x),
///     None => Err(Simple::custom(span, format!("'{}' is not a digit", c))),
/// });
///
/// assert_eq!(numeral.parse("3"), Ok(3));
/// assert_eq!(numeral.parse("7"), Ok(7));
/// assert_eq!(numeral.parse("f"), Err(vec![Simple::custom(0..1, "'f' is not a digit")]));
/// ```
pub fn filter_map<I, O, F: Fn(E::Span, I) -> Result<O, E>, E: Error<I>>(f: F) -> FilterMap<F, E> {
    FilterMap(f, PhantomData)
}

/// See [`any`].
pub type Any<I, E> = Filter<fn(&I) -> bool, E>;

/// A parser that accepts any input (but not the end of input).
///
/// The output type of this parser is `I`, the input that was found.
///
/// # Examples
///
/// ```
/// # use chumsky::{prelude::*, error::Cheap};
/// let any = any::<char, Cheap<char>>();
///
/// assert_eq!(any.parse("a"), Ok('a'));
/// assert_eq!(any.parse("7"), Ok('7'));
/// assert_eq!(any.parse("\t"), Ok('\t'));
/// assert!(any.parse("").is_err());
/// ```
pub fn any<I, E>() -> Any<I, E> {
    Filter(|_| true, PhantomData)
}

/// See [`fn@todo`].
pub struct Todo<I, O, E>(PhantomData<(I, O, E)>);

/// A parser that can be used whenever you want to implement a parser later.
///
/// This parser is analagous to the [`todo!`] macro, but will produce a panic when used to parse input, not
/// immediately.
///
/// This function is useful when developing your parser, allowing you to prototype and run parts of your parser without
/// committing to implementing the entire thing immediately.
///
/// The output type of this parser is whatever you want it to be: it'll never produce output!
///
/// # Examples
///
/// ```should_panic
/// # use chumsky::prelude::*;
/// let int = just::<_, _, Simple<char>>("0x").ignore_then(todo())
///     .or(just("0b").ignore_then(text::digits(2)))
///     .or(text::int(10));
///
/// // Decimal numbers are parsed
/// assert_eq!(int.parse("12"), Ok("12".to_string()));
/// // Binary numbers are parsed
/// assert_eq!(int.parse("0b00101"), Ok("00101".to_string()));
/// // Parsing hexidecimal numbers results in a panic because the parser is unimplemented
/// int.parse("0xd4");
/// ```
pub fn todo<I, O, E>() -> Todo<I, O, E> {
    Todo(PhantomData)
}

impl<I, O, E> Copy for Todo<I, O, E> {}
impl<I, O, E> Clone for Todo<I, O, E> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<I: Clone, O, S, E: Error<I>> Parser<I, O, S> for Todo<I, O, E> {
    type Error = E;

    fn parse_inner<D: Debugger>(
        &self,
        _debugger: &mut D,
        _stream: &mut StreamOf<I, Self::Error>,
    ) -> PResult<I, O, Self::Error> {
        todo!("Attempted to use an unimplemented parser.")
    }

    fn parse_inner_verbose(
        &self,
        d: &mut Verbose,
        s: &mut StreamOf<I, Self::Error>,
    ) -> PResult<I, O, Self::Error> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
    fn parse_inner_silent(
        &self,
        d: &mut Silent,
        s: &mut StreamOf<I, Self::Error>,
    ) -> PResult<I, O, Self::Error> {
        #[allow(deprecated)]
        self.parse_inner(d, s)
    }
}

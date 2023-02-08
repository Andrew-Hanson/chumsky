//! Combinators that allow combining and extending existing parsers.
//!
//! *“Ford... you're turning into a penguin. Stop it.”*
//!
//! Although it's *sometimes* useful to be able to name their type, most of these parsers are much easier to work with
//! when accessed through their respective methods on [`Parser`].

use super::*;
use core::mem::MaybeUninit;

/// Alter the configuration of a struct using parse-time context
pub struct Configure<A, F> {
    pub(crate) parser: A,
    pub(crate) cfg: F,
}

impl<A: Copy, F: Copy> Copy for Configure<A, F> {}
impl<A: Clone, F: Clone> Clone for Configure<A, F> {
    fn clone(&self) -> Self {
        Configure {
            parser: self.parser.clone(),
            cfg: self.cfg.clone(),
        }
    }
}

impl<'a, I, O, E, A, F> Parser<'a, I, O, E> for Configure<A, F>
where
    A: Parser<'a, I, O, E>,
    F: Fn(A::Config, &E::Context) -> A::Config,
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error>
        where
            Self: Sized,
    {
        let cfg = (self.cfg)(A::Config::default(), inp.ctx());
        self.parser.go_cfg::<M>(inp, cfg)
    }

    go_extra!(O);
}

/// See [`Parser::map_slice`].
pub struct MapSlice<'a, A, I, O, E, F, U>
where
    I: Input + SliceInput + ?Sized,
    E: ParserExtra<'a, I>,
    I::Slice: 'a,
    A: Parser<'a, I, O, E>,
    F: Fn(&'a I::Slice) -> U,
{
    pub(crate) parser: A,
    pub(crate) mapper: F,
    pub(crate) phantom: PhantomData<(&'a I::Slice, O, E)>,
}

impl<'a, A: Copy, I, O, E, F: Copy, U> Copy for MapSlice<'a, A, I, O, E, F, U>
where
    I: Input + SliceInput + Sized,
    E: ParserExtra<'a, I>,
    I::Slice: 'a,
    A: Parser<'a, I, O, E>,
    F: Fn(&'a I::Slice) -> U,
{
}
impl<'a, A: Clone, I, O, E, F: Clone, U> Clone for MapSlice<'a, A, I, O, E, F, U>
where
    I: Input + SliceInput + ?Sized,
    E: ParserExtra<'a, I>,
    I::Slice: 'a,
    A: Parser<'a, I, O, E>,
    F: Fn(&'a I::Slice) -> U,
{
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            mapper: self.mapper.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, O, E, A, F, U> Parser<'a, I, U, E> for MapSlice<'a, A, I, O, E, F, U>
where
    I: Input + SliceInput + ?Sized,
    E: ParserExtra<'a, I>,
    I::Slice: 'a,
    A: Parser<'a, I, O, E>,
    F: Fn(&'a I::Slice) -> U,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, U, E::Error> {
        let before = inp.save();
        self.parser.go::<Check>(inp)?;
        let after = inp.save();

        Ok(M::bind(|| (self.mapper)(inp.slice(before..after))))
    }

    go_extra!(U);
}

/// See [`Parser::slice`]
pub struct Slice<A, O> {
    pub(crate) parser: A,
    pub(crate) phantom: PhantomData<O>,
}

impl<A: Copy, O> Copy for Slice<A, O> {}
impl<A: Clone, O> Clone for Slice<A, O> {
    fn clone(&self) -> Self {
        Slice {
            parser: self.parser.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, A, I, O, E> Parser<'a, I, &'a I::Slice, E> for Slice<A, O>
where
    A: Parser<'a, I, O, E>,
    I: Input + SliceInput + ?Sized,
    E: ParserExtra<'a, I>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, &'a I::Slice, E::Error>
    where
        Self: Sized,
    {
        let before = inp.save();
        self.parser.go::<Check>(inp)?;
        let after = inp.save();

        Ok(M::bind(|| inp.slice(before..after)))
    }

    go_extra!(&'a I::Slice);
}

/// See [`Parser::filter`].
pub struct Filter<A, F> {
    pub(crate) parser: A,
    pub(crate) filter: F,
}

impl<A: Copy + ?Sized, F: Copy> Copy for Filter<A, F> {}
impl<A: Clone, F: Clone> Clone for Filter<A, F> {
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            filter: self.filter.clone(),
        }
    }
}

impl<'a, A, I, O, E, F> Parser<'a, I, O, E> for Filter<A, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    F: Fn(&O) -> bool,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        self.parser.go::<Emit>(inp).and_then(|out| {
            if (self.filter)(&out) {
                Ok(M::bind(|| out))
            } else {
                let span = inp.span_since(before);
                Err(Located::at(
                    inp.save(),
                    E::Error::expected_found(None, None, span),
                ))
            }
        })
    }

    go_extra!(O);
}

/// See [`Parser::map`].
#[derive(Copy, Clone)]
pub struct Map<A, OA, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
    pub(crate) phantom: PhantomData<OA>,
}

impl<'a, I, O, E, A, OA, F> Parser<'a, I, O, E> for Map<A, OA, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    F: Fn(OA) -> O,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        self.parser
            .go::<M>(inp)
            .map(|out| M::map(out, &self.mapper))
    }

    go_extra!(O);
}

/// See [`Parser::map_with_span`].
#[derive(Copy, Clone)]
pub struct MapWithSpan<A, OA, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
    pub(crate) phantom: PhantomData<OA>,
}

impl<'a, I, O, E, A, OA, F> Parser<'a, I, O, E> for MapWithSpan<A, OA, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    F: Fn(OA, I::Span) -> O,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        self.parser.go::<M>(inp).map(|out| {
            M::map(out, |out| {
                let span = inp.span_since(before);
                (self.mapper)(out, span)
            })
        })
    }

    go_extra!(O);
}

/// See [`Parser::map_with_state`].
#[derive(Copy, Clone)]
pub struct MapWithState<A, OA, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
    pub(crate) phantom: PhantomData<OA>,
}

impl<'a, I, O, E, A, OA, F> Parser<'a, I, O, E> for MapWithState<A, OA, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    F: Fn(OA, I::Span, &mut E::State) -> O,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        self.parser.go::<Emit>(inp).map(|out| {
            M::bind(|| {
                let span = inp.span_since(before);
                let state = inp.state();
                (self.mapper)(out, span, state)
            })
        })
    }

    go_extra!(O);
}

/// See [`Parser::try_map`].
#[derive(Copy, Clone)]
pub struct TryMap<A, OA, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
    pub(crate) phantom: PhantomData<OA>,
}

impl<'a, I, O, E, A, OA, F> Parser<'a, I, O, E> for TryMap<A, OA, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    F: Fn(OA, I::Span) -> Result<O, E::Error>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        self.parser.go::<Emit>(inp).and_then(|out| {
            let span = inp.span_since(before);
            match (self.mapper)(out, span) {
                Ok(out) => Ok(M::bind(|| out)),
                Err(e) => Err(Located::at(inp.save(), e)),
            }
        })
    }

    go_extra!(O);
}

/// See [`Parser::try_map_with_state`].
#[derive(Copy, Clone)]
pub struct TryMapWithState<A, OA, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
    pub(crate) phantom: PhantomData<OA>,
}

impl<'a, I, O, E, A, OA, F> Parser<'a, I, O, E> for TryMapWithState<A, OA, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    F: Fn(OA, I::Span, &mut E::State) -> Result<O, E::Error>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        self.parser.go::<Emit>(inp).and_then(|out| {
            let span = inp.span_since(before);
            let state = inp.state();
            match (self.mapper)(out, span, state) {
                Ok(out) => Ok(M::bind(|| out)),
                Err(e) => Err(Located::at(inp.save(), e)),
            }
        })
    }

    go_extra!(O);
}

/// See [`Parser::to`].
pub struct To<A, OA, O, E> {
    pub(crate) parser: A,
    pub(crate) to: O,
    pub(crate) phantom: PhantomData<(OA, E)>,
}

impl<A: Copy, OA, O: Copy, E> Copy for To<A, OA, O, E> {}
impl<A: Clone, OA, O: Clone, E> Clone for To<A, OA, O, E> {
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            to: self.to.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, O, E, A, OA> Parser<'a, I, O, E> for To<A, OA, O, E>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    O: Clone,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        self.parser
            .go::<Check>(inp)
            .map(|_| M::bind(|| self.to.clone()))
    }

    go_extra!(O);
}

/// See [`Parser::ignored`].
pub struct Ignored<A, OA> {
    pub(crate) parser: A,
    pub(crate) phantom: PhantomData<OA>,
}

impl<A: Copy, OA> Copy for Ignored<A, OA> {}
impl<A: Clone, OA> Clone for Ignored<A, OA> {
    fn clone(&self) -> Self {
        Ignored { parser: self.parser.clone(), phantom: PhantomData }
    }
}

impl<'a, I, E, A, OA> Parser<'a, I, (), E> for Ignored<A, OA>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, (), E::Error> {
        self.parser.go::<Check>(inp).map(|_| M::bind(|| ()))
    }

    go_extra!(());
}

/// See [`Parser::then`].
pub struct Then<A, B, OA, OB, E> {
    pub(crate) parser_a: A,
    pub(crate) parser_b: B,
    pub(crate) phantom: PhantomData<(OA, OB, E)>,
}

impl<A: Copy, B: Copy, OA, OB, E> Copy for Then<A, B, OA, OB, E> {}
impl<A: Clone, B: Clone, OA, OB, E> Clone for Then<A, B, OA, OB, E> {
    fn clone(&self) -> Self {
        Self {
            parser_a: self.parser_a.clone(),
            parser_b: self.parser_b.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, E, A, B, OA, OB> Parser<'a, I, (OA, OB), E> for Then<A, B, OA, OB, E>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, (OA, OB), E::Error> {
        let a = self.parser_a.go::<M>(inp)?;
        let b = self.parser_b.go::<M>(inp)?;
        Ok(M::combine(a, b, |a: OA, b: OB| (a, b)))
    }

    go_extra!((OA, OB));
}

/// See [`Parser::ignore_then`].
pub struct IgnoreThen<A, B, OA, E> {
    pub(crate) parser_a: A,
    pub(crate) parser_b: B,
    pub(crate) phantom: PhantomData<(OA, E)>,
}

impl<A: Copy, B: Copy, OA, E> Copy for IgnoreThen<A, B, OA, E> {}
impl<A: Clone, B: Clone, OA, E> Clone for IgnoreThen<A, B, OA, E> {
    fn clone(&self) -> Self {
        Self {
            parser_a: self.parser_a.clone(),
            parser_b: self.parser_b.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, E, A, B, OA, OB> Parser<'a, I, OB, E> for IgnoreThen<A, B, OA, E>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OB, E::Error> {
        let _a = self.parser_a.go::<Check>(inp)?;
        let b = self.parser_b.go::<M>(inp)?;
        Ok(M::map(b, |b: OB| b))
    }

    go_extra!(OB);
}

/// See [`Parser::then_ignore`].
pub struct ThenIgnore<A, B, OB, E> {
    pub(crate) parser_a: A,
    pub(crate) parser_b: B,
    pub(crate) phantom: PhantomData<(OB, E)>,
}

impl<A: Copy, B: Copy, OB, E> Copy for ThenIgnore<A, B, OB, E> {}
impl<A: Clone, B: Clone, OB, E> Clone for ThenIgnore<A, B, OB, E> {
    fn clone(&self) -> Self {
        Self {
            parser_a: self.parser_a.clone(),
            parser_b: self.parser_b.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, E, A, B, OA, OB> Parser<'a, I, OA, E> for ThenIgnore<A, B, OB, E>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OA, E::Error> {
        let a = self.parser_a.go::<M>(inp)?;
        let _b = self.parser_b.go::<Check>(inp)?;
        Ok(M::map(a, |a: OA| a))
    }

    go_extra!(OA);
}

/// See [`Parser::then_with`].
pub struct ThenWith<A, B, OA, F, I: ?Sized, E> {
    pub(crate) parser: A,
    pub(crate) then: F,
    pub(crate) phantom: PhantomData<(B, OA, E, I)>,
}

impl<A: Copy, B, OA, F: Copy, I: ?Sized, E> Copy for ThenWith<A, B, OA, F, I, E> {}
impl<A: Clone, B, OA, F: Clone, I: ?Sized, E> Clone for ThenWith<A, B, OA, F, I, E> {
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            then: self.then.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, E, A, B, OA, OB, F> Parser<'a, I, OB, E> for ThenWith<A, B, OA, F, I, E>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
    F: Fn(OA) -> B,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OB, E::Error> {
        let before = inp.save();
        match self.parser.go::<Emit>(inp) {
            Ok(output) => {
                let then = (self.then)(output);

                let before = inp.save();
                match then.go::<M>(inp) {
                    Ok(output) => Ok(output),
                    Err(e) => {
                        inp.rewind(before);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                inp.rewind(before);
                Err(e)
            }
        }
    }

    go_extra!(OB);
}

/// See [`Parser::then_with_ctx`].
pub struct ThenWithCtx<A, B, OA, F, I: ?Sized, E> {
    pub(crate) parser: A,
    pub(crate) then: B,
    pub(crate) make_ctx: F,
    pub(crate) phantom: PhantomData<(B, OA, E, I)>,
}

impl<A: Copy, B: Copy, OA, F: Copy, I: ?Sized, E> Copy for ThenWithCtx<A, B, OA, F, I, E> {}
impl<A: Clone, B: Clone, OA, F: Clone, I: ?Sized, E> Clone for ThenWithCtx<A, B, OA, F, I, E> {
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            then: self.then.clone(),
            make_ctx: self.make_ctx.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, E, CtxN, A, B, OA, OB, F> Parser<'a, I, OB, E> for ThenWithCtx<A, B, OA, F, I, extra::Full<E::Error, E::State, CtxN>>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, extra::Full<E::Error, E::State, CtxN>>,
    F: Fn(OA, &E::Context) -> CtxN,
    CtxN: 'a,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OB, E::Error> {
        let p1 = self.parser.go::<Emit>(inp)?;
        let ctx = (self.make_ctx)(p1, inp.ctx());
        inp.with_ctx(
            ctx,
            |inp| self.then.go::<M>(inp)
        )
    }

    go_extra!(OB);
}

/// See [`Parser::delimited_by`].
#[derive(Copy, Clone)]
pub struct DelimitedBy<A, B, C, OB, OC> {
    pub(crate) parser: A,
    pub(crate) start: B,
    pub(crate) end: C,
    pub(crate) phantom: PhantomData<(OB, OC)>,
}

impl<'a, I, E, A, B, C, OA, OB, OC> Parser<'a, I, OA, E> for DelimitedBy<A, B, C, OB, OC>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
    C: Parser<'a, I, OC, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OA, E::Error> {
        let _ = self.start.go::<Check>(inp)?;
        let a = self.parser.go::<M>(inp)?;
        let _ = self.end.go::<Check>(inp)?;
        Ok(a)
    }

    go_extra!(OA);
}

/// See [`Parser::padded_by`].
#[derive(Copy, Clone)]
pub struct PaddedBy<A, B, OB> {
    pub(crate) parser: A,
    pub(crate) padding: B,
    pub(crate) phantom: PhantomData<OB>,
}

impl<'a, I, E, A, B, OA, OB> Parser<'a, I, OA, E> for PaddedBy<A, B, OB>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OA, E::Error> {
        let _ = self.padding.go::<Check>(inp)?;
        let a = self.parser.go::<M>(inp)?;
        let _ = self.padding.go::<Check>(inp)?;
        Ok(a)
    }

    go_extra!(OA);
}

/// See [`Parser::or`].
#[derive(Copy, Clone)]
pub struct Or<A, B> {
    pub(crate) parser_a: A,
    pub(crate) parser_b: B,
}

impl<'a, I, O, E, A, B> Parser<'a, I, O, E> for Or<A, B>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    B: Parser<'a, I, O, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        match self.parser_a.go::<M>(inp) {
            Ok(out) => Ok(out),
            Err(ea) => {
                // TODO: prioritise errors
                inp.rewind(before);
                match self.parser_b.go::<M>(inp) {
                    Ok(out) => Ok(out),
                    Err(eb) => Err(ea.prioritize(eb, |a, b| a.merge(b))),
                }
            }
        }
    }

    go_extra!(O);
}

/// See [`Parser::recover_with`].
#[derive(Copy, Clone)]
pub struct RecoverWith<A, F> {
    pub(crate) parser: A,
    pub(crate) fallback: F,
}

impl<'a, I, O, E, A, F> Parser<'a, I, O, E> for RecoverWith<A, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    F: Parser<'a, I, O, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        match self.parser.go::<M>(inp) {
            Ok(out) => Ok(out),
            Err(e) => {
                inp.rewind(before);
                match self.fallback.go::<M>(inp) {
                    Ok(out) => {
                        inp.emit(e.err);
                        Ok(out)
                    }
                    Err(_) => Err(e),
                }
            }
        }
    }

    go_extra!(O);
}

/// TODO
#[derive(Default)]
pub struct RepeatedCfg {
    at_least: Option<usize>,
    at_most: Option<usize>,
}

impl RepeatedCfg {
    /// TODO
    pub fn at_least(mut self, n: usize) -> Self {
        self.at_least = Some(n);
        self
    }

    /// TODO
    pub fn at_most(mut self, n: usize) -> Self {
        self.at_most = Some(n);
        self
    }

    /// TODO
    pub fn exactly(mut self, n: usize) -> Self {
        self.at_least = Some(n);
        self.at_most = Some(n);
        self
    }
}

/// See [`Parser::repeated`].
// FIXME: why C has default value?
pub struct Repeated<A, OA, I: ?Sized, E, C = ()> {
    pub(crate) parser: A,
    pub(crate) at_least: usize,
    pub(crate) at_most: Option<usize>,
    pub(crate) phantom: PhantomData<(OA, E, C, I)>,
}

impl<A: Copy, OA, I: ?Sized, C, E> Copy for Repeated<A, OA, I, E, C> {}
impl<A: Clone, OA, I: ?Sized, C, E> Clone for Repeated<A, OA, I, E, C> {
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            at_least: self.at_least,
            at_most: self.at_most,
            phantom: PhantomData,
        }
    }
}

impl<'a, A, OA, I, C, E> Repeated<A, OA, I, E, C>
where
    A: Parser<'a, I, OA, E>,
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
{
    /// Require that the pattern appear at least a minimum number of times.
    pub fn at_least(self, at_least: usize) -> Self {
        Self { at_least, ..self }
    }

    /// Require that the pattern appear at most a maximum number of times.
    pub fn at_most(self, at_most: usize) -> Self {
        Self {
            at_most: Some(at_most),
            ..self
        }
    }

    /// Require that the pattern appear exactly the given number of times. If the value provided
    /// is constant, consider instead using [`Parser::repeated_exactly`]
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let ring = just::<_, _, extra::Err<Simple<str>>>('O');
    ///
    /// let for_the_elves = ring
    ///     .repeated()
    ///     .exactly(3)
    ///     .collect::<Vec<_>>();
    ///
    /// let for_the_dwarves = ring
    ///     .repeated()
    ///     .exactly(6)
    ///     .collect::<Vec<_>>();
    ///
    /// let for_the_humans = ring
    ///     .repeated()
    ///     .exactly(9)
    ///     .collect::<Vec<_>>();
    ///
    /// let for_sauron = ring
    ///     .repeated()
    ///     .exactly(1)
    ///     .collect::<Vec<_>>();
    ///
    /// let rings = for_the_elves
    ///     .then(for_the_dwarves)
    ///     .then(for_the_humans)
    ///     .then(for_sauron)
    ///     .then_ignore(end());
    ///
    /// assert!(rings.parse("OOOOOOOOOOOOOOOOOO").has_errors()); // Too few rings!
    /// assert!(rings.parse("OOOOOOOOOOOOOOOOOOOO").has_errors()); // Too many rings!
    /// // The perfect number of rings
    /// assert_eq!(
    ///     rings.parse("OOOOOOOOOOOOOOOOOOO").into_result(),
    ///     Ok(((((vec!['O'; 3]), vec!['O'; 6]), vec!['O'; 9]), vec!['O'; 1])),
    /// );
    /// ````
    pub fn exactly(self, exactly: usize) -> Self {
        Self {
            at_least: exactly,
            at_most: Some(exactly),
            ..self
        }
    }

    /// Set the type of [`Container`] to collect into.
    pub fn collect<D: Container<OA>>(self) -> Repeated<A, OA, I, E, D>
    where
        A: Parser<'a, I, OA, E>,
    {
        Repeated {
            parser: self.parser,
            at_least: self.at_least,
            at_most: self.at_most,
            phantom: PhantomData,
        }
    }

    /// Output the number of items parsed.
    ///
    /// This is sugar for [`.collect::<usize>()`](Self::collect).
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    ///
    /// // Counts how many chess squares are in the input.
    /// let squares = one_of::<_, _, extra::Err<Simple<str>>>('a'..='z').then(one_of('1'..='8')).padded().repeated().count();
    ///
    /// assert_eq!(squares.parse("a1 b2 c3").into_result(), Ok(3));
    /// assert_eq!(squares.parse("e5 e7 c6 c7 f6 d5 e6 d7 e4 c5 d6 c4 b6 f5").into_result(), Ok(14));
    /// assert_eq!(squares.parse("").into_result(), Ok(0));
    /// ```
    pub fn count(self) -> Repeated<A, OA, I, E, usize>
    where
        A: Parser<'a, I, OA, E>,
    {
        self.collect()
    }
}

impl<'a, I, E, A, OA, C> Parser<'a, I, C, E> for Repeated<A, OA, I, E, C>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    C: Container<OA>,
{
    type Config = RepeatedCfg;

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, C, E::Error> {
        Self::go_cfg::<M>(self, inp, RepeatedCfg::default())
    }

    fn go_cfg<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>, cfg: Self::Config) -> PResult<M, C, E::Error> {
        let mut count = 0;
        let mut output = M::bind::<C, _>(|| C::default());

        let at_least = cfg.at_least.unwrap_or(self.at_least);
        let at_most = cfg.at_most.or(self.at_most);

        loop {
            let before = inp.save();
            match self.parser.go::<M>(inp) {
                Ok(item) => {
                    output = M::combine(output, item, |mut output: C, item| {
                        output.push(item);
                        output
                    });
                    count += 1;

                    if let Some(at_most) = at_most {
                        if count >= at_most {
                            break Ok(output);
                        }
                    }
                }
                Err(e) => {
                    inp.rewind(before);
                    break if count >= at_least {
                        Ok(output)
                    } else {
                        Err(e)
                    };
                }
            }
        }
    }

    go_extra!(C);
}

/// See [`Parser::separated_by`].
pub struct SeparatedBy<A, B, OA, OB, I: ?Sized, E, C = ()> {
    pub(crate) parser: A,
    pub(crate) separator: B,
    pub(crate) at_least: usize,
    pub(crate) at_most: Option<usize>,
    pub(crate) allow_leading: bool,
    pub(crate) allow_trailing: bool,
    pub(crate) phantom: PhantomData<(OA, OB, C, E, I)>,
}

impl<A: Copy, B: Copy, OA, OB, I: ?Sized, E, C> Copy for SeparatedBy<A, B, OA, OB, I, E, C> {}
impl<A: Clone, B: Clone, OA, OB, I: ?Sized, E, C> Clone
    for SeparatedBy<A, B, OA, OB, I, E, C>
{
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            separator: self.separator.clone(),
            at_least: self.at_least,
            at_most: self.at_most,
            allow_leading: self.allow_leading,
            allow_trailing: self.allow_trailing,
            phantom: PhantomData,
        }
    }
}

impl<'a, A, B, OA, OB, I, C, E> SeparatedBy<A, B, OA, OB, I, E, C>
where
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
{
    /// Require that the pattern appear at least a minimum number of times.
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let numbers = just::<_, _, extra::Err<Simple<str>>>('-')
    ///     .separated_by(just('.'))
    ///     .at_least(2)
    ///     .collect::<Vec<_>>();
    ///
    /// assert!(numbers.parse("").has_errors());
    /// assert!(numbers.parse("-").has_errors());
    /// assert_eq!(numbers.parse("-.-").into_result(), Ok(vec!['-', '-']));
    /// ````
    pub fn at_least(self, at_least: usize) -> Self {
        Self { at_least, ..self }
    }

    /// Require that the pattern appear at most a maximum number of times.
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let row_4 = text::int::<_, _, extra::Err<Simple<str>>>(10)
    ///     .padded()
    ///     .separated_by(just(','))
    ///     .at_most(4)
    ///     .collect::<Vec<_>>();
    ///
    /// let matrix_4x4 = row_4
    ///     .separated_by(just(','))
    ///     .at_most(4)
    ///     .collect::<Vec<_>>();
    ///
    /// assert_eq!(
    ///     matrix_4x4.parse("0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15").into_result(),
    ///     Ok(vec![
    ///         vec!["0", "1", "2", "3"],
    ///         vec!["4", "5", "6", "7"],
    ///         vec!["8", "9", "10", "11"],
    ///         vec!["12", "13", "14", "15"],
    ///     ]),
    /// );
    /// ````
    pub fn at_most(self, at_most: usize) -> Self {
        Self {
            at_most: Some(at_most),
            ..self
        }
    }

    /// Require that the pattern appear exactly the given number of times. If the value provided is
    /// constant, consider instead using [`Parser::separated_by_exactly`].
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let coordinate_3d = text::int::<_, _, extra::Err<Simple<str>>>(10)
    ///     .padded()
    ///     .separated_by(just(','))
    ///     .exactly(3)
    ///     .collect::<Vec<_>>()
    ///     .then_ignore(end());
    ///
    /// // Not enough elements
    /// assert!(coordinate_3d.parse("4, 3").has_errors());
    /// // Too many elements
    /// assert!(coordinate_3d.parse("7, 2, 13, 4").has_errors());
    /// // Just the right number of elements
    /// assert_eq!(coordinate_3d.parse("5, 0, 12").into_result(), Ok(vec!["5", "0", "12"]));
    /// ````
    pub fn exactly(self, exactly: usize) -> Self {
        Self {
            at_least: exactly,
            at_most: Some(exactly),
            ..self
        }
    }

    /// Allow a leading separator to appear before the first item.
    ///
    /// Note that even if no items are parsed, a leading separator *is* permitted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let r#enum = text::keyword::<_, _, _, extra::Err<Simple<str>>>("enum")
    ///     .padded()
    ///     .ignore_then(text::ident()
    ///         .padded()
    ///         .separated_by(just('|'))
    ///         .allow_leading()
    ///         .collect::<Vec<_>>());
    ///
    /// assert_eq!(r#enum.parse("enum True | False").into_result(), Ok(vec!["True", "False"]));
    /// assert_eq!(r#enum.parse("
    ///     enum
    ///     | True
    ///     | False
    /// ").into_result(), Ok(vec!["True", "False"]));
    /// ```
    pub fn allow_leading(self) -> Self {
        Self {
            allow_leading: true,
            ..self
        }
    }

    /// Allow a trailing separator to appear after the last item.
    ///
    /// Note that if no items are parsed, no leading separator is permitted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let numbers = text::int::<_, _, extra::Err<Simple<str>>>(10)
    ///     .padded()
    ///     .separated_by(just(','))
    ///     .allow_trailing()
    ///     .collect::<Vec<_>>()
    ///     .delimited_by(just('('), just(')'));
    ///
    /// assert_eq!(numbers.parse("(1, 2)").into_result(), Ok(vec!["1", "2"]));
    /// assert_eq!(numbers.parse("(1, 2,)").into_result(), Ok(vec!["1", "2"]));
    /// ```
    pub fn allow_trailing(self) -> Self {
        Self {
            allow_trailing: true,
            ..self
        }
    }

    /// Set the type of [`Container`] to collect into.
    pub fn collect<D: Container<OA>>(self) -> SeparatedBy<A, B, OA, OB, I, E, D>
    where
        A: Parser<'a, I, OA, E>,
        B: Parser<'a, I, OB, E>,
    {
        SeparatedBy {
            parser: self.parser,
            separator: self.separator,
            at_least: self.at_least,
            at_most: self.at_most,
            allow_leading: self.allow_leading,
            allow_trailing: self.allow_trailing,
            phantom: PhantomData,
        }
    }

    /// Output the number of items parsed.
    ///
    /// This is sugar for [`.collect::<usize>()`](Self::collect).
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    ///
    /// // Counts how many chess squares are in the input.
    /// let squares = one_of::<_, _, extra::Err<Simple<str>>>('a'..='z').then(one_of('1'..='8')).separated_by(just(',')).allow_trailing().count();
    ///
    /// assert_eq!(squares.parse("a1,b2,c3,").into_result(), Ok(3));
    /// assert_eq!(squares.parse("e5,e7,c6,c7,f6,d5,e6,d7,e4,c5,d6,c4,b6,f5").into_result(), Ok(14));
    /// assert_eq!(squares.parse("").into_result(), Ok(0));
    /// ```
    pub fn count(self) -> SeparatedBy<A, B, OA, OB, I, E, usize>
    where
        A: Parser<'a, I, OA, E>,
        B: Parser<'a, I, OB, E>,
    {
        self.collect()
    }
}

impl<'a, I, E, A, B, OA, OB, C> Parser<'a, I, C, E> for SeparatedBy<A, B, OA, OB, I, E, C>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
    C: Container<OA>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, C, E::Error> {
        // STEPS:
        // 1. If allow_leading -> Consume separator if there
        //    if Ok  -> continue
        //    if Err -> rewind and continue
        //
        // 2. Consume item
        //    if Ok -> add to output and continue
        //    if Err && count >= self.at_least -> rewind and return output
        //    if Err && count < self.at_least -> rewind and return Err
        //
        // 3. Consume separator
        //    if Ok => continue
        //    if Err && count >= self.at_least => rewind and break
        //    if Err && count < self.at_least => rewind and return Err
        //
        // 4. Consume item
        //    if Ok && count >= self.at_most -> add to output and break
        //    if Ok && count < self.at_most -> add to output and continue
        //    if Err && count >= self.at_least => rewind and break
        //    if Err && count < self.at_least => rewind and return Err
        //
        // 5. Goto 3 until 'break'
        //
        // 6. If allow_trailing -> Consume separator
        //    if Ok -> continue
        //    if Err -> rewind and continue
        //
        // 7. Return output

        // Setup
        let mut count = 0;
        let mut output = M::bind::<C, _>(|| C::default());

        // Step 1
        if self.allow_leading {
            let before_separator = inp.save();
            if let Err(_) = self.separator.go::<Check>(inp) {
                inp.rewind(before_separator);
            }
        }

        // Step 2
        let before = inp.save();
        match self.parser.go::<M>(inp) {
            Ok(item) => {
                output = M::combine(output, item, |mut output: C, item| {
                    output.push(item);
                    output
                });
                count += 1;
            }
            Err(..) if self.at_least == 0 => {
                inp.rewind(before);
                return Ok(output);
            }
            Err(err) => {
                inp.rewind(before);
                return Err(err);
            }
        }

        loop {
            // Step 3
            let before_separator = inp.save();
            match self.separator.go::<Check>(inp) {
                Ok(..) => {
                    // Do nothing
                }
                Err(err) if count < self.at_least => {
                    inp.rewind(before_separator);
                    return Err(err);
                }
                Err(..) => {
                    inp.rewind(before_separator);
                    break;
                }
            }

            // Step 4
            match self.parser.go::<M>(inp) {
                Ok(item) => {
                    output = M::combine(output, item, |mut output: C, item| {
                        output.push(item);
                        output
                    });
                    count += 1;

                    if self.at_most.map_or(false, |max| count >= max) {
                        break;
                    } else {
                        continue;
                    }
                }
                Err(err) if count < self.at_least => {
                    // We have errored before we have reached the count,
                    // and therefore should return this error, as we are
                    // still expecting items
                    inp.rewind(before_separator);
                    return Err(err);
                }
                Err(..) => {
                    // We are not expecting any more items, so it is okay
                    // for it to fail, though if it does, we shouldn't have
                    // consumed the separator, so we need to rewind to it.
                    inp.rewind(before_separator);
                    break;
                }
            }

            // Step 5
            // continue
        }

        // Step 6
        if self.allow_trailing {
            let before_separator = inp.save();
            if let Err(_) = self.separator.go::<Check>(inp) {
                inp.rewind(before_separator);
            }
        }

        // Step 7
        Ok(output)
    }

    go_extra!(C);
}

/// See [`Parser::or_not`].
#[derive(Copy, Clone)]
pub struct OrNot<A> {
    pub(crate) parser: A,
}

impl<'a, I, O, E, A> Parser<'a, I, Option<O>, E> for OrNot<A>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, Option<O>, E::Error> {
        let before = inp.save();
        Ok(match self.parser.go::<M>(inp) {
            Ok(o) => M::map::<O, _, _>(o, Some),
            Err(_) => {
                inp.rewind(before);
                M::bind::<Option<O>, _>(|| None)
            }
        })
    }

    go_extra!(Option<O>);
}

/// See [`Parser::not`].
#[derive(Copy, Clone)]
pub struct Not<A, OA> {
    pub(crate) parser: A,
    pub(crate) phantom: PhantomData<OA>,
}

impl<'a, I, E, A, OA> Parser<'a, I, (), E> for Not<A, OA>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, (), E::Error> {
        let before = inp.save();

        let result = self.parser.go::<Check>(inp);
        inp.rewind(before);

        match result {
            Ok(_) => {
                let (at, tok) = inp.next();
                Err(Located::at(
                    at,
                    E::Error::expected_found(None, tok, inp.span_since(before)),
                ))
            }
            Err(_) => Ok(M::bind(|| ())),
        }
    }

    go_extra!(());
}

/// See [`Parser::and_is`].
#[derive(Copy, Clone)]
pub struct AndIs<A, B, OB> {
    pub(crate) parser_a: A,
    pub(crate) parser_b: B,
    pub(crate) phantom: PhantomData<OB>,
}

impl<'a, I, E, A, B, OA, OB> Parser<'a, I, OA, E> for AndIs<A, B, OB>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, OA, E::Error> {
        let before = inp.save();
        match self.parser_a.go::<M>(inp) {
            Ok(out) => {
                // A succeeded -- go back to the beginning and try B
                let after = inp.save();
                inp.rewind(before);

                match self.parser_b.go::<Check>(inp) {
                    Ok(_) => {
                        // B succeeded -- go to the end of A and return its output
                        inp.rewind(after);
                        Ok(out)
                    }
                    Err(e) => {
                        // B failed -- go back to the beginning and fail
                        inp.rewind(before);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // A failed -- go back to the beginning and fail
                inp.rewind(before);
                Err(e)
            }
        }
    }

    go_extra!(OA);
}

/// See [`Parser::repeated_exactly`].
#[derive(Copy, Clone)]
pub struct RepeatedExactly<A, OA, C, const N: usize> {
    pub(crate) parser: A,
    pub(crate) phantom: PhantomData<(OA, C)>,
}

impl<A, OA, C, const N: usize> RepeatedExactly<A, OA, C, N> {
    /// Set the type of [`ContainerExactly`] to collect into.
    pub fn collect<'a, I, E, D>(self) -> RepeatedExactly<A, OA, D, N>
    where
        A: Parser<'a, I, OA, E>,
        I: Input + ?Sized,
        E: ParserExtra<'a, I>,
        D: ContainerExactly<OA, N>,
    {
        RepeatedExactly {
            parser: self.parser,
            phantom: PhantomData,
        }
    }
}

impl<'a, I, E, A, OA, C, const N: usize> Parser<'a, I, C, E> for RepeatedExactly<A, OA, C, N>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    C: ContainerExactly<OA, N>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, C, E::Error> {
        let mut i = 0;
        let mut output = M::bind(|| C::uninit());
        loop {
            let before = inp.save();
            match self.parser.go::<M>(inp) {
                Ok(out) => {
                    output = M::map(output, |mut output| {
                        M::map(out, |out| {
                            C::write(&mut output, i, out);
                        });
                        output
                    });
                    i += 1;
                    if i == N {
                        // SAFETY: All entries with an index < i are filled
                        break Ok(M::map(output, |output| unsafe { C::take(output) }));
                    }
                }
                Err(e) => {
                    inp.rewind(before);
                    // SAFETY: All entries with an index < i are filled
                    unsafe {
                        M::map(output, |mut output| C::drop_before(&mut output, i));
                    }
                    break Err(e);
                }
            }
        }
    }

    go_extra!(C);
}

/// See [`Parser::separated_by_exactly`].
#[derive(Copy, Clone)]
pub struct SeparatedByExactly<A, B, OB, C, const N: usize> {
    pub(crate) parser: A,
    pub(crate) separator: B,
    pub(crate) allow_leading: bool,
    pub(crate) allow_trailing: bool,
    pub(crate) phantom: PhantomData<(OB, C)>,
}

impl<A, B, OB, C, const N: usize> SeparatedByExactly<A, B, OB, C, N> {
    /// Allow a leading separator to appear before the first item.
    ///
    /// Note that even if no items are parsed, a leading separator *is* permitted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let r#enum = text::keyword::<_, _, _, extra::Err<Simple<str>>>("enum")
    ///     .padded()
    ///     .ignore_then(text::ident()
    ///         .padded()
    ///         .separated_by(just('|'))
    ///         .allow_leading()
    ///         .collect::<Vec<_>>());
    ///
    /// assert_eq!(r#enum.parse("enum True | False").into_result(), Ok(vec!["True", "False"]));
    /// assert_eq!(r#enum.parse("
    ///     enum
    ///     | True
    ///     | False
    /// ").into_result(), Ok(vec!["True", "False"]));
    /// ```
    pub fn allow_leading(self) -> Self {
        Self {
            allow_leading: true,
            ..self
        }
    }

    /// Allow a trailing separator to appear after the last item.
    ///
    /// Note that if no items are parsed, no trailing separator is permitted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chumsky::zero_copy::prelude::*;
    /// let numbers = text::int::<_, _, extra::Err<Simple<str>>>(10)
    ///     .padded()
    ///     .separated_by(just(','))
    ///     .allow_trailing()
    ///     .collect::<Vec<_>>()
    ///     .delimited_by(just('('), just(')'));
    ///
    /// assert_eq!(numbers.parse("(1, 2)").into_result(), Ok(vec!["1", "2"]));
    /// assert_eq!(numbers.parse("(1, 2,)").into_result(), Ok(vec!["1", "2"]));
    /// ```
    pub fn allow_trailing(self) -> Self {
        Self {
            allow_trailing: true,
            ..self
        }
    }

    /// Set the type of [`ContainerExactly`] to collect into.
    pub fn collect<'a, I, OA, E, D>(self) -> SeparatedByExactly<A, B, OB, D, N>
    where
        A: Parser<'a, I, OA, E>,
        I: Input,
        E: ParserExtra<'a, I>,
        D: ContainerExactly<OA, N>,
    {
        SeparatedByExactly {
            parser: self.parser,
            separator: self.separator,
            allow_leading: self.allow_leading,
            allow_trailing: self.allow_trailing,
            phantom: PhantomData,
        }
    }
}

// FIXME: why parser output is not C ?
impl<'a, I, E, A, B, OA, OB, C, const N: usize> Parser<'a, I, [OA; N], E>
    for SeparatedByExactly<A, B, OB, C, N>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    B: Parser<'a, I, OB, E>,
    C: ContainerExactly<OA, N>,
{
    type Config = ();

    // FIXME: why parse result output is not C ?
    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, [OA; N], E::Error> {
        if self.allow_leading {
            let before_separator = inp.save();
            if let Err(_) = self.separator.go::<Check>(inp) {
                inp.rewind(before_separator);
            }
        }

        let mut i = 0;
        let mut output = <MaybeUninit<_> as MaybeUninitExt<_>>::uninit_array();
        loop {
            let before = inp.save();
            match self.parser.go::<M>(inp) {
                Ok(out) => {
                    output[i].write(out);
                    i += 1;
                    if i == N {
                        if self.allow_trailing {
                            let before_separator = inp.save();
                            if let Err(_) = self.separator.go::<Check>(inp) {
                                inp.rewind(before_separator);
                            }
                        }

                        // SAFETY: All entries with an index < i are filled
                        break Ok(M::array::<OA, N>(unsafe {
                            MaybeUninitExt::array_assume_init(output)
                        }));
                    } else {
                        let before_separator = inp.save();
                        if let Err(e) = self.separator.go::<Check>(inp) {
                            inp.rewind(before_separator);
                            // SAFETY: All entries with an index < i are filled
                            output[..i]
                                .iter_mut()
                                .for_each(|o| unsafe { o.assume_init_drop() });
                            break Err(e);
                        }
                    }
                }
                Err(e) => {
                    inp.rewind(before);
                    // SAFETY: All entries with an index < i are filled
                    output[..i]
                        .iter_mut()
                        .for_each(|o| unsafe { o.assume_init_drop() });
                    break Err(e);
                }
            }
        }
    }

    go_extra!([OA; N]);
}

/// See [`Parser::foldr`].
pub struct Foldr<P, F, A, B, E> {
    pub(crate) parser: P,
    pub(crate) folder: F,
    pub(crate) phantom: PhantomData<(A, B, E)>,
}

impl<P: Copy, F: Copy, A, B, E> Copy for Foldr<P, F, A, B, E> {}
impl<P: Clone, F: Clone, A, B, E> Clone for Foldr<P, F, A, B, E> {
    fn clone(&self) -> Self {
        Foldr {
            parser: self.parser.clone(),
            folder: self.folder.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, P, F, A, B, E> Parser<'a, I, B, E> for Foldr<P, F, A, B, E>
where
    I: Input + ?Sized,
    P: Parser<'a, I, (A, B), E>,
    E: ParserExtra<'a, I>,
    A: IntoIterator,
    A::IntoIter: DoubleEndedIterator,
    F: Fn(A::Item, B) -> B,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, B, E::Error>
    where
        Self: Sized,
    {
        self.parser.go::<M>(inp).map(|out| {
            M::map(out, |(init, end)| {
                init.into_iter().rfold(end, |b, a| (self.folder)(a, b))
            })
        })
    }

    go_extra!(B);
}

/// See [`Parser::foldl`].
pub struct Foldl<P, F, A, B, E> {
    pub(crate) parser: P,
    pub(crate) folder: F,
    pub(crate) phantom: PhantomData<(A, B, E)>,
}

impl<P: Copy, F: Copy, A, B, E> Copy for Foldl<P, F, A, B, E> {}
impl<P: Clone, F: Clone, A, B, E> Clone for Foldl<P, F, A, B, E> {
    fn clone(&self) -> Self {
        Foldl {
            parser: self.parser.clone(),
            folder: self.folder.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, P, F, A, B, E> Parser<'a, I, A, E> for Foldl<P, F, A, B, E>
where
    I: Input + ?Sized,
    P: Parser<'a, I, (A, B), E>,
    E: ParserExtra<'a, I>,
    B: IntoIterator,
    F: Fn(A, B::Item) -> A,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, A, E::Error>
    where
        Self: Sized,
    {
        self.parser.go::<M>(inp).map(|out| {
            M::map(out, |(head, tail)| {
                tail.into_iter().fold(head, &self.folder)
            })
        })
    }

    go_extra!(A);
}

/// See [`Parser::rewind`].
#[derive(Copy, Clone)]
pub struct Rewind<A> {
    pub(crate) parser: A,
}

impl<'a, I, O, E, A> Parser<'a, I, O, E> for Rewind<A>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error> {
        let before = inp.save();
        match self.parser.go::<M>(inp) {
            Ok(o) => {
                inp.rewind(before);
                Ok(o)
            }
            Err(e) => Err(e),
        }
    }

    go_extra!(O);
}

/// See [`Parser::map_err`].
#[derive(Copy, Clone)]
pub struct MapErr<A, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
}

impl<'a, I, O, E, A, F> Parser<'a, I, O, E> for MapErr<A, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    F: Fn(E::Error) -> E::Error,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error>
    where
        Self: Sized,
    {
        self.parser.go::<M>(inp).map_err(|mut e| {
            e.err = (self.mapper)(e.err);
            e
        })
    }

    go_extra!(O);
}

/// See [`Parser::map_err_with_span`].
#[derive(Copy, Clone)]
pub struct MapErrWithSpan<A, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
}

impl<'a, I, O, E, A, F> Parser<'a, I, O, E> for MapErrWithSpan<A, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    F: Fn(E::Error, I::Span) -> E::Error,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error>
    where
        Self: Sized,
    {
        let start = inp.save();
        self.parser.go::<M>(inp).map_err(|mut e| {
            let span = inp.span_since(start);
            e.err = (self.mapper)(e.err, span);
            e
        })
    }

    go_extra!(O);
}

/// See [`Parser::map_err_with_state`].
#[derive(Copy, Clone)]
pub struct MapErrWithState<A, F> {
    pub(crate) parser: A,
    pub(crate) mapper: F,
}

impl<'a, I, O, E, A, F> Parser<'a, I, O, E> for MapErrWithState<A, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    F: Fn(E::Error, I::Span, &mut E::State) -> E::Error,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error>
    where
        Self: Sized,
    {
        let start = inp.save();
        self.parser.go::<M>(inp).map_err(|mut e| {
            let span = inp.span_since(start);
            e.err = (self.mapper)(e.err, span, inp.state());
            e
        })
    }

    go_extra!(O);
}

/// See [`Parser::validate`]
pub struct Validate<A, OA, F> {
    pub(crate) parser: A,
    pub(crate) validator: F,
    pub(crate) phantom: PhantomData<OA>,
}

impl<A: Copy, OA, F: Copy> Copy for Validate<A, OA, F> {}
impl<A: Clone, OA, F: Clone> Clone for Validate<A, OA, F> {
    fn clone(&self) -> Self {
        Validate {
            parser: self.parser.clone(),
            validator: self.validator.clone(),
            phantom: PhantomData,
        }
    }
}

impl<'a, I, OA, U, E, A, F> Parser<'a, I, U, E> for Validate<A, OA, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, OA, E>,
    F: Fn(OA, I::Span, &mut Emitter<E::Error>) -> U,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, U, E::Error>
    where
        Self: Sized,
    {
        let before = inp.save();
        self.parser.go::<Emit>(inp).map(|out| {
            let span = inp.span_since(before);
            let mut emitter = Emitter::new();
            let out = (self.validator)(out, span, &mut emitter);
            for err in emitter.errors() {
                inp.emit(err);
            }
            M::bind(|| out)
        })
    }

    go_extra!(U);
}

/// See [`Parser::or_else`].
#[derive(Copy, Clone)]
pub struct OrElse<A, F> {
    pub(crate) parser: A,
    pub(crate) or_else: F,
}

impl<'a, I, O, E, A, F> Parser<'a, I, O, E> for OrElse<A, F>
where
    I: Input + ?Sized,
    E: ParserExtra<'a, I>,
    A: Parser<'a, I, O, E>,
    F: Fn(E::Error) -> Result<O, E::Error>,
{
    type Config = ();

    fn go<M: Mode>(&self, inp: &mut InputRef<'a, '_, I, E>) -> PResult<M, O, E::Error>
    where
        Self: Sized,
    {
        match self.parser.go::<M>(inp) {
            Ok(o) => Ok(o),
            Err(err) => match (self.or_else)(err.err) {
                Err(e) => Err(Located {
                    pos: err.pos,
                    err: e,
                }),
                Ok(out) => Ok(M::bind(|| out)),
            },
        }
    }

    go_extra!(O);
}

#[cfg(test)]
mod tests {
    use crate::zero_copy::prelude::*;

    #[test]
    fn separated_by_at_least() {
        let parser = just::<_, _, EmptyErr, ()>('-')
            .separated_by(just(','))
            .at_least(3)
            .collect();

        assert_eq!(parser.parse("-,-,-").into_result(), Ok(vec!['-', '-', '-']));
    }

    #[test]
    fn separated_by_at_least_without_leading() {
        let parser = just::<_, _, EmptyErr, ()>('-')
            .separated_by(just(','))
            .at_least(3)
            .collect::<Vec<_>>();

        // Is empty means no errors
        assert!(parser.parse(",-,-,-").has_errors());
    }

    #[test]
    fn separated_by_at_least_without_trailing() {
        let parser = just::<_, _, EmptyErr, ()>('-')
            .separated_by(just(','))
            .at_least(3)
            .collect::<Vec<_>>()
            .then(end());

        // Is empty means no errors
        assert!(parser.parse("-,-,-,").has_errors());
    }

    #[test]
    fn separated_by_at_least_with_leading() {
        let parser = just::<_, _, EmptyErr, ()>('-')
            .separated_by(just(','))
            .allow_leading()
            .at_least(3)
            .collect();

        assert_eq!(parser.parse(",-,-,-").into_result(), Ok(vec!['-', '-', '-']));
        assert!(parser.parse(",-,-").has_errors());
    }

    #[test]
    fn separated_by_at_least_with_trailing() {
        let parser = just::<_, _, EmptyErr, ()>('-')
            .separated_by(just(','))
            .allow_trailing()
            .at_least(3)
            .collect();

        assert_eq!(parser.parse("-,-,-,").into_result(), Ok(vec!['-', '-', '-']));
        assert!(parser.parse("-,-,").has_errors());
    }

    #[test]
    fn separated_by_leaves_last_separator() {
        let parser = just::<_, _, EmptyErr, ()>('-')
            .separated_by(just(','))
            .collect::<Vec<_>>()
            .chain(just(','));
        assert_eq!(
            parser.parse("-,-,-,").into_result(),
            Ok(vec!['-', '-', '-', ',']),
        )
    }
}

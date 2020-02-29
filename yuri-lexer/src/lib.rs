use nom::branch::alt;
use nom::character::complete::{alpha1, alphanumeric0, multispace0};
use nom::combinator::recognize;
use nom::sequence::{delimited, tuple};
use nom::IResult;

type Span<'a> = nom_locate::LocatedSpan<&'a str>;

pub enum TokenKind {
    Ident,
    ParenOpen,
    ParenClose,
    BraceOpen,
    BraceClose,
    Colon,
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
}

pub struct Token<'a> {
    pub kind: TokenKind,
    pub span: Span<'a>,
}

macro_rules! token_symbol {
    ($name:ident, $tag:expr, $kind:expr) => {
        fn $name(s: Span) -> IResult<Span, Token> {
            nom::bytes::complete::tag($tag)(s)
                .map(|(s, span)| {
                    (s, Token {
                        kind: $kind,
                        span,
                    })
                })
        }
    };
}

token_symbol!(token_paren_open, "(", TokenKind::ParenOpen);
token_symbol!(token_paren_close, ")", TokenKind::ParenClose);
token_symbol!(token_brace_open, "{", TokenKind::BraceOpen);
token_symbol!(token_brace_close, "}", TokenKind::BraceClose);
token_symbol!(token_colon, ":", TokenKind::Colon);
token_symbol!(token_equal, "=", TokenKind::Equal);
token_symbol!(token_plus, "+", TokenKind::Plus);
token_symbol!(token_minus, "-", TokenKind::Minus);
token_symbol!(token_star, "*", TokenKind::Star);
token_symbol!(token_slash, "/", TokenKind::Slash);

fn token_ident(s: Span) -> IResult<Span, Token> {
    // alphabetic followed by alphanumerics
    recognize(tuple((alpha1, alphanumeric0)))(s)
        .map(|(s, span)| {
            (s, Token {
                kind: TokenKind::Ident,
                span,
            })
        })
}

pub fn next_token(s: Span) -> IResult<Span, Token> {
    let alt = alt((
        token_paren_open,
        token_paren_close,
        token_brace_open,
        token_brace_close,
        token_colon,
        token_equal,
        token_plus,
        token_minus,
        token_star,
        token_slash,
        token_ident,
    ));
    delimited(multispace0, alt, multispace0)(s)
}
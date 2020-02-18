pub struct Span {
    pub line: usize,
    pub column: usize,
    pub len: usize,
}
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

pub struct Token<'s> {
    pub kind: TokenKind,
    pub span: Span,
    pub token: &'s str,
}

fn first_token(input: &str) -> (usize, Token) {
    todo!()
}

pub fn tokenize(mut input: &str) -> impl Iterator<Item = Token> + '_ {
    std::iter::from_fn(move || {
        if input.is_empty() {
            return None;
        }
        let (pos, token) = first_token(input);
        input = &input[pos + token.len..];
        Some(token)
    })
}
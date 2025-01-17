use std::iter::Peekable;

pub use rustc_ast::token::{BinOpToken, Delimiter, Lit, LitKind};
use rustc_ast::{
    token::{self, TokenKind},
    tokenstream::{self, TokenStream, TokenTree},
};
use rustc_span::{symbol::kw, BytePos, Symbol};

#[derive(Clone, Debug)]
pub enum Token {
    Caret,
    EqEq,
    Eq,
    AndAnd,
    OrOr,
    Plus,
    Minus,
    Star,
    Colon,
    Comma,
    Semi,
    RArrow,
    Dot,
    Lt,
    Le,
    Gt,
    Ge,
    At,
    Fn,
    Iff,
    FatArrow,
    Mut,
    Where,
    Requires,
    Ensures,
    Literal(Lit),
    Ident(Symbol),
    OpenDelim(Delimiter),
    CloseDelim(Delimiter),
    Invalid,
    Ref,
    And,
    Percent,
    Strg,
    Type,
    Ignore,
    Assume,
    Check,
    If,
    Else,
}

pub(crate) struct Cursor {
    stack: Vec<Frame>,
    offset: BytePos,
    symbs: Symbols,
}

struct Symbols {
    fn_: Symbol,
    ref_: Symbol,
    requires: Symbol,
    ensures: Symbol,
    strg: Symbol,
}

struct Frame {
    cursor: Peekable<tokenstream::Cursor>,
    close: Option<(Location, Token, Location)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Location(pub(crate) BytePos);

impl Cursor {
    pub(crate) fn new(stream: TokenStream, offset: BytePos) -> Self {
        Cursor {
            stack: vec![Frame { cursor: stream.into_trees().peekable(), close: None }],
            offset,
            symbs: Symbols {
                fn_: Symbol::intern("fn"),
                ref_: Symbol::intern("ref"),
                strg: Symbol::intern("strg"),
                requires: Symbol::intern("requires"),
                ensures: Symbol::intern("ensures"),
            },
        }
    }

    fn map_token(&self, token: token::Token) -> (Location, Token, Location) {
        let span = token.span;
        let token = match token.kind {
            TokenKind::Lt => Token::Lt,
            TokenKind::Le => Token::Le,
            TokenKind::EqEq => Token::EqEq,
            TokenKind::Eq => Token::Eq,
            TokenKind::AndAnd => Token::AndAnd,
            TokenKind::OrOr => Token::OrOr,
            TokenKind::FatArrow => Token::FatArrow,
            TokenKind::Gt => Token::Gt,
            TokenKind::Ge => Token::Ge,
            TokenKind::At => Token::At,
            TokenKind::Comma => Token::Comma,
            TokenKind::Colon => Token::Colon,
            TokenKind::Semi => Token::Semi,
            TokenKind::RArrow => Token::RArrow,
            TokenKind::Dot => Token::Dot,
            TokenKind::OpenDelim(delim) => Token::OpenDelim(delim),
            TokenKind::CloseDelim(delim) => Token::CloseDelim(delim),
            TokenKind::Literal(lit) if lit.suffix.is_none() => Token::Literal(lit),
            TokenKind::Ident(symb, _) if symb == kw::True || symb == kw::False => {
                Token::Literal(Lit { kind: LitKind::Bool, symbol: symb, suffix: None })
            }
            TokenKind::Ident(symb, _) if symb == self.symbs.ref_ => Token::Ref,
            TokenKind::Ident(symb, _) if symb == self.symbs.fn_ => Token::Fn,
            TokenKind::Ident(symb, _) if symb == self.symbs.strg => Token::Strg,
            TokenKind::Ident(symb, _) if symb == self.symbs.requires => Token::Requires,
            TokenKind::Ident(symb, _) if symb == self.symbs.ensures => Token::Ensures,
            TokenKind::Ident(symb, _) if symb == kw::Mut => Token::Mut,
            TokenKind::Ident(symb, _) if symb == kw::Where => Token::Where,
            TokenKind::Ident(symb, _) if symb == kw::Type => Token::Type,
            TokenKind::Ident(symb, _) if symb == kw::If => Token::If,
            TokenKind::Ident(symb, _) if symb == kw::Else => Token::Else,
            TokenKind::Ident(symb, _) => Token::Ident(symb),
            TokenKind::BinOp(BinOpToken::Or) => Token::Caret,
            TokenKind::BinOp(BinOpToken::Plus) => Token::Plus,
            TokenKind::BinOp(BinOpToken::Minus) => Token::Minus,
            TokenKind::BinOp(BinOpToken::And) => Token::And,
            TokenKind::BinOp(BinOpToken::Percent) => Token::Percent,
            TokenKind::BinOp(BinOpToken::Star) => Token::Star,
            _ => Token::Invalid,
        };
        (Location(span.lo() - self.offset), token, Location(span.hi() - self.offset))
    }
}

impl Iterator for Cursor {
    type Item = (Location, Token, Location);

    fn next(&mut self) -> Option<Self::Item> {
        let top = self.stack.last_mut()?;

        match top.cursor.next() {
            Some(TokenTree::Token(token, _)) => {
                if let Some(TokenTree::Token(next, _)) = top.cursor.peek() {
                    match (&token.kind, &next.kind) {
                        (TokenKind::Le, TokenKind::Gt) if token.span.hi() == next.span.lo() => {
                            let lo = Location(token.span.lo() - self.offset);
                            let hi = Location(next.span.hi() - self.offset);
                            top.cursor.next();
                            return Some((lo, Token::Iff, hi));
                        }
                        _ => {}
                    }
                }
                Some(self.map_token(token))
            }
            Some(TokenTree::Delimited(span, delim, tokens)) => {
                let close = (
                    Location(span.close.lo() - self.offset),
                    Token::CloseDelim(delim),
                    Location(span.close.hi() - self.offset),
                );
                self.stack
                    .push(Frame { cursor: tokens.into_trees().peekable(), close: Some(close) });
                let token = token::Token { kind: TokenKind::OpenDelim(delim), span: span.open };
                Some(self.map_token(token))
            }
            None => self.stack.pop().unwrap().close,
        }
    }
}

impl Default for Location {
    fn default() -> Self {
        Location(BytePos(0))
    }
}

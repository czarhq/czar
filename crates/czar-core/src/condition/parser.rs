//! Tokenizer and recursive-descent parser for the condition DSL.

use super::{CompareOp, ConditionExpr, Value};

// ─── Tokenizer ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    // Keywords
    Tasks,
    Output,
    And,
    Or,
    Not,
    True,
    False,
    // Identifiers
    Ident(String),
    // Punctuation
    Dot,
    LParen,
    RParen,
    // Operators
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
}

pub(super) fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '.' => {
                tokens.push(Token::Dot);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '=' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(Token::Eq);
                i += 2;
            }
            '!' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(Token::Ne);
                i += 2;
            }
            '>' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(Token::Ge);
                i += 2;
            }
            '>' => {
                tokens.push(Token::Gt);
                i += 1;
            }
            '<' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(Token::Le);
                i += 2;
            }
            '<' => {
                tokens.push(Token::Lt);
                i += 1;
            }
            '"' | '\'' => {
                let quote = c;
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        match chars[i] {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            '\\' => s.push('\\'),
                            '"' => s.push('"'),
                            '\'' => s.push('\''),
                            c => {
                                s.push('\\');
                                s.push(c);
                            }
                        }
                    } else {
                        s.push(chars[i]);
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("unterminated string literal".to_string());
                }
                i += 1;
                tokens.push(Token::Str(s));
            }
            '-' if chars
                .get(i + 1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false) =>
            {
                i = tokenize_number(&chars, i, &mut tokens)?;
            }
            c if c.is_ascii_digit() => {
                i = tokenize_number(&chars, i, &mut tokens)?;
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let tok = match s.as_str() {
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "true" => Token::True,
                    "false" => Token::False,
                    "tasks" => Token::Tasks,
                    "output" => Token::Output,
                    _ => Token::Ident(s),
                };
                tokens.push(tok);
            }
            c => return Err(format!("unexpected character: {:?}", c)),
        }
    }

    Ok(tokens)
}

fn tokenize_number(chars: &[char], mut i: usize, tokens: &mut Vec<Token>) -> Result<usize, String> {
    let start = i;
    if chars[i] == '-' {
        i += 1;
    }
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    let is_float = i < chars.len()
        && chars[i] == '.'
        && chars
            .get(i + 1)
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false);
    if is_float {
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }
    let s: String = chars[start..i].iter().collect();
    if is_float {
        let f: f64 = s.parse().map_err(|_| format!("invalid float: {}", s))?;
        tokens.push(Token::Float(f));
    } else {
        let n: i64 = s.parse().map_err(|_| format!("invalid integer: {}", s))?;
        tokens.push(Token::Int(n));
    }
    Ok(i)
}

// ─── Parser ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub(super) struct Parser {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
}

impl Parser {
    pub(super) fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub(super) fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.advance() {
            Some(ref tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("expected {:?}, got {:?}", expected, tok)),
            None => Err(format!("expected {:?}, got end of expression", expected)),
        }
    }

    pub(super) fn parse_condition(&mut self) -> Result<ConditionExpr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<ConditionExpr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and()?;
            left = ConditionExpr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<ConditionExpr, String> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_not()?;
            left = ConditionExpr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<ConditionExpr, String> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let expr = self.parse_not()?;
            return Ok(ConditionExpr::Not(Box::new(expr)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<ConditionExpr, String> {
        if matches!(self.peek(), Some(Token::LParen)) {
            self.advance();
            let expr = self.parse_condition()?;
            self.expect(&Token::RParen)?;
            return Ok(expr);
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<ConditionExpr, String> {
        let left = self.parse_value()?;
        let op = self.parse_op()?;
        let right = self.parse_value()?;
        Ok(ConditionExpr::Comparison { left, op, right })
    }

    fn parse_op(&mut self) -> Result<CompareOp, String> {
        match self.advance() {
            Some(Token::Eq) => Ok(CompareOp::Eq),
            Some(Token::Ne) => Ok(CompareOp::Ne),
            Some(Token::Gt) => Ok(CompareOp::Gt),
            Some(Token::Ge) => Ok(CompareOp::Ge),
            Some(Token::Lt) => Ok(CompareOp::Lt),
            Some(Token::Le) => Ok(CompareOp::Le),
            Some(tok) => Err(format!(
                "expected comparison operator (==, !=, >, >=, <, <=), got {:?}",
                tok
            )),
            None => Err(
                "expected comparison operator (==, !=, >, >=, <, <=), got end of expression"
                    .to_string(),
            ),
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(Token::Tasks) => self.parse_task_ref(),
            Some(Token::Int(_)) => {
                if let Some(Token::Int(n)) = self.advance() {
                    Ok(Value::Int(n))
                } else {
                    unreachable!()
                }
            }
            Some(Token::Float(_)) => {
                if let Some(Token::Float(f)) = self.advance() {
                    Ok(Value::Float(f))
                } else {
                    unreachable!()
                }
            }
            Some(Token::True) => {
                self.advance();
                Ok(Value::Bool(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Value::Bool(false))
            }
            Some(Token::Str(_)) => {
                if let Some(Token::Str(s)) = self.advance() {
                    Ok(Value::Str(s))
                } else {
                    unreachable!()
                }
            }
            Some(tok) => Err(format!(
                "expected value (task ref or literal), got {:?}",
                tok
            )),
            None => Err("expected value, got end of expression".to_string()),
        }
    }

    fn parse_task_ref(&mut self) -> Result<Value, String> {
        self.expect(&Token::Tasks)?;
        self.expect(&Token::Dot)?;

        let task_name = match self.advance() {
            Some(Token::Ident(name)) => name,
            Some(tok) => return Err(format!("expected task name identifier, got {:?}", tok)),
            None => return Err("expected task name identifier, got end of expression".to_string()),
        };

        self.expect(&Token::Dot)?;
        self.expect(&Token::Output)?;

        let mut fields = Vec::new();
        while matches!(self.peek(), Some(Token::Dot)) {
            self.advance();
            match self.advance() {
                Some(Token::Ident(field)) => fields.push(field),
                Some(tok) => return Err(format!("expected field name after '.', got {:?}", tok)),
                None => {
                    return Err("expected field name after '.', got end of expression".to_string());
                }
            }
        }

        Ok(Value::TaskRef {
            task: task_name,
            fields,
        })
    }
}

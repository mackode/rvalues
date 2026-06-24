use std::collections::HashMap;
use chrono::{NaiveDate, Local};
use regex::Regex;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Num(f64),
    Str(String),
    Var(String),
    Ident(String),
    Op(String),
    LParen,
    RParen,
    Comma,
    Semicolon,
}

pub struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    can_be_unary: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().peekable(),
            can_be_unary: true,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.chars.next();
        c
    }

    pub fn next_token(&mut self) -> Result<Option<Token>, String> {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.next();
                continue;
            }

            if c == '$' {
                self.next();
                let mut var_name = String::new();
                while let Some(vc) = self.peek() {
                    if vc == '_' || vc.is_alphanumeric() {
                        var_name.push(vc);
                        self.next();
                    } else {
                        break;
                    }
                }
                if var_name.is_empty() {
                    return Err("Unnamed variable after $".to_string());
                }
                self.can_be_unary = false;
                return Ok(Some(Token::Var(var_name)));
            }

            if c == '"' || c == '\'' {
                let quote = c;
                self.next();
                let mut s = String::new();
                while let Some(sc) = self.peek() {
                    if sc == '\\' {
                        self.next();
                        if let Some(esc) = self.next() {
                            match esc {
                                'n' => s.push('\n'),
                                'r' => s.push('\r'),
                                't' => s.push('\t'),
                                _ => s.push(esc),
                            }
                        } else {
                            return Err("Invalid escape in string".to_string());
                        }
                    } else if sc == quote {
                        self.next();
                        self.can_be_unary = false;
                        return Ok(Some(Token::Str(s)));
                    } else {
                        s.push(sc);
                        self.next();
                    }
                }
                return Err("Unterminated string literal".to_string());
            }

            if c.is_ascii_digit() || (c == '.' && self.peek_second_is_digit()) {
                let mut num_str = String::new();
                let mut has_dot = false;
                while let Some(nc) = self.peek() {
                    if nc == '.' {
                        if !has_dot {
                            has_dot = true;
                            num_str.push(nc);
                            self.next();
                        } else {
                            break;
                        }
                    } else if nc.is_ascii_digit() {
                        num_str.push(nc);
                        self.next();
                    } else {
                        break;
                    }
                }
                let val: f64 = num_str.parse().map_err(|_| format!("Invalid number: {}", num_str))?;
                self.can_be_unary = false;
                return Ok(Some(Token::Num(val)));
            }

            if c.is_ascii_alphabetic() {
                let mut ident = String::new();
                while let Some(ic) = self.peek() {
                    if ic.is_ascii_alphanumeric() || ic == '_' {
                        ident.push(ic);
                        self.next();
                    } else {
                        break;
                    }
                }
                self.can_be_unary = self.peek() == Some('(');
                return Ok(Some(Token::Ident(ident)));
            }

            if c == '(' {
                self.next();
                self.can_be_unary = true;
                return Ok(Some(Token::LParen));
            }
            if c == ')' {
                self.next();
                self.can_be_unary = false;
                return Ok(Some(Token::RParen));
            }
            if c == ',' {
                self.next();
                self.can_be_unary = true;
                return Ok(Some(Token::Comma));
            }
            if c == ';' {
                self.next();
                self.can_be_unary = true;
                return Ok(Some(Token::Semicolon));
            }

            // Operator parsing
            if c == '-' && self.can_be_unary {
                self.next();
                return Ok(Some(Token::Op("UM".to_string())));
            }

            let mut op = String::new();
            op.push(c);
            self.next();
            if let Some(next_c) = self.peek() {
                let double_op = format!("{}{}", c, next_c);
                if ["==", "!=", "<=", ">=", "&&", "||", "<>"].contains(&double_op.as_str()) {
                    op = double_op;
                    self.next();
                }
            }
            self.can_be_unary = op != ")";
            return Ok(Some(Token::Op(op)));
        }
        Ok(None)
    }

    fn peek_second_is_digit(&self) -> bool {
        let mut temp = self.chars.clone();
        temp.next();
        temp.peek().map_or(false, |c| c.is_ascii_digit())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    Var(String),
    Func(String, Vec<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Unary(String, Box<Expr>),
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Option<Token>,
    peek_token: Option<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token()?;
        let peek_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token,
            peek_token,
        })
    }

    fn next_token(&mut self) -> Result<(), String> {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token()?;
        Ok(())
    }

    fn get_precedence(op: &str) -> i32 {
        match op {
            "*" | "/" | "%" => 10,
            "+" | "-" => 8,
            "==" | "<>" | "!=" | "<=" | ">=" | "<" | ">" => 6,
            "." => 4,
            "&&" | "||" => 3,
            _ => 0,
        }
    }

    pub fn parse_expression(&mut self, precedence: i32) -> Result<Expr, String> {
        let mut left = self.parse_prefix()?;

        while self.peek_token.is_some() && precedence < self.peek_precedence() {
            if let Some(Token::Op(op)) = &self.peek_token {
                let op = op.clone();
                self.next_token()?;
                left = self.parse_infix(left, &op)?;
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn peek_precedence(&self) -> i32 {
        if let Some(Token::Op(op)) = &self.peek_token {
            Self::get_precedence(op)
        } else {
            0
        }
    }

    fn cur_precedence(&self) -> i32 {
        if let Some(Token::Op(op)) = &self.current_token {
            Self::get_precedence(op)
        } else {
            0
        }
    }

    fn parse_prefix(&mut self) -> Result<Expr, String> {
        match &self.current_token {
            Some(Token::Num(n)) => Ok(Expr::Num(*n)),
            Some(Token::Str(s)) => Ok(Expr::Str(s.clone())),
            Some(Token::Var(v)) => Ok(Expr::Var(v.clone())),
            Some(Token::Ident(id)) => {
                let id = id.clone();
                if self.peek_token == Some(Token::LParen) {
                    self.next_token()?; // consume Ident, current becomes LParen
                    self.next_token()?; // consume LParen, current becomes first arg or RParen
                    let mut args = Vec::new();
                    if self.current_token != Some(Token::RParen) {
                        args.push(self.parse_expression(0)?);
                        while self.peek_token == Some(Token::Comma) {
                            self.next_token()?; // consume last token of previous arg, current becomes Comma
                            self.next_token()?; // consume Comma, current becomes next arg
                            args.push(self.parse_expression(0)?);
                        }
                        if self.peek_token != Some(Token::RParen) {
                            return Err("Expected matching ')' in function call".to_string());
                        }
                    }
                    self.next_token()?; // consume last token of last arg or LParen, current becomes RParen
                    Ok(Expr::Func(id, args))
                } else {
                    Ok(Expr::Str(id))
                }
            }
            Some(Token::LParen) => {
                self.next_token()?; // consume LParen
                let expr = self.parse_expression(0)?;
                if self.peek_token != Some(Token::RParen) {
                    return Err("Expected matching ')'".to_string());
                }
                self.next_token()?; // consume last token of expr, current becomes RParen
                Ok(expr)
            }
            Some(Token::Op(op)) if op == "UM" => {
                self.next_token()?; // consume UM
                let expr = self.parse_expression(11)?;
                Ok(Expr::Unary("-".to_string(), Box::new(expr)))
            }
            _ => Err(format!("Unexpected prefix token: {:?}", self.current_token)),
        }
    }

    fn parse_infix(&mut self, left: Expr, op: &str) -> Result<Expr, String> {
        let precedence = Self::get_precedence(op);
        self.next_token()?; // consume operator
        let right = self.parse_expression(precedence)?;
        Ok(Expr::Binary(op.to_string(), Box::new(left), Box::new(right)))
    }

    pub fn parse_all_expressions(&mut self) -> Result<Vec<Expr>, String> {
        let mut exprs = Vec::new();
        while self.current_token.is_some() {
            exprs.push(self.parse_expression(0)?);
            self.next_token()?; // advance past the last token of the expression
            if self.current_token == Some(Token::Semicolon) {
                self.next_token()?; // consume Semicolon
            } else if self.current_token.is_some() {
                return Err(format!("Trailing tokens in expression: {:?}", self.current_token));
            }
        }
        Ok(exprs)
    }
}

pub fn parse(input: &str) -> Result<Vec<Expr>, String> {
    let mut parser = Parser::new(input)?;
    parser.parse_all_expressions()
}

// Helper formatting utilities
fn to_string(val: &str) -> String {
    val.to_string()
}

fn to_real(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn is_number(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

fn is_integer(s: &str) -> bool {
    s.parse::<i32>().is_ok()
}

fn to_integer(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(0)
}

pub fn to_bool(s: &str) -> bool {
    if is_number(s) {
        to_real(s) != 0.0
    } else {
        !s.is_empty()
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 3 {
        let y: i32 = parts[0].parse().ok()?;
        let m: u32 = parts[1].parse().ok()?;
        let d: u32 = parts[2].parse().ok()?;
        NaiveDate::from_ymd_opt(y, m, d)
    } else {
        None
    }
}

impl Expr {
    pub fn eval(&self, record: &[String], vars: &HashMap<String, String>, rng_seed: Option<u64>) -> Result<String, String> {
        match self {
            Expr::Num(n) => Ok(n.to_string()),
            Expr::Str(s) => Ok(s.clone()),
            Expr::Var(v) => {
                if is_integer(v) {
                    let idx = to_integer(v) - 1;
                    if idx < 0 {
                        return Err(format!("Invalid positional parameter: ${}", v));
                    }
                    if idx >= record.len() as i32 {
                        Ok(String::new())
                    } else {
                        Ok(record[idx as usize].clone())
                    }
                } else {
                    vars.get(v)
                        .cloned()
                        .ok_or_else(|| format!("Unknown variable: ${}", v))
                }
            }
            Expr::Unary(op, expr) => {
                let val = expr.eval(record, vars, rng_seed)?;
                if op == "-" {
                    let n = -to_real(&val);
                    Ok(n.to_string())
                } else {
                    Err(format!("Unknown unary operator: {}", op))
                }
            }
            Expr::Binary(op, left, right) => {
                let lhs = left.eval(record, vars, rng_seed)?;
                let rhs = right.eval(record, vars, rng_seed)?;

                match op.as_str() {
                    "." => Ok(format!("{}{}", lhs, rhs)),
                    "+" => Ok((to_real(&lhs) + to_real(&rhs)).to_string()),
                    "-" => Ok((to_real(&lhs) - to_real(&rhs)).to_string()),
                    "*" => Ok((to_real(&lhs) * to_real(&rhs)).to_string()),
                    "/" => {
                        let r = to_real(&rhs);
                        if r == 0.0 {
                            return Err("Divide by zero".to_string());
                        }
                        Ok((to_real(&lhs) / r).to_string())
                    }
                    "%" => {
                        let l = to_real(&lhs);
                        let r = to_real(&rhs);
                        if l < 0.0 || r < 0.0 {
                            return Err("Invalid operands for % operator".to_string());
                        }
                        let result = (l as i64) % (r as i64);
                        Ok(result.to_string())
                    }
                    "==" | "!=" | "<>" | "<" | ">" | "<=" | ">=" => {
                        let res = if is_number(&lhs) && is_number(&rhs) {
                            let dl = to_real(&lhs);
                            let dr = to_real(&rhs);
                            match op.as_str() {
                                "==" => dl == dr,
                                "!=" | "<>" => dl != dr,
                                "<" => dl < dr,
                                ">" => dl > dr,
                                "<=" => dl <= dr,
                                ">=" => dl >= dr,
                                _ => false,
                            }
                        } else {
                            match op.as_str() {
                                "==" => lhs == rhs,
                                "!=" | "<>" => lhs != rhs,
                                "<" => lhs < rhs,
                                ">" => lhs > rhs,
                                "<=" => lhs <= rhs,
                                ">=" => lhs >= rhs,
                                _ => false,
                            }
                        };
                        Ok(if res { "1".to_string() } else { "0".to_string() })
                    }
                    "&&" => {
                        let res = to_bool(&lhs) && to_bool(&rhs);
                        Ok(if res { "1".to_string() } else { "0".to_string() })
                    }
                    "||" => {
                        let res = to_bool(&lhs) || to_bool(&rhs);
                        Ok(if res { "1".to_string() } else { "0".to_string() })
                    }
                    _ => Err(format!("Unknown binary operator: {}", op)),
                }
            }
            Expr::Func(name, args) => {
                let mut evaluated_args = Vec::new();
                for arg in args {
                    evaluated_args.push(arg.eval(record, vars, rng_seed)?);
                }

                match name.to_ascii_lowercase().as_str() {
                    "abs" => {
                        if evaluated_args.len() != 1 {
                            return Err("abs() expects 1 argument".to_string());
                        }
                        let n = to_real(&evaluated_args[0]).abs();
                        Ok(n.to_string())
                    }
                    "bool" => {
                        if evaluated_args.len() != 1 {
                            return Err("bool() expects 1 argument".to_string());
                        }
                        Ok(if to_bool(&evaluated_args[0]) { "1".to_string() } else { "0".to_string() })
                    }
                    "day" => {
                        if evaluated_args.len() != 1 {
                            return Err("day() expects 1 argument".to_string());
                        }
                        let d = parse_date(&evaluated_args[0])
                            .map(|date| chrono::Datelike::day(&date).to_string())
                            .unwrap_or_default();
                        Ok(d)
                    }
                    "env" => {
                        if evaluated_args.len() != 1 {
                            return Err("env() expects 1 argument".to_string());
                        }
                        Ok(std::env::var(&evaluated_args[0]).unwrap_or_default())
                    }
                    "field" => {
                        if evaluated_args.len() != 1 {
                            return Err("field() expects 1 argument".to_string());
                        }
                        if !is_integer(&evaluated_args[0]) {
                            return Err("Parameter of field() must be integer".to_string());
                        }
                        let i = to_integer(&evaluated_args[0]) - 1;
                        if i < 0 || i >= record.len() as i32 {
                            Ok(String::new())
                        } else {
                            Ok(record[i as usize].clone())
                        }
                    }
                    "find" => {
                        if evaluated_args.len() != 1 {
                            return Err("find() expects 1 argument".to_string());
                        }
                        let re = Regex::new(&evaluated_args[0])
                            .map_err(|e| format!("Invalid regex in find(): {}", e))?;
                        let mut found_idx = 0;
                        for (i, val) in record.iter().enumerate() {
                            if re.is_match(val) {
                                found_idx = i + 1;
                                break;
                            }
                        }
                        Ok(found_idx.to_string())
                    }
                    "if" => {
                        if evaluated_args.len() != 3 {
                            return Err("if() expects 3 arguments".to_string());
                        }
                        if to_bool(&evaluated_args[0]) {
                            Ok(evaluated_args[1].clone())
                        } else {
                            Ok(evaluated_args[2].clone())
                        }
                    }
                    "index" => {
                        if evaluated_args.len() != 2 {
                            return Err("index() expects 2 arguments".to_string());
                        }
                        let val = &evaluated_args[0];
                        let list = &evaluated_args[1];
                        let items: Vec<&str> = list.split(',').collect();
                        let pos = items.iter().position(|&x| x == val).map_or(0, |idx| idx + 1);
                        Ok(pos.to_string())
                    }
                    "int" => {
                        if evaluated_args.len() != 1 {
                            return Err("int() expects 1 argument".to_string());
                        }
                        let n = to_real(&evaluated_args[0]) as i64;
                        Ok(n.to_string())
                    }
                    "isdate" => {
                        if evaluated_args.len() != 1 {
                            return Err("isdate() expects 1 argument".to_string());
                        }
                        Ok(if parse_date(&evaluated_args[0]).is_some() { "1".to_string() } else { "0".to_string() })
                    }
                    "isempty" => {
                        if evaluated_args.len() != 1 {
                            return Err("isempty() expects 1 argument".to_string());
                        }
                        let empty = evaluated_args[0].trim().is_empty();
                        Ok(if empty { "1".to_string() } else { "0".to_string() })
                    }
                    "isint" => {
                        if evaluated_args.len() != 1 {
                            return Err("isint() expects 1 argument".to_string());
                        }
                        Ok(if is_integer(&evaluated_args[0]) { "1".to_string() } else { "0".to_string() })
                    }
                    "isnum" => {
                        if evaluated_args.len() != 1 {
                            return Err("isnum() expects 1 argument".to_string());
                        }
                        Ok(if is_number(&evaluated_args[0]) { "1".to_string() } else { "0".to_string() })
                    }
                    "month" => {
                        if evaluated_args.len() != 1 {
                            return Err("month() expects 1 argument".to_string());
                        }
                        let m = parse_date(&evaluated_args[0])
                            .map(|date| chrono::Datelike::month(&date).to_string())
                            .unwrap_or_default();
                        Ok(m)
                    }
                    "not" => {
                        if evaluated_args.len() != 1 {
                            return Err("not() expects 1 argument".to_string());
                        }
                        Ok(if to_bool(&evaluated_args[0]) { "0".to_string() } else { "1".to_string() })
                    }
                    "pos" => {
                        if evaluated_args.len() != 2 {
                            return Err("pos() expects 2 arguments".to_string());
                        }
                        let haystack = &evaluated_args[0];
                        let needle = &evaluated_args[1];
                        let idx = haystack.find(needle).map_or(0, |pos| pos + 1);
                        Ok(idx.to_string())
                    }
                    "random" => {
                        if !evaluated_args.is_empty() {
                            return Err("random() expects 0 arguments".to_string());
                        }
                        let seed = rng_seed.unwrap_or_else(|| {
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                        });
                        // Simple ThreadRng or seeded generator
                        let mut rng = ChaCha8Rng::seed_from_u64(seed);
                        let val: f64 = rng.r#gen();
                        Ok(val.to_string())
                    }
                    "sign" => {
                        if evaluated_args.len() != 1 {
                            return Err("sign() expects 1 argument".to_string());
                        }
                        let n = to_real(&evaluated_args[0]);
                        Ok(if n == 0.0 { "0" } else if n > 0.0 { "1" } else { "-1" }.to_string())
                    }
                    "substr" => {
                        if evaluated_args.len() != 3 {
                            return Err("substr() expects 3 arguments".to_string());
                        }
                        let s = &evaluated_args[0];
                        let pos = to_integer(&evaluated_args[1]) - 1;
                        let len = to_integer(&evaluated_args[2]);
                        if pos < 0 {
                            return Err("Invalid position in substr()".to_string());
                        }
                        if len < 0 {
                            return Err("Invalid length in substr()".to_string());
                        }
                        // Use char boundary safety substring
                        let chars: Vec<char> = s.chars().collect();
                        if pos as usize >= chars.len() {
                            Ok(String::new())
                        } else {
                            let end = (pos as usize + len as usize).min(chars.len());
                            let sub: String = chars[pos as usize..end].iter().collect();
                            Ok(sub)
                        }
                    }
                    "trim" => {
                        if evaluated_args.len() != 1 {
                            return Err("trim() expects 1 argument".to_string());
                        }
                        Ok(evaluated_args[0].trim().to_string())
                    }
                    "today" => {
                        if !evaluated_args.is_empty() {
                            return Err("today() expects 0 arguments".to_string());
                        }
                        let date = Local::now().date_naive();
                        Ok(date.format("%Y-%m-%d").to_string())
                    }
                    "now" => {
                        if !evaluated_args.is_empty() {
                            return Err("now() expects 0 arguments".to_string());
                        }
                        let time = Local::now().time();
                        Ok(time.format("%H:%M:%S").to_string())
                    }
                    "upper" => {
                        if evaluated_args.len() != 1 {
                            return Err("upper() expects 1 argument".to_string());
                        }
                        Ok(evaluated_args[0].to_uppercase())
                    }
                    "lower" => {
                        if evaluated_args.len() != 1 {
                            return Err("lower() expects 1 argument".to_string());
                        }
                        Ok(evaluated_args[0].to_lowercase())
                    }
                    "len" => {
                        if evaluated_args.len() != 1 {
                            return Err("len() expects 1 argument".to_string());
                        }
                        Ok(evaluated_args[0].chars().count().to_string())
                    }
                    "streq" => {
                        if evaluated_args.len() != 2 {
                            return Err("streq() expects 2 arguments".to_string());
                        }
                        let eq = evaluated_args[0].to_ascii_lowercase() == evaluated_args[1].to_ascii_lowercase();
                        Ok(if eq { "1".to_string() } else { "0".to_string() })
                    }
                    "match" => {
                        if evaluated_args.len() != 2 {
                            return Err("match() expects 2 arguments".to_string());
                        }
                        let re = Regex::new(&evaluated_args[1])
                            .map_err(|e| format!("Invalid regex in match(): {}", e))?;
                        Ok(if re.is_match(&evaluated_args[0]) { "1".to_string() } else { "0".to_string() })
                    }
                    "max" => {
                        if evaluated_args.len() != 2 {
                            return Err("max() expects 2 arguments".to_string());
                        }
                        let a = &evaluated_args[0];
                        let b = &evaluated_args[1];
                        if is_number(a) && is_number(b) {
                            let da = to_real(a);
                            let db = to_real(b);
                            Ok(if da > db { a.clone() } else { b.clone() })
                        } else {
                            Ok(if a > b { a.clone() } else { b.clone() })
                        }
                    }
                    "min" => {
                        if evaluated_args.len() != 2 {
                            return Err("min() expects 2 arguments".to_string());
                        }
                        let a = &evaluated_args[0];
                        let b = &evaluated_args[1];
                        if is_number(a) && is_number(b) {
                            let da = to_real(a);
                            let db = to_real(b);
                            Ok(if da < db { a.clone() } else { b.clone() })
                        } else {
                            Ok(if a < b { a.clone() } else { b.clone() })
                        }
                    }
                    "pick" => {
                        if evaluated_args.len() != 2 {
                            return Err("pick() expects 2 arguments".to_string());
                        }
                        if !is_integer(&evaluated_args[0]) {
                            return Err("First parameter of pick() must be integer".to_string());
                        }
                        let n = to_integer(&evaluated_args[0]);
                        let list = &evaluated_args[1];
                        let items: Vec<&str> = list.split(',').collect();
                        if n < 1 || n > items.len() as i32 {
                            Ok(String::new())
                        } else {
                            Ok(items[(n - 1) as usize].to_string())
                        }
                    }
                    "year" => {
                        if evaluated_args.len() != 1 {
                            return Err("year() expects 1 argument".to_string());
                        }
                        let y = parse_date(&evaluated_args[0])
                            .map(|date| chrono::Datelike::year(&date).to_string())
                            .unwrap_or_default();
                        Ok(y)
                    }
                    "round" => {
                        if evaluated_args.len() != 2 {
                            return Err("round() expects 2 arguments".to_string());
                        }
                        if !is_integer(&evaluated_args[1]) {
                            return Err("Second parameter of round() must be integer".to_string());
                        }
                        let n = if is_number(&evaluated_args[0]) { to_real(&evaluated_args[0]) } else { 0.0 };
                        let d = to_integer(&evaluated_args[1]);
                        if d < 0 {
                            return Err("Second parameter of round() must be non-negative".to_string());
                        }
                        Ok(format!("{:.*}", d as usize, n))
                    }
                    _ => Err(format!("Unknown function: {}", name)),
                }
            }
        }
    }
}

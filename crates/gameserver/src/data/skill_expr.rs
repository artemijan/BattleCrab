//! The `{…}` computed-value expressions in skill XMLs (Java evaluates them
//! with exp4j at parse time — `SkillData.parseNodeValue`). The dist uses 85
//! distinct expressions, all plain arithmetic over three variables:
//! `{base + base / 100 * subIndex}`, `{23+index*3}`, `{0.99 - 0.006 *
//! (subIndex - 1)}`, … — numbers, `+ − * /`, parentheses, unary minus,
//! `base`/`index`/`subIndex`. This is a tiny recursive-descent evaluator for
//! exactly that grammar; anything else (including the one truncated
//! expression the dist ships) evaluates to `None` and the row is dropped,
//! like Java's exception path skipping the value.

/// Variables available to a skill-value expression.
#[derive(Debug, Clone, Copy)]
pub struct ExprVars {
    /// The same level's non-enchanted value (`values[level][-1]` in Java).
    /// `None` when the field has no base — an expression referencing `base`
    /// then fails to evaluate, as Java's missing-variable throw does.
    pub base: Option<f64>,
    /// `(level − fromLevel) + 1`.
    pub index: f64,
    /// `(subLevel − fromSubLevel) + 1`.
    pub sub_index: f64,
}

/// Evaluate `{…}` (braces included) against the variables. `None` on any
/// parse error, unknown identifier, or missing `base`.
pub fn eval_braced(text: &str, vars: ExprVars) -> Option<f64> {
    let inner = text.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut p = Parser {
        bytes: inner.as_bytes(),
        pos: 0,
        vars,
    };
    let v = p.expr()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return None; // trailing garbage
    }
    v.is_finite().then_some(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    vars: ExprVars,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| b.is_ascii_whitespace())
        {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.pos).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.pos += 1;
                    v += self.term()?;
                }
                Some(b'-') => {
                    self.pos += 1;
                    v -= self.term()?;
                }
                _ => return Some(v),
            }
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.pos += 1;
                    v *= self.factor()?;
                }
                Some(b'/') => {
                    self.pos += 1;
                    v /= self.factor()?;
                }
                _ => return Some(v),
            }
        }
    }

    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            b'-' => {
                self.pos += 1;
                Some(-self.factor()?)
            }
            b'(' => {
                self.pos += 1;
                let v = self.expr()?;
                if self.peek()? != b')' {
                    return None;
                }
                self.pos += 1;
                Some(v)
            }
            b'0'..=b'9' | b'.' => {
                let start = self.pos;
                while self
                    .bytes
                    .get(self.pos)
                    .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
                {
                    self.pos += 1;
                }
                std::str::from_utf8(&self.bytes[start..self.pos])
                    .ok()?
                    .parse()
                    .ok()
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                let start = self.pos;
                while self
                    .bytes
                    .get(self.pos)
                    .is_some_and(|b| b.is_ascii_alphanumeric())
                {
                    self.pos += 1;
                }
                match &self.bytes[start..self.pos] {
                    b"base" => self.vars.base,
                    b"index" => Some(self.vars.index),
                    b"subIndex" => Some(self.vars.sub_index),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(base: f64) -> ExprVars {
        ExprVars {
            base: Some(base),
            index: 3.0,
            sub_index: 7.0,
        }
    }

    /// Real dist expressions, one per shape.
    #[test]
    fn dist_expression_shapes_evaluate() {
        assert_eq!(
            eval_braced("{base + base / 100 * subIndex}", v(200.0)),
            Some(214.0)
        );
        assert_eq!(eval_braced("{23+index*3}", v(0.0)), Some(32.0));
        assert_eq!(
            eval_braced("{0.99 - 0.006 * (subIndex - 1)}", v(0.0)),
            Some(0.99 - 0.006 * 6.0)
        );
        assert_eq!(
            eval_braced("{-0.6 - (0.4 * subIndex)}", v(0.0)),
            Some(-0.6 - 0.4 * 7.0)
        );
        assert_eq!(eval_braced("{base - (2 * subIndex)}", v(100.0)), Some(86.0));
        assert_eq!(eval_braced("{1800 + subIndex * 15}", v(0.0)), Some(1905.0));
    }

    /// Failure modes: the dist's one truncated expression, unknown names,
    /// `base` referenced without a base value.
    #[test]
    fn malformed_expressions_are_none() {
        assert_eq!(eval_braced("{-1 - ((subIndex - 1}", v(1.0)), None);
        assert_eq!(eval_braced("{foo + 1}", v(1.0)), None);
        assert_eq!(
            eval_braced(
                "{base + 1}",
                ExprVars {
                    base: None,
                    index: 1.0,
                    sub_index: 1.0
                }
            ),
            None
        );
        assert_eq!(eval_braced("plain text", v(1.0)), None);
    }
}

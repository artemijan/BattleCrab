//! Value types for columns SQLite stores in more than one storage class.

use sea_orm::sea_query::{ArrayType, ColumnType, Nullable, ValueType, ValueTypeErr};
use sea_orm::{DbErr, QueryResult, TryGetError, TryGetable, Value};

/// A number that may be stored as either `INTEGER` or `REAL`.
///
/// The Java server writes doubles into columns the DDL declares as integers —
/// `characters.curHp` is `MEDIUMINT`, `pets.curHp` is `int` — and SQLite's type
/// affinity keeps whichever form the value actually took: `1234.5` stays REAL,
/// `1234.0` becomes INTEGER. One column therefore holds both storage classes,
/// often within one table.
///
/// sqlx 0.9 refuses to decode an `f64` from an INTEGER value, so a plain `f64`
/// field fails to load any character whose HP happens to be a whole number —
/// which is most of them. This type tries REAL first and falls back to INTEGER,
/// exactly as the hand-written `getf` helper did before the ORM.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct LooseF64(pub f64);

impl From<f64> for LooseF64 {
    fn from(v: f64) -> Self {
        Self(v)
    }
}

impl From<LooseF64> for f64 {
    fn from(v: LooseF64) -> Self {
        v.0
    }
}

impl From<LooseF64> for Value {
    /// Always writes REAL: a fractional value must survive the round trip, and
    /// SQLite narrows it back to INTEGER by itself when it is whole.
    fn from(v: LooseF64) -> Self {
        Value::Double(Some(v.0))
    }
}

impl TryGetable for LooseF64 {
    fn try_get_by<I: sea_orm::ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        match <f64 as TryGetable>::try_get_by(res, index) {
            Ok(v) => Ok(Self(v)),
            // A `Null` error means the column really is NULL; only a decode
            // failure is worth retrying as an integer.
            Err(TryGetError::Null(col)) => Err(TryGetError::Null(col)),
            Err(_) => <i64 as TryGetable>::try_get_by(res, index).map(|v| Self(v as f64)),
        }
    }
}

impl ValueType for LooseF64 {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::Double(Some(v)) => Ok(Self(v)),
            Value::Float(Some(v)) => Ok(Self(v as f64)),
            Value::BigInt(Some(v)) => Ok(Self(v as f64)),
            Value::Int(Some(v)) => Ok(Self(f64::from(v))),
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        stringify!(LooseF64).to_owned()
    }

    fn array_type() -> ArrayType {
        ArrayType::Double
    }

    fn column_type() -> ColumnType {
        ColumnType::Double
    }
}

impl Nullable for LooseF64 {
    fn null() -> Value {
        Value::Double(None)
    }
}

impl TryFrom<LooseF64> for i64 {
    type Error = DbErr;

    fn try_from(v: LooseF64) -> Result<Self, Self::Error> {
        Ok(v.0 as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_both_ways() {
        assert_eq!(f64::from(LooseF64::from(1234.5)), 1234.5);
        assert_eq!(Value::from(LooseF64(1.0)), Value::Double(Some(1.0)));
    }

    /// The whole point: a whole number that SQLite filed as INTEGER must still
    /// arrive as a float.
    #[test]
    fn accepts_an_integer_value() {
        assert_eq!(
            <LooseF64 as ValueType>::try_from(Value::BigInt(Some(1234))).unwrap(),
            LooseF64(1234.0)
        );
        assert_eq!(
            <LooseF64 as ValueType>::try_from(Value::Double(Some(1234.5))).unwrap(),
            LooseF64(1234.5)
        );
    }
}

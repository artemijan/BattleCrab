use std::str::FromStr;

/// Parse format `id,seconds;id,seconds;…`. Malformed pairs are
/// skipped rather than failing the boot, matching Java's lenient split.
pub(crate) fn parse_tuples_separated_by_semicolon<C, A, B>(raw: &str) -> C
where
    C: FromIterator<(A, B)>,
    A: FromStr,
    B: FromStr,
{
    raw.split(';')
        .filter_map(|pair| {
            let (id, secs) = pair.split_once(',')?;
            Some((id.trim().parse().ok()?, secs.trim().parse().ok()?))
        })
        .collect()
}

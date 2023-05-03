use std::time::Duration;

use nom::{bytes::streaming::tag, number::complete::float, IResult};

use serde::{de::Visitor, Deserializer};

struct StringyDuration;

impl<'de> Visitor<'de> for StringyDuration {
    type Value = Duration;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("EXPECTED: path and query component, eg: /foo?bar=baz")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let parse_res: IResult<&str, f32> = nom::sequence::terminated(float, tag("s"))(v);
        let (_, secs) = parse_res
            .map_err(|e| E::custom(format!("Unable to parse stringy duration: {:?}", e)))?;
        //     separated_list1(delimited(space0, tag(","), space0), parse_thunk)(v)?;
        // match matched.as_slice() {
        //     [only] => Ok((input, only.clone())),
        //     rest => Ok((input, Language::Splat(rest.to_vec()))),
        // }
        Ok(Duration::from_secs_f32(secs))
    }
}

pub fn deserialize<'de, D>(de: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    de.deserialize_str(StringyDuration)
}

use serde_derive::Deserialize;

use core::fmt;

use serde::{
    de::{self, SeqAccess, Visitor},
    Deserializer,
};

#[derive(Debug, Deserialize)]


remove exchange utils and make this specific to each exchange that needs it, add a note that we could make one impl of this if we had const generics with serde
struct StringF64ArrayVisitor<const N: usize>;

impl<'a, const N: usize> Visitor<'a> for StringF64ArrayVisitor<N> {
    type Value = Vec<[f64; N]>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&format!(
            "a vector of {}-element arrays of strings representing floats",
            N
        ))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'a>,
    {
        let mut vec = vec![];

        while let Some(arr) = seq.next_element::<[String; N]>()? {
            let mut f_arr = [0f64; N];
            for i in 0..N {
                f_arr[i] = arr[i].parse().map_err(de::Error::custom)?;
            }
            vec.push(f_arr);
        }

        Ok(vec)
    }
}

pub fn convert_array_items_to_f64<'a, D, const N: usize>(
    deserializer: D,
) -> Result<Vec<[f64; N]>, D::Error>
where
    D: Deserializer<'a>,
{
    deserializer.deserialize_seq(StringF64ArrayVisitor::<N>)
}

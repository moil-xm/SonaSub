use cpal::{Data, FromSample, Sample, SampleFormat, SizedSample, I24, U24};

pub(crate) fn data_to_f32(data: &Data) -> Result<Vec<f32>, String> {
    match data.sample_format() {
        SampleFormat::F32 => convert_typed::<f32>(data),
        SampleFormat::F64 => convert_typed::<f64>(data),
        SampleFormat::I8 => convert_typed::<i8>(data),
        SampleFormat::I16 => convert_typed::<i16>(data),
        SampleFormat::I24 => convert_typed::<I24>(data),
        SampleFormat::I32 => convert_typed::<i32>(data),
        SampleFormat::I64 => convert_typed::<i64>(data),
        SampleFormat::U8 => convert_typed::<u8>(data),
        SampleFormat::U16 => convert_typed::<u16>(data),
        SampleFormat::U24 => convert_typed::<U24>(data),
        SampleFormat::U32 => convert_typed::<u32>(data),
        SampleFormat::U64 => convert_typed::<u64>(data),
        format => Err(format!("Audio sample format `{format}` is not supported.")),
    }
}

fn convert_typed<T>(data: &Data) -> Result<Vec<f32>, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let samples = data.as_slice::<T>().ok_or_else(|| {
        format!(
            "The audio backend reported `{}` but supplied a different sample type.",
            data.sample_format()
        )
    })?;
    Ok(samples
        .iter()
        .map(|sample| f32::from_sample(*sample))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converted<T>(samples: &[T]) -> Vec<f32>
    where
        T: Sample + Copy,
        f32: FromSample<T>,
    {
        samples
            .iter()
            .map(|sample| f32::from_sample(*sample))
            .collect()
    }

    #[test]
    fn converts_float_and_integer_sample_formats_to_f32() {
        assert_eq!(converted(&[0.0f32, 0.5, -0.5]), vec![0.0, 0.5, -0.5]);
        assert_eq!(converted(&[0.0f64, 0.25]), vec![0.0, 0.25]);
        assert_eq!(converted(&[0i8, 127, -128]), vec![0.0, 0.9921875, -1.0]);
        assert_eq!(converted(&[0i16, 32767]), vec![0.0, 0.9999695]);
        assert_eq!(converted(&[0u8, 255]), vec![-1.0, 0.9921875]);
        assert_eq!(converted(&[0u16, 65535]), vec![-1.0, 0.9999695]);
        assert_eq!(converted(&[0i32, i32::MAX]), vec![0.0, 1.0]);
        assert_eq!(converted(&[0u32, u32::MAX]), vec![-1.0, 1.0]);
        assert_eq!(converted(&[0i64, i64::MAX]), vec![0.0, 1.0]);
        assert_eq!(converted(&[0u64, u64::MAX]), vec![-1.0, 1.0]);
    }

    #[test]
    fn converts_24_bit_containers() {
        let signed = [I24::new(0).unwrap(), I24::new((1 << 23) - 1).unwrap()];
        let unsigned = [U24::new(1 << 23).unwrap(), U24::new(16_777_215).unwrap()];
        let signed = converted(&signed);
        let unsigned = converted(&unsigned);
        assert_eq!(signed[0], 0.0);
        assert!((signed[1] - 1.0).abs() < 0.001);
        assert_eq!(unsigned[0], 0.0);
        assert!((unsigned[1] - 1.0).abs() < 0.001);
    }
}

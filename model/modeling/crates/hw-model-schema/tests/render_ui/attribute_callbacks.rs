use hw_model_schema::export::{OutputMetadata, record, value};

struct Fragment;
impl record::Backend for Fragment {
    fn record_size(
        &mut self,
        _: usize,
        _: record::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl value::Backend for Fragment {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;

    fn metadata(&mut self, _: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        unreachable!()
    }
}

fn main() {}

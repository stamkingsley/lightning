fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos("schema/proto/lightning.proto")?;
    tonic_prost_build::compile_protos("schema/proto/management.proto")?;
    Ok(())
}
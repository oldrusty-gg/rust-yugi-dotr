#[test]
fn duel_model_shaders_pass_wgsl_validation() {
    for (name, source) in [
        ("duel_model", include_str!("../assets/duel_model.wgsl")),
        (
            "duel_model_present",
            include_str!("../assets/duel_model_present.wgsl"),
        ),
    ] {
        let source = source.replace("#{MATERIAL_BIND_GROUP}", "3");
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|error| panic!("{name} shader validation failed: {error:?}"));
    }
}

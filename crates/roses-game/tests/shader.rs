#[test]
fn captured_shader_passes_wgsl_validation() {
    // Resolve the one Bevy shader preprocessor placeholder. This validates the
    // standalone shader source, not Bevy pipeline linkage or GPU behavior.
    let source = include_str!("../assets/captured.wgsl").replace("#{MATERIAL_BIND_GROUP}", "3");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("Shader validation failed: {error:?}"));
}

#[test]
fn opening_composite_passes_wgsl_validation() {
    let source =
        include_str!("../assets/opening_composite.wgsl").replace("#{MATERIAL_BIND_GROUP}", "3");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
fn clouds_shader_passes_wgsl_validation() {
    let source = include_str!("../assets/clouds.wgsl").replace("#{MATERIAL_BIND_GROUP}", "3");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("Cloud shader validation failed: {error:?}"));
}

#[test]
fn duel_board_shader_passes_wgsl_validation() {
    let source = include_str!("../assets/duel_board.wgsl").replace("#{MATERIAL_BIND_GROUP}", "3");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("Duel board shader validation failed: {error:?}"));
}

#[test]
fn duel_overlay_and_present_shaders_pass_wgsl_validation() {
    for (name, source) in [
        ("duel_overlay", include_str!("../assets/duel_overlay.wgsl")),
        (
            "duel_board_present",
            include_str!("../assets/duel_board_present.wgsl"),
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

/// Every material that uses the shared board-present shader must declare
/// each binding the shader reads (Bevy otherwise fails pipeline validation
/// at run time, as the campaign map did when binding 14 was added).
#[test]
fn present_shader_bindings_are_declared_by_each_material() {
    let shader = include_str!("../assets/duel_board_present.wgsl");
    let wanted: std::collections::BTreeSet<u32> = shader
        .split("@binding(")
        .skip(1)
        .map(|s| s[..s.find(')').unwrap()].parse().unwrap())
        .collect();
    for (name, source) in [
        ("duel_board_render.rs", include_str!("../src/duel_board_render.rs")),
        ("campaign_map_render.rs", include_str!("../src/campaign_map_render.rs")),
    ] {
        let start = source.find("pub struct PresentMaterial {").unwrap();
        let body = &source[start..start + source[start..].find("\n}").unwrap()];
        let declared: std::collections::BTreeSet<u32> = ["#[texture(", "#[sampler(", "#[uniform("]
            .iter()
            .flat_map(|tag| body.split(tag).skip(1).map(|s| s[..s.find(')').unwrap()].parse::<u32>().unwrap()))
            .collect();
        assert_eq!(declared, wanted, "{name}: PresentMaterial bindings vs duel_board_present.wgsl");
    }
}

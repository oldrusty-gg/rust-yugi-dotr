use super::*;

fn synthetic_constants() -> SceneConstants {
    let mut sine_table = Vec::with_capacity(1025);
    for i in 0..1025u32 {
        let angle = f64::from(i) * std::f64::consts::FRAC_PI_2 / 1024.0;
        sine_table.push(angle.sin() as f32);
    }
    sine_table[1024] = 1.0;
    SceneConstants {
        camera_focal: 800.0,
        camera_scale_x: 1.0,
        camera_scale_y: 0.94,
        camera_near: 1.0,
        camera_far: 4096.0,
        viewport: 512.0,
        viewport_offset: 256.0,
        display_mode: 9,
        display_aspect: f32::from_bits(0x3fb6_db6a),
        gs_center: 2048.0,
        z_format: 0x31,
        z_max: 16_777_215.0,
        fog_near: 2000.0,
        fog_far: 4000.0,
        fog_255: 255.0,
        light_colour: 0.5,
        light_angles: [-0x3800, 0x800, 0],
        light_base_direction: [0.0, 0.0, 1.0, 0.0],
        simon_eye_z: 512.0,
        simon_pan_scale: -64.0,
        simon_pan_divisor: 100.0,
        simon_target_y: f32::from_bits(0x41a1_999a),
        simon_level_scale: 768.0,
        simon_level_divisor: 4096.0,
        two: 2.0,
        frustum_y_slope: f32::from_bits(0x44b3_0666),
        frustum_x_slope: 2046.0,
        frustum_x_slope_squared: f32::from_bits(0x4a7f_8010),
        frustum_four: 4.0,
        colour_multiplier: f32::from_bits(0x3f4c_cccd),
        colour_multiplier_blue: 1.0,
        radius_scale: f32::from_bits(0x3f88_2b93),
        sine_table,
        normal_scale: [f32::from_bits(0x3800_1001); 4],
        scale_one: [1.0; 4],
        zero_vector: [0.0; 4],
    }
}

#[test]
fn sine_table_lookup_matches_the_special_cases_and_interpolates() {
    let c = synthetic_constants();
    assert_eq!(c.sin(0).unwrap(), 0.0);
    assert_eq!(c.sin(0x4000).unwrap(), 1.0);
    assert_eq!(c.sin(-0x4000).unwrap(), -1.0);
    assert_eq!(c.cos(0).unwrap(), 1.0);
    assert_eq!(c.cos(0x4000).unwrap(), 0.0);
    assert_eq!(c.sin(0x8000).unwrap(), 0.0);
    // 0x800 = 11.25 degrees.
    assert!((f64::from(c.sin(0x800).unwrap()) - (11.25f64).to_radians().sin()).abs() < 1e-6);
    assert!(
        (f64::from(c.sin(0x805).unwrap()) - (11.25f64 + 5.0 * 360.0 / 65536.0).to_radians().sin())
            .abs()
            < 2e-6
    );
    let r = c.rotation([0, 0, 0]).unwrap();
    assert_eq!(r[0], [1.0, 0.0, -0.0, 0.0]);
    assert_eq!(r[1][1], 1.0);
    assert_eq!(r[2][2], 1.0);
    assert_eq!(r[3], [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn light_direction_matches_the_captured_default_light() {
    let c = synthetic_constants();
    let rot = c.rotation(c.light_angles).unwrap();
    // Third column of the rotation: the direction the default light uses.
    let d = [rot[2][0], rot[2][1], rot[2][2]];
    let expected = [
        f32::from_bits(0x3d1b_e50d),
        f32::from_bits(0x3f7b_14be),
        f32::from_bits(0x3e43_ef15),
    ];
    for i in 0..3 {
        assert!(
            (f64::from(d[i]) - f64::from(expected[i])).abs() < 2e-6,
            "{i}: {} vs {}",
            d[i],
            expected[i]
        );
    }
}

#[test]
fn inverse_round_trips_a_rigid_transform() {
    let c = synthetic_constants();
    let cam = CameraMatrices::simon(&c, SimonPose::default()).unwrap();
    // Eye maps to the origin and the target lies on +Z of the view.
    let e = vu_mat_vec(&cam.view, &[0.0, 0.0, 512.0, 1.0]).unwrap();
    assert!(e[0].abs() < 1e-3 && e[1].abs() < 1e-3 && e[2].abs() < 1e-3);
    let t = vu_mat_vec(
        &cam.view,
        &[cam.target[0], cam.target[1], cam.target[2], 1.0],
    )
    .unwrap();
    assert!(t[0].abs() < 1e-3 && t[1].abs() < 1e-3 && t[2] > 500.0);
    // View · inverse(view) is the identity to float precision.
    let inv = invert(&cam.view).unwrap();
    let id = vu_mat_mul(&cam.view, &inv).unwrap();
    for (i, col) in id.iter().enumerate() {
        for (j, v) in col.iter().enumerate() {
            let want = if i == j { 1.0 } else { 0.0 };
            assert!((v - want).abs() < 1e-4, "{i},{j}: {v}");
        }
    }
    assert!(invert(&[[0.0; 4]; 4]).is_err());
    // Projection: focal 800, y scale 0.94 · aspect, GS centre 2048, PSMZ24.
    let p = cam.projection;
    assert_eq!(p[0][0], 800.0);
    assert_eq!(p[2], [2048.0, 2048.0, -2048.25, 1.0]);
    assert_eq!(p[3], [0.0, 0.0, 16_779_264.0, 0.0]);
    assert_eq!(p[1][1].to_bits(), 0x4486_4923);
    assert_eq!(cam.frustum_norm, [4_826_116.0, 705_536.0, 705_536.0]);
}

#[test]
fn fog_and_light_rows_follow_the_packet_arithmetic() {
    let c = synthetic_constants();
    let cam = CameraMatrices::simon(&c, SimonPose::default()).unwrap();
    let l = Lighting::derive(&c, &cam).unwrap();
    assert_eq!(
        l.fog_qword83,
        [
            f32::from_bits(0x3e02_8f5c),
            255.0,
            f32::from_bits(0x3e02_8f5c),
            510.0
        ]
    );
    assert_eq!(l.draw_fog_vif83, [0.0, 0.0, 0.0, 255.0]);
    for row in &l.light_rows {
        assert_eq!(row[0], row[1]);
        assert_eq!(row[1], row[2]);
        assert_eq!(row[3], 0.0);
    }
    let len: f64 = l
        .light_rows
        .iter()
        .map(|r| f64::from(r[0]) * f64::from(r[0]))
        .sum::<f64>()
        .sqrt();
    assert!((len - 0.5).abs() < 1e-5, "{len}");
}

#[test]
fn angle_interpolation_wraps_and_truncates() {
    assert_eq!(lerp_angle(0, 567, 0.97, 0.03).unwrap(), 17);
    // 0x7F00 and 0x8100 (the low sixteen bits of -0x7F00) are 512 apart: no wrap.
    assert_eq!(lerp_angle(0x7f00, -0x7f00, 0.5, 0.5).unwrap(), 0x8000);
    // Nearest crossing over 0x8000 wraps the second angle.
    assert_eq!(lerp_angle(0xff00, 0x0100, 0.5, 0.5).unwrap(), 0x10000);
    assert_eq!(key_time(-1).unwrap(), 4_294_967_296.0);
    let keys = [
        [0, 0, 0, 0],
        [100, 0, 567, 0],
        [101, 0, 578, 0],
        [102, 0, 589, 0],
    ];
    assert_eq!(track_segment(&keys, 3.0).unwrap(), (0, Some(1)));
    assert_eq!(track_segment(&keys, 100.5).unwrap(), (1, Some(2)));
    assert_eq!(track_segment(&keys, 101.5).unwrap(), (2, Some(3)));
    // Past the last key the game reads one key beyond the array; with the
    // frame at or after the last key's time the bisection ends on the last
    // key whatever that word holds (the sky's frame 500).
    assert_eq!(track_segment(&keys, 500.0).unwrap(), (3, None));
    assert_eq!(track_segment(&keys[..2], 500.0).unwrap(), (0, None));
    assert_eq!(eval_angle_track(&keys, 3.0).unwrap(), [0, 17, 0]);
}

#[test]
fn emitted_materials_scale_only_lit_draws() {
    let m = [0.8f32, 0.8, 1.0];
    let lit = [
        127.0, 127.0, 127.0, 0.0, 200.0, 200.0, 200.0, 128.0, 255.0, 255.0, 255.0, 0.0,
    ];
    let e = emitted_material(&lit, m).unwrap();
    assert_eq!(e[0], ee::mul(127.0, 0.8).unwrap());
    assert_eq!(e[4], ee::mul(200.0, 0.8).unwrap());
    assert_eq!(e[7], 128.0);
    assert_eq!(e[8], 255.0);
    let unlit = [
        127.0, 127.0, 127.0, 0.0, 0.0, 0.0, 0.0, 128.0, 0.0, 0.0, 0.0, 0.0,
    ];
    assert_eq!(emitted_material(&unlit, m).unwrap(), unlit);
    let colour = vertex_colour(
        [0, 32760, 0],
        &[
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        &[[0.0; 4], [0.5, 0.5, 0.5, 0.0], [0.0; 4]],
        &e,
    );
    assert_eq!(colour[3], 128);
    assert!(colour[0] > 101);
}

#[test]
fn motion_parser_rejects_bad_headers() {
    let mut bytes = vec![0u8; 0x60];
    bytes[..4].copy_from_slice(b"MOT\x1a");
    bytes[4..8].copy_from_slice(&0x60u32.to_le_bytes());
    bytes[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
    bytes[0x14..0x18].copy_from_slice(&0x20u32.to_le_bytes());
    bytes[0x20..0x24].copy_from_slice(&0x30u32.to_le_bytes());
    bytes[0x24..0x28].copy_from_slice(&5u32.to_le_bytes());
    bytes[0x34..0x38].copy_from_slice(&0x50u32.to_le_bytes());
    bytes[0x3c..0x40].copy_from_slice(&1u32.to_le_bytes());
    let m = Motion::parse(&bytes, 0).unwrap();
    assert_eq!(m.frame_count, 5);
    assert_eq!(m.tracks[0].rotation.len(), 1);
    assert!(m.tracks[0].translation.is_empty());
    bytes[0x3c..0x40].copy_from_slice(&9u32.to_le_bytes());
    assert!(Motion::parse(&bytes, 0).is_err());
    bytes[0] = b'X';
    assert!(Motion::parse(&bytes, 0).is_err());
}

#[test]
#[ignore = "Requires the user's extracted executable and MODEL.MRG plus the private capture (ROSES_TEST_ELF_PATH, ROSES_TEST_MODEL_ARCHIVE_PATH, ROSES_SCENE_JSON)"]
fn private_derivation_matches_the_captured_simon_scene() {
    let read = |name: &str| {
        std::fs::read(std::env::var_os(name).unwrap_or_else(|| panic!("{name}"))).unwrap()
    };
    let executable = read("ROSES_TEST_ELF_PATH");
    let archive = read("ROSES_TEST_MODEL_ARCHIVE_PATH");
    let json_path =
        std::path::PathBuf::from(std::env::var_os("ROSES_SCENE_JSON").expect("ROSES_SCENE_JSON"));
    let scene = SimonScene::derive(&executable, &archive, SimonPose::default()).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(json_path).unwrap()).unwrap();
    let columns = |v: &serde_json::Value| -> Vec<Vec<f32>> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|c| {
                c.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap() as f32)
                    .collect()
            })
            .collect()
    };
    let bits = |m: &[Qword]| -> Vec<Vec<u32>> {
        m.iter()
            .map(|q| q.iter().map(|v| v.to_bits()).collect())
            .collect()
    };
    let bits_json = |v: &serde_json::Value| -> Vec<Vec<u32>> {
        columns(v)
            .iter()
            .map(|q| q.iter().map(|v| v.to_bits()).collect())
            .collect()
    };
    assert_eq!(
        bits(&scene.camera.projection),
        bits_json(&json["gs_projection_columns"])
    );
    assert_eq!(
        bits(&scene.lighting.light_rows),
        bits_json(&json["light_from_view_normal_columns"])
    );
    assert_eq!(
        scene.lighting.fog_qword83.map(f32::to_bits).to_vec(),
        columns(&serde_json::Value::Array(vec![
            json["fog"]["vu_qword83"].clone()
        ]))[0]
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    );
    let mut checked = 0;
    for mesh in json["meshes"].as_array().unwrap() {
        let slot = mesh["object_slot"].as_u64().unwrap() as u16;
        let draw = mesh["draw_index"].as_u64().unwrap() as u32;
        let model = scene.models.iter().find(|m| m.slot == slot).unwrap();
        assert_eq!(
            bits(&model.emitted_matrix),
            bits_json(&mesh["emitted_matrix_columns"]),
            "slot {slot} matrix"
        );
        assert_eq!(
            bits(&model.normal_rows),
            bits_json(&mesh["normal_from_source_columns"]),
            "slot {slot} normals"
        );
        assert_eq!(
            model.vu_program_entry,
            mesh["vu_program_entry"].as_u64().unwrap() as u32,
            "slot {slot} entry"
        );
        assert_eq!(
            model.emitted_matrix_sha256,
            mesh["emitted_matrix_sha256"].as_str().unwrap(),
            "slot {slot} hash"
        );
        assert_eq!(
            model.record_sha256,
            mesh["source_record_sha256"].as_str().unwrap()
        );
        assert_eq!(
            model.archive_offset,
            mesh["source_record_offset"].as_u64().unwrap()
        );
        let d = scene
            .meshes
            .iter()
            .find(|d| d.object_slot == slot && d.draw_index == draw)
            .unwrap();
        let emitted: Vec<u32> = mesh["material_vectors_emitted"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| (v.as_f64().unwrap() as f32).to_bits())
            .collect();
        assert_eq!(
            d.material_vectors_emitted
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            emitted,
            "slot {slot} draw {draw} material"
        );
        let colours: Vec<Vec<u8>> = mesh["vertex_rgba_gs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as u8)
                    .collect()
            })
            .collect();
        assert_eq!(
            d.vertex_rgba_gs
                .iter()
                .map(|c| c.to_vec())
                .collect::<Vec<_>>(),
            colours,
            "slot {slot} draw {draw} colours"
        );
        let indices: Vec<u32> = mesh["indices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(d.indices, indices, "slot {slot} draw {draw} indices");
        checked += 1;
    }
    assert_eq!(checked, 32);
}

#[test]
fn animation_frames_advance_and_restart_like_the_object_updater() {
    // Values replayed from 0x1EE6E0 on the saved PAL image (sky object, 501 frames).
    assert_eq!(advance_animation_frame(3.0, 1.0, 1, 501).unwrap(), 4.0);
    assert_eq!(advance_animation_frame(498.0, 1.0, 1, 501).unwrap(), 499.0);
    assert_eq!(advance_animation_frame(499.0, 1.0, 1, 501).unwrap(), 500.0);
    assert_eq!(advance_animation_frame(500.0, 1.0, 1, 501).unwrap(), 1.0);
    assert_eq!(advance_animation_frame(499.0, 1.0, 2, 501).unwrap(), 500.0);
    assert_eq!(advance_animation_frame(1.0, 1.0, 1, 1).unwrap(), 1.0);
    assert!(advance_animation_frame(1.0, 1.0, 1, 0).is_err());
    let mut bytes = vec![0u8; 0x60];
    bytes[..4].copy_from_slice(b"MOT\x1a");
    bytes[4..8].copy_from_slice(&0x60u32.to_le_bytes());
    bytes[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
    bytes[0x14..0x18].copy_from_slice(&0x20u32.to_le_bytes());
    bytes[0x20..0x24].copy_from_slice(&0x30u32.to_le_bytes());
    bytes[0x24..0x28].copy_from_slice(&501u32.to_le_bytes());
    let m = Motion::parse(&bytes, 0).unwrap();
    assert_eq!(m.frame_after(1.0, 2, 1.0, 1).unwrap(), 3.0);
    assert_eq!(m.frame_after(1.0, 499, 1.0, 1).unwrap(), 500.0);
    assert_eq!(m.frame_after(1.0, 500, 1.0, 1).unwrap(), 1.0);
    assert_eq!(m.frame_after(1.0, 502, 1.0, 1).unwrap(), 3.0);
}

/// The sky's per-frame matrices: frame 3 is the derived (captured) pose's
/// emitted matrix bit for bit, other frames differ only by a turn about the
/// model's Y axis (the Y column is unchanged, the X/Z columns keep their
/// lengths), and 1.0 → 500 covers frame_count − 1 frames.
#[test]
#[ignore = "Requires the user's extracted executable and MODEL.MRG (ROSES_TEST_ELF_PATH, ROSES_TEST_MODEL_ARCHIVE_PATH)"]
fn private_sky_animation_matrices_cover_the_motion() {
    let read = |name: &str| {
        std::fs::read(std::env::var_os(name).unwrap_or_else(|| panic!("{name}"))).unwrap()
    };
    let executable = read("ROSES_TEST_ELF_PATH");
    let archive = read("ROSES_TEST_MODEL_ARCHIVE_PATH");
    let scene = SimonScene::derive(&executable, &archive, SimonPose::default()).unwrap();
    // Frame 500 reads past the track's last key (the MOT block's end): the
    // last key's angle.
    let angle = |f: f32| {
        let s = SimonScene::derive(&executable, &archive, SimonPose { sky_frame: f, ..SimonPose::default() }).unwrap();
        s.models.iter().find(|m| m.slot == 37).unwrap().sky_rotation_angles
    };
    assert_eq!(angle(1.0), Some([0, 6, 0]));
    assert_eq!(angle(499.0), Some([0, 5455, 0]));
    assert_eq!(angle(500.0), Some([0, 5461, 0]));
    let animated = scene.animation_matrices().unwrap();
    assert_eq!(animated.iter().map(|a| a.slot).collect::<Vec<_>>(), [37]);
    let sky = &animated[0];
    assert_eq!((sky.frame_count, sky.frames.len()), (501, 500));
    let world = scene.models.iter().find(|m| m.slot == 37).unwrap();
    let bits = |m: &Matrix| m.map(|q| q.map(f32::to_bits));
    assert_eq!(bits(&sky.frames[2].0), bits(&world.emitted_matrix));
    assert!(sky.frames.iter().all(|(_, view)| *view == world.view_space));
    let len = |q: &Qword| (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
    let base = &sky.frames[0].0;
    for (m, _) in &sky.frames {
        for k in 0..3 {
            assert!((m[1][k] - base[1][k]).abs() <= 1e-6 * len(&base[1]).max(1e-6), "Y column");
        }
        assert!((len(&m[0]) - len(&base[0])).abs() < 1e-4 * len(&base[0]));
        assert!((len(&m[2]) - len(&base[2])).abs() < 1e-4 * len(&base[2]));
        assert_eq!(bits(&[m[3], [0.0; 4], [0.0; 4], [0.0; 4]])[0], bits(&[base[3], [0.0; 4], [0.0; 4], [0.0; 4]])[0]);
    }
    assert_ne!(bits(&sky.frames[0].0), bits(&sky.frames[499].0));
}

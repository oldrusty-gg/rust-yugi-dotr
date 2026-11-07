//! A frame-time line in game.log (the game's stderr): every 60 s of play
//! (`ROSES_FRAME_STATS_S=<s>` sets the interval), the adapter that renders
//! and the window's size and present mode, the frames shown, their average
//! and worst time, how long the game update (input, the 25 Hz ticks that
//! ran, the HUD) and the render world's prepare and draw took on average,
//! and how many cameras rendered at the end. For reports (issue #20); it
//! changes nothing the game does or draws.

use bevy::{
    app::{App, First, Last},
    ecs::{
        resource::Resource,
        schedule::IntoScheduleConfigs,
        system::{Query, Res, ResMut},
        query::With,
    },
    camera::Camera,
    window::{PrimaryWindow, Window},
    render::{Render, RenderApp, RenderSystems, renderer::RenderAdapterInfo},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Nanoseconds the render world spent this interval: prepare (extract
/// commands to the draw), the draw (graph, submit, present) and its frames.
#[derive(Resource, Clone, Default)]
struct RenderTimes(Arc<[AtomicU64; 3]>);

#[derive(Resource)]
struct RenderMark(Instant, Instant);

#[derive(Resource)]
struct FrameStats {
    every: Duration,
    since: Instant,
    last: Option<Instant>,
    update_start: Instant,
    frames: u64,
    total_frames: u64,
    sum: Duration,
    worst: Duration,
    update: Duration,
    render: RenderTimes,
}

pub fn plugin(app: &mut App) {
    let every = std::env::var("ROSES_FRAME_STATS_S")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| *s > 0.0)
        .map_or(Duration::from_secs(60), Duration::from_secs_f64);
    let render = RenderTimes::default();
    let now = Instant::now();
    app.insert_resource(FrameStats {
        every,
        since: now,
        last: None,
        update_start: now,
        frames: 0,
        total_frames: 0,
        sum: Duration::ZERO,
        worst: Duration::ZERO,
        update: Duration::ZERO,
        render: render.clone(),
    })
    .add_systems(First, begin_update)
    .add_systems(Last, end_update.after(crate::render_cost::CamerasRested));
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .insert_resource(render)
            .insert_resource(RenderMark(now, now))
            .add_systems(Render, render_begin.before(RenderSystems::ExtractCommands))
            .add_systems(
                Render,
                render_draw.after(RenderSystems::PrepareBindGroups).before(RenderSystems::Render),
            )
            .add_systems(Render, render_end.after(RenderSystems::Render).before(RenderSystems::Cleanup));
    }
}

fn render_begin(mut mark: ResMut<RenderMark>) {
    let now = Instant::now();
    *mark = RenderMark(now, now);
}

fn render_draw(mut mark: ResMut<RenderMark>) {
    mark.1 = Instant::now();
}

fn render_end(mark: Res<RenderMark>, times: Res<RenderTimes>) {
    let prepare = mark.1.saturating_duration_since(mark.0).as_nanos() as u64;
    let draw = mark.1.elapsed().as_nanos() as u64;
    times.0[0].fetch_add(prepare, Ordering::Relaxed);
    times.0[1].fetch_add(draw, Ordering::Relaxed);
    times.0[2].fetch_add(1, Ordering::Relaxed);
}

fn begin_update(mut stats: ResMut<FrameStats>) {
    stats.update_start = Instant::now();
}

fn end_update(
    mut stats: ResMut<FrameStats>,
    adapter: Option<Res<RenderAdapterInfo>>,
    cameras: Query<&Camera>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    let now = Instant::now();
    let stats = &mut *stats;
    stats.update += now.saturating_duration_since(stats.update_start);
    if let Some(last) = stats.last.replace(now) {
        let frame = now.saturating_duration_since(last);
        stats.frames += 1;
        stats.total_frames += 1;
        stats.sum += frame;
        stats.worst = stats.worst.max(frame);
    } else {
        stats.since = now;
        stats.update = Duration::ZERO;
        return;
    }
    let span = now.saturating_duration_since(stats.since);
    if span < stats.every || stats.frames == 0 {
        return;
    }
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let avg = ms(stats.sum) / stats.frames as f64;
    let render = |i: usize| stats.render.0[i].swap(0, Ordering::Relaxed);
    let (prepare, draw, render_frames) = (render(0), render(1), render(2).max(1));
    let mut adapter = adapter.as_ref().map_or_else(
        || "no adapter".to_string(),
        |a| format!("{}, {:?}", a.0.name, a.0.backend),
    );
    match window.single() {
        Ok(w) => adapter += &format!(", window {}x{} {:?}", w.physical_width(), w.physical_height(), w.present_mode),
        Err(_) => adapter += ", no window",
    }
    eprintln!(
        "Frame time [{adapter}]: {:.0} s, {} frames (to frame {}), avg {avg:.1} ms ({:.1} fps), worst {:.1} ms; update avg {:.1} ms, render prepare avg {:.1} ms, draw avg {:.1} ms; {} of {} cameras rendering",
        span.as_secs_f64(),
        stats.frames,
        stats.total_frames,
        1000.0 / avg.max(0.001),
        ms(stats.worst),
        ms(stats.update) / stats.frames as f64,
        prepare as f64 / 1e6 / render_frames as f64,
        draw as f64 / 1e6 / render_frames as f64,
        cameras.iter().filter(|c| c.is_active).count(),
        cameras.iter().count(),
    );
    stats.since = now;
    stats.frames = 0;
    stats.sum = Duration::ZERO;
    stats.worst = Duration::ZERO;
    stats.update = Duration::ZERO;
}

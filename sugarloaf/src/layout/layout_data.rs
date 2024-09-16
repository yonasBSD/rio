// Copyright (c) 2023-present, Raphael Amorim.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//
// layout_data.rs was originally retired from dfrg/swash_demo licensed under MIT
// https://github.com/dfrg/swash_demo/blob/master/LICENSE

use super::render_data::*;
use super::Glyph;
use crate::font_introspector::text::cluster::ClusterInfo;
use crate::layout::FragmentStyle;

/// Cluster represents multiple glyphs.
pub const CLUSTER_DETAILED: u8 = 1;
/// Trailing clusters for a ligature.
pub const CLUSTER_CONTINUATION: u8 = 2;
/// Last continuation cluster in a ligature.
pub const CLUSTER_LAST_CONTINUATION: u8 = 4;
/// Empty clusters.
pub const CLUSTER_EMPTY: u8 = 8;
/// Cluster is a ligature.
pub const CLUSTER_LIGATURE: u8 = 16;
/// Cluster is an explicit line break.
pub const CLUSTER_NEWLINE: u8 = 32;

#[derive(Copy, Debug, Clone)]
pub struct ClusterData {
    pub info: ClusterInfo,
    pub flags: u8,
    /// Length of the cluster in the source text.
    pub len: u8,
    /// Offset of the cluster in the source text.
    pub offset: u32,
    /// Depending on `flags`, either an index into `glyphs` or an index
    /// into `detailed_clusters`
    pub glyphs: u32,
}

impl ClusterData {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.flags & CLUSTER_EMPTY != 0
    }

    #[inline]
    pub fn is_detailed(&self) -> bool {
        self.flags & CLUSTER_DETAILED != 0
    }

    #[inline]
    pub fn is_ligature(&self) -> bool {
        self.flags & CLUSTER_LIGATURE != 0
    }

    #[inline]
    pub fn is_continuation(&self) -> bool {
        self.flags & CLUSTER_CONTINUATION != 0
    }

    #[inline]
    pub fn is_last_continuation(&self) -> bool {
        self.flags & CLUSTER_LAST_CONTINUATION != 0
    }

    #[inline]
    pub fn is_newline(&self) -> bool {
        self.flags & CLUSTER_NEWLINE != 0
    }

    pub fn glyphs<'a>(
        &self,
        detail: &[DetailedClusterData],
        glyphs: &'a [GlyphData],
    ) -> &'a [GlyphData] {
        if self.is_detailed() {
            let detail = &detail[self.glyphs as usize];
            &glyphs[detail.glyphs.0 as usize..detail.glyphs.1 as usize]
        } else if self.is_empty() {
            &[]
        } else {
            &glyphs[self.glyphs as usize..self.glyphs as usize + 1]
        }
    }

    #[inline]
    pub fn advance(
        &self,
        detail: &[DetailedClusterData],
        glyphs: &[GlyphData],
        detail_glyphs: &[Glyph],
    ) -> f32 {
        if self.is_detailed() {
            detail
                .get(self.glyphs as usize)
                .map(|x| x.advance)
                .unwrap_or(0.)
        } else if self.is_continuation() {
            f32::from_bits(self.glyphs)
        } else if self.is_empty() {
            0.
        } else if let Some(glyph) = glyphs.get(self.glyphs as usize) {
            if glyph.is_simple() {
                glyph.simple_data().1
            } else {
                detail_glyphs
                    .get(glyph.detail_index())
                    .map(|x| x.advance)
                    .unwrap_or(0.)
            }
        } else {
            0.
        }
    }

    pub fn glyphs_mut<'a>(
        &self,
        detail: &[DetailedClusterData],
        glyphs: &'a mut [GlyphData],
    ) -> &'a mut [GlyphData] {
        if self.is_detailed() {
            let detail = &detail[self.glyphs as usize];
            &mut glyphs[detail.glyphs.0 as usize..detail.glyphs.1 as usize]
        } else if self.is_empty() {
            &mut []
        } else {
            &mut glyphs[self.glyphs as usize..self.glyphs as usize + 1]
        }
    }
}

#[derive(Copy, Debug, Clone)]
pub struct DetailedClusterData {
    /// Range in `glyphs`
    pub glyphs: (u32, u32),
    /// Advance of the cluster.
    pub advance: f32,
}

pub const GLYPH_DETAILED: u32 = 0x80000000;

#[derive(Copy, Debug, Clone)]
pub struct GlyphData {
    pub data: u32,
    pub size: usize,
}

impl GlyphData {
    pub fn simple(id: u16, advance: f32, size: usize) -> Self {
        let advance = (advance * 64.).max(0.) as u32;
        Self {
            data: (id as u32 | (advance & 0x7FFF) << 16),
            size,
        }
    }

    pub fn is_simple(self) -> bool {
        self.data & GLYPH_DETAILED == 0
    }

    pub fn simple_data(self) -> (u16, f32) {
        ((self.data & 0xFFFF) as u16, (self.data >> 16) as f32 / 64.)
    }

    pub fn detail_index(self) -> usize {
        (self.data & !GLYPH_DETAILED) as usize
    }

    pub fn add_spacing(&mut self, spacing: f32) {
        let (id, advance) = self.simple_data();
        *self = Self::simple(id, (advance + spacing).max(0.), self.size);
    }

    pub fn clear_advance(&mut self) {
        let (id, _advance) = self.simple_data();
        *self = Self::simple(id, 0., self.size);
    }
}

#[derive(Copy, Debug, Clone)]
pub struct RunData {
    pub span: FragmentStyle,
    pub line: u32,
    pub size: f32,
    // pub whitespace: bool,
    // pub trailing_whitespace: bool,
    pub clusters: (u32, u32),
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub strikeout_offset: f32,
    pub strikeout_size: f32,
    pub advance: f32,
}

#[derive(Clone, Debug, Default)]
pub struct LayoutData {
    /// Simple glyphs.
    pub glyphs: Vec<GlyphData>,
    /// Detailed glyphs.
    pub detailed_glyphs: Vec<Glyph>,
    /// Simple clusters.
    pub clusters: Vec<ClusterData>,
    /// Detailed clusters.
    pub detailed_clusters: Vec<DetailedClusterData>,
    /// Glyph runs.
    pub runs: Vec<RunData>,
    /// Last shaped span.
    pub last_span: usize,
}

impl LayoutData {
    pub fn clear(&mut self) {
        self.glyphs.clear();
        self.detailed_glyphs.clear();
        self.clusters.clear();
        self.detailed_clusters.clear();
        self.runs.clear();
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct LineData {
    pub x: f32,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    // pub alignment: Alignment,
    // pub trailing_whitespace: bool,
    pub explicit_break: bool,
    pub width: f32,
    pub max_advance: Option<f32>,
    pub runs: (u32, u32),
    pub clusters: (u32, u32),
}

impl LineData {
    pub fn size(&self) -> f32 {
        self.ascent + self.descent + self.leading
    }
}

#[derive(Clone, Debug, Default)]
pub struct LineLayoutData {
    pub lines: Vec<LineData>,
    pub runs: Vec<RunData>,
    pub clusters: Vec<(u32, f32)>,
}

impl LineLayoutData {
    #[inline]
    pub fn clear(&mut self) {
        self.lines.clear();
        self.runs.clear();
        self.clusters.clear();
    }

    #[inline]
    pub fn run_index_for_cluster(&self, cluster: u32) -> Option<usize> {
        for (i, run) in self.runs.iter().enumerate() {
            if cluster >= run.clusters.0 && cluster < run.clusters.1 {
                return Some(i);
            }
        }
        self.runs.len().checked_sub(1)
    }

    #[inline]
    pub fn run_data_for_cluster(&self, cluster: u32) -> Option<&RunData> {
        self.runs.get(self.run_index_for_cluster(cluster)?)
    }

    #[inline]
    pub fn line_index_for_cluster(&self, cluster: u32) -> usize {
        for (i, line) in self.lines.iter().enumerate() {
            if cluster >= line.clusters.0 && cluster < line.clusters.1 {
                return i;
            }
        }
        self.lines.len().saturating_sub(1)
    }

    #[inline]
    pub fn logical_to_visual(&self, cluster: u32) -> u32 {
        // FIXME: linear search
        for (i, x) in self.clusters.iter().enumerate() {
            if x.0 == cluster {
                return i as u32;
            }
        }
        0
    }

    pub fn visual_to_logical(&self, cluster: u32) -> u32 {
        let limit = self.clusters.len();
        if limit == 0 {
            return 0;
        }
        let index = (cluster as usize).min(limit - 1);
        self.clusters.get(index).map(|x| x.0).unwrap_or(0)
    }
}

pub struct BreakLines<'a> {
    layout: &'a mut LayoutData,
    lines: &'a mut LineLayoutData,
    state: BreakerState,
}

impl<'a> BreakLines<'a> {
    pub(super) fn new(layout: &'a mut LayoutData, lines: &'a mut LineLayoutData) -> Self {
        Self {
            layout,
            lines,
            state: BreakerState::default(),
        }
    }

    #[inline]
    pub fn break_without_advance_or_alignment(&'a mut self) {
        let run_len = self.layout.runs.len();
        let mut y = 0.;

        for i in 0..self.layout.runs.len() {
            let run = &self.layout.runs[i];

            let mut should_commit_line = false;

            if i == run_len - 1 {
                should_commit_line = true;
            } else {
                // If next run has a different line number then
                // try to commit line
                let next_run = &self.layout.runs[i + 1];
                if next_run.line != run.line {
                    should_commit_line = true;
                }
            }

            self.state.line.runs.1 = i as u32 + 1;
            // self.state.line.clusters.1 = self.state.j as u32;
            self.state.line.clusters.1 = run.clusters.1;

            if should_commit_line
                && commit_line(
                    self.layout,
                    self.lines,
                    &mut self.state.line,
                    None,
                    true,
                    run,
                    &mut y,
                )
            {
                self.state.runs = self.lines.runs.len();
                self.state.lines = self.lines.lines.len();
                self.state.line.x = 0.;
                // self.state.j += 1;
                self.state.line.clusters.1 = run.clusters.1 + 1;
            }
        }
    }
}

#[derive(Copy, Clone, Default)]
struct LineState {
    x: f32,
    runs: (u32, u32),
    clusters: (u32, u32),
}

#[derive(Copy, Clone, Default)]
struct BreakerState {
    runs: usize,
    lines: usize,
    line: LineState,
}

#[inline]
fn commit_line(
    layout: &LayoutData,
    lines: &mut LineLayoutData,
    state: &mut LineState,
    max_advance: Option<f32>,
    explicit: bool,
    run_data: &RunData,
    y: &mut f32,
) -> bool {
    state.clusters.1 = state.clusters.1.min(layout.clusters.len() as u32);
    if state.runs.0 == state.runs.1 || state.clusters.0 == state.clusters.1 {
        return false;
    }
    let line_index = lines.lines.len() as u32;
    let last_run = (state.runs.1 - state.runs.0) as usize - 1;
    let runs_start = lines.runs.len() as u32;
    for (i, run) in layout.runs[make_range(state.runs)].iter().enumerate() {
        let mut cluster_range = run.clusters;
        if i == 0 {
            cluster_range.0 = state.clusters.0;
        }
        if i == last_run {
            cluster_range.1 = state.clusters.1;
        }
        if cluster_range.0 >= cluster_range.1 {
            continue;
        }
        let mut copy = run.to_owned();
        copy.clusters = cluster_range;
        copy.line = line_index;
        lines.runs.push(copy);
    }
    let runs_end = lines.runs.len() as u32;
    if runs_start == runs_end {
        return false;
    }
    let mut line = LineData {
        runs: (runs_start, runs_end),
        clusters: state.clusters,
        width: state.x,
        max_advance,
        explicit_break: explicit,
        ascent: run_data.ascent.round(),
        descent: run_data.descent.round(),
        leading: (run_data.leading).round() * 2.,
        ..Default::default()
    };

    let above = line.ascent;
    let below = line.descent;
    line.baseline = *y + above;
    *y = line.baseline + below;

    lines.lines.push(line);
    state.clusters.0 = state.clusters.1;
    state.clusters.1 += 1;
    state.runs.0 = state.runs.1 - 1;

    true
}

// Copyright (c) 2023-present, Raphael Amorim.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//
// builder.rs was originally retired from dfrg/swash_demo licensed under MIT
// https://github.com/dfrg/swash_demo/blob/master/LICENSE

//! Render data builder.

use super::builder_data::*;
use super::MAX_ID;
use crate::font::{FontContext, FontLibrary, FontLibraryData};
use crate::font_introspector::shape::cluster::GlyphCluster;
use crate::font_introspector::shape::cluster::OwnedGlyphCluster;
use crate::font_introspector::shape::ShapeContext;
use crate::font_introspector::text::cluster::{CharCluster, CharInfo, Parser, Token};
use crate::font_introspector::text::{analyze, Script};
use crate::font_introspector::{Setting, Synthesis};
use crate::layout::render_data::{RenderData, RunCacheEntry};
use lru::LruCache;
use std::num::NonZeroUsize;

pub struct RunCache {
    inner: LruCache<u64, RunCacheEntry>,
}

impl RunCache {
    #[inline]
    fn new() -> Self {
        Self {
            inner: LruCache::new(NonZeroUsize::new(256).unwrap()),
        }
    }

    #[inline]
    fn put(&mut self, line_hash: u64, data: RunCacheEntry) {
        if data.runs.is_empty() {
            return;
        }

        if let Some(line) = self.inner.get_mut(&line_hash) {
            *line = data;
        } else {
            self.inner.put(line_hash, data);
        }
    }
}

/// Context for paragraph layout.
pub struct LayoutContext {
    fcx: FontContext,
    fonts: FontLibrary,
    font_features: Vec<crate::font_introspector::Setting<u16>>,
    scx: ShapeContext,
    state: BuilderState,
    cache: RunCache,
    cache_analysis: LruCache<String, Vec<CharInfo>>,
    shaper_cache: ShaperCache,
}

impl LayoutContext {
    /// Creates a new layout context with the specified font library.
    pub fn new(font_library: &FontLibrary) -> Self {
        Self {
            fonts: font_library.clone(),
            fcx: FontContext::default(),
            scx: ShapeContext::new(),
            state: BuilderState::new(),
            cache: RunCache::new(),
            shaper_cache: ShaperCache::new(),
            font_features: vec![],
            cache_analysis: LruCache::new(NonZeroUsize::new(256).unwrap()),
        }
    }

    #[inline]
    pub fn font_library(&self) -> &FontLibrary {
        &self.fonts
    }

    #[inline]
    pub fn set_font_features(
        &mut self,
        font_features: Vec<crate::font_introspector::Setting<u16>>,
    ) {
        self.font_features = font_features;
    }

    /// Creates a new builder for computing a paragraph layout with the
    /// specified direction, language and scaling factor.
    #[inline]
    pub fn builder(&mut self, scale: f32, font_size: f32) -> ParagraphBuilder {
        self.state.clear();
        self.state.begin();
        let prev_font_size = self.state.font_size;
        self.state.scale = scale;
        self.state.font_size = font_size * scale;

        if prev_font_size != self.state.font_size {
            self.cache.inner.clear();
            self.shaper_cache.clear();
        }
        ParagraphBuilder {
            fcx: &mut self.fcx,
            // bidi: &mut self.bidi,
            // needs_bidi: false,
            font_features: &self.font_features,
            fonts: &self.fonts,
            scx: &mut self.scx,
            s: &mut self.state,
            last_offset: 0,
            cache: &mut self.cache,
            shaper_cache: &mut self.shaper_cache,
            cache_analysis: &mut self.cache_analysis,
        }
    }

    #[inline]
    pub fn clear_cache(&mut self) {
        self.cache.inner.clear();
    }
}

/// Builder for computing the layout of a paragraph.
pub struct ParagraphBuilder<'a> {
    fcx: &'a mut FontContext,
    fonts: &'a FontLibrary,
    font_features: &'a Vec<crate::font_introspector::Setting<u16>>,
    scx: &'a mut ShapeContext,
    s: &'a mut BuilderState,
    last_offset: u32,
    cache: &'a mut RunCache,
    shaper_cache: &'a mut ShaperCache,
    cache_analysis: &'a mut LruCache<String, Vec<CharInfo>>,
}

impl<'a> ParagraphBuilder<'a> {
    #[inline]
    pub fn set_hash(&mut self, hash: u64) {
        let current_line = self.s.current_line();
        self.s.lines[current_line].hash = hash;
    }

    #[inline]
    pub fn new_line(&mut self) {
        self.s.new_line();
    }

    /// Adds a text fragment to the paragraph.
    pub fn add_text(&mut self, text: &str, style: FragmentStyle) -> Option<()> {
        let current_line = self.s.current_line();
        let line = &mut self.s.lines[current_line];
        let id = line.text.frags.len();
        if id > MAX_ID {
            return None;
        }

        // If the text is just space then break shaping
        // let mut break_shaping = text.trim().is_empty();

        let mut offset = self.last_offset;
        line.styles.push(style);
        let span_id = line.styles.len() - 1;

        macro_rules! push_char {
            ($ch: expr) => {{
                line.text.content.push($ch);
                line.text.offsets.push(offset);
                offset += ($ch).len_utf8() as u32;
            }};
        }

        let start = line.text.content.len();

        for ch in text.chars() {
            push_char!(ch);
        }

        let break_shaping = if let Some(prev_frag) = line.fragments.last() {
            let prev_style = line.styles[prev_frag.span];
            prev_style != style
        } else {
            true
        };

        let end = line.text.content.len();
        let len = end - start;
        line.text.frags.reserve(len);
        for _ in 0..len {
            line.text.frags.push(id as u32);
        }

        line.text.spans.reserve(len);
        for _ in 0..len {
            line.text.spans.push(span_id);
        }

        line.fragments.push(FragmentData {
            span: span_id,
            break_shaping,
            start,
            end,
            vars: style.font_vars,
        });

        self.last_offset = offset;
        Some(())
    }

    /// Consumes the builder and fills the specified paragraph with the result.
    pub fn build_into(mut self, render_data: &mut RenderData) {
        self.resolve(render_data);
    }

    /// Consumes the builder and returns the resulting paragraph.
    pub fn build(self) -> RenderData {
        let mut render_data = RenderData::default();
        self.build_into(&mut render_data);
        render_data
    }
}

impl<'a> ParagraphBuilder<'a> {
    #[inline]
    fn process_from_cache(
        &mut self,
        render_data: &mut RenderData,
        current_line: usize,
    ) -> bool {
        let hash = self.s.lines[current_line].hash;
        if let Some(data) = self.cache.inner.get(&hash) {
            render_data.push_run_from_cached_line(data, current_line as u32);

            return true;
        }

        false
    }

    fn resolve(&mut self, render_data: &mut RenderData) {
        // let start = std::time::Instant::now();
        for line_number in 0..self.s.lines.len() {
            // In case should render only requested lines
            // and the line number isn't part of the requested then process from cache
            // if render_specific_lines && !lines_to_render.contains(&line_number) {
            if self.process_from_cache(render_data, line_number) {
                continue;
            }

            let line = &mut self.s.lines[line_number];
            let content_key = line.text.content.iter().collect();
            if let Some(cached_analysis) = self.cache_analysis.get(&content_key) {
                line.text.info.extend_from_slice(cached_analysis);
            } else {
                let mut analysis = analyze(line.text.content.iter());
                let mut cache = Vec::with_capacity(line.text.content.len());
                for (props, boundary) in analysis.by_ref() {
                    let char_info = CharInfo::new(props, boundary);
                    line.text.info.push(char_info);
                    cache.push(char_info);
                }
                self.cache_analysis.put(content_key, cache);
            }

            self.itemize(line_number);
            // let start = std::time::Instant::now();
            self.shape(render_data, line_number);
            // let duration = start.elapsed();
            // println!("Time elapsed in shape is: {:?}", duration);
        }
        // let duration = start.elapsed();
        // println!("Time elapsed in resolve is: {:?}", duration);
    }

    #[inline]
    fn itemize(&mut self, line_number: usize) {
        let line = &mut self.s.lines[line_number];
        let limit = line.text.content.len();
        if line.text.frags.is_empty() || limit == 0 {
            return;
        }
        // let mut last_script = line
        //     .text
        //     .info
        //     .iter()
        //     .map(|i| i.script())
        //     .find(|s| real_script(*s))
        //     .unwrap_or(Script::Latin);
        let mut last_frag = line.fragments.first().unwrap();
        // let last_level = 0;
        let mut last_vars = last_frag.vars;
        let mut item = ItemData {
            // script: last_script,
            // level: last_level,
            start: last_frag.start,
            end: last_frag.start,
            vars: last_vars,
        };
        macro_rules! push_item {
            () => {
                if item.start < limit && item.start < item.end {
                    // item.script = last_script;
                    // item.level = last_level;
                    item.vars = last_vars;
                    line.items.push(item);
                    item.start = item.end;
                }
            };
        }
        for frag in &line.fragments {
            if frag.break_shaping || frag.start != last_frag.end {
                push_item!();
                item.start = frag.start;
                item.end = frag.start;
            }
            last_frag = frag;
            last_vars = frag.vars;
            let range = frag.start..frag.end;
            // for &props in &line.text.info[range] {
            for &_props in &line.text.info[range] {
                //     let script = props.script();
                // let real = real_script(script);
                // if script != last_script && real {
                //     //item.end += 1;
                //     // push_item!();
                //     if real {
                //         last_script = script;
                //     }
                // } else {
                item.end += 1;
                // }
            }
        }
        push_item!();
    }

    #[inline]
    fn shape(&mut self, render_data: &mut RenderData, current_line: usize) {
        // let start = std::time::Instant::now();
        let mut char_cluster = CharCluster::new();
        let line = &self.s.lines[current_line];
        let font_library = { &self.fonts.inner.read().unwrap() };
        for item in &line.items {
            let range = item.start..item.end;
            let span_index = self.s.lines[current_line].text.spans[item.start];
            let style = self.s.lines[current_line].styles[span_index];
            let vars = self.s.vars.get(item.vars);
            let mut shape_state = ShapeState {
                // script: item.script,
                script: Script::Latin,
                features: self.font_features,
                vars,
                synth: Synthesis::default(),
                state: self.s,
                span: &self.s.lines[current_line].styles[span_index],
                font_id: None,
                size: self.s.font_size,
            };

            let chars = self.s.lines[current_line].text.content[range.to_owned()]
                .iter()
                .zip(&self.s.lines[current_line].text.offsets[range.to_owned()])
                .zip(&self.s.lines[current_line].text.info[range])
                .map(|z| {
                    let ((&ch, &offset), &info) = z;
                    Token {
                        ch,
                        offset,
                        len: ch.len_utf8() as u8,
                        info,
                        data: 0,
                    }
                });

            let mut parser = Parser::new(Script::Latin, chars);
            if !parser.next(&mut char_cluster) {
                continue;
            }
            shape_state.font_id = self.fcx.map_cluster(
                &mut char_cluster,
                &mut shape_state.synth,
                font_library,
                &style,
            );

            while shape_clusters(
                self.fcx,
                font_library,
                self.scx,
                &mut shape_state,
                &mut parser,
                &mut char_cluster,
                render_data,
                current_line,
                self.shaper_cache,
            ) {}

            self.cache.put(
                self.s.lines[current_line].hash,
                render_data.last_cached_run.to_owned(),
            );
        }
        // let duration = start.elapsed();
        // println!("Time elapsed in shape is: {:?}", duration);
    }
}

struct ShapeState<'a> {
    state: &'a BuilderState,
    features: &'a [Setting<u16>],
    synth: Synthesis,
    vars: &'a [Setting<f32>],
    script: Script,
    span: &'a FragmentStyle,
    font_id: Option<usize>,
    size: f32,
}

pub struct ShaperCache {
    pub inner: LruCache<String, Vec<OwnedGlyphCluster>>,
    stash: Vec<OwnedGlyphCluster>,
    key: String,
}

impl ShaperCache {
    pub fn new() -> Self {
        ShaperCache {
            inner: LruCache::new(NonZeroUsize::new(256).unwrap()),
            stash: vec![],
            key: String::new(),
        }
    }

    #[inline]
    pub fn shape_with(&mut self) -> Option<&Vec<OwnedGlyphCluster>> {
        if self.key.is_empty() {
            return None;
        }

        self.inner.get(&self.key)
    }

    #[inline]
    fn add_cluster(&mut self, chars: &[crate::font_introspector::text::cluster::Char]) {
        for character in chars {
            self.key.push(character.ch);
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.stash.clear();
        self.key.clear();
        self.inner.clear();
    }

    #[inline]
    pub fn add_glyph_cluster(&mut self, glyph_cluster: &GlyphCluster) {
        self.stash.push(glyph_cluster.into());
    }

    #[inline]
    pub fn finish(&mut self) {
        if !self.key.is_empty() && !self.stash.is_empty() {
            self.inner.put(
                std::mem::take(&mut self.key),
                std::mem::take(&mut self.stash),
            );
        } else {
            self.stash.clear();
            self.key.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn shape_clusters<I>(
    fcx: &mut FontContext,
    fonts: &FontLibraryData,
    scx: &mut ShapeContext,
    state: &mut ShapeState,
    parser: &mut Parser<I>,
    cluster: &mut CharCluster,
    render_data: &mut RenderData,
    current_line: usize,
    shaper_cache: &mut ShaperCache,
) -> bool
where
    I: Iterator<Item = Token> + Clone,
{
    if state.font_id.is_none() {
        return false;
    }

    let current_font_id = state.font_id.unwrap();
    let mut shaper = scx
        .builder(fonts[current_font_id].as_ref())
        .script(state.script)
        .size(state.size)
        .features(state.features.iter().copied())
        .variations(state.synth.variations().iter().copied())
        .variations(state.vars.iter().copied())
        .build();

    let mut synth = Synthesis::default();
    loop {
        shaper_cache.add_cluster(cluster.chars());
        shaper.add_cluster(cluster);

        if !parser.next(cluster) {
            render_data.push_run(
                state.span,
                &current_font_id,
                state.size,
                current_line as u32,
                state.state.lines[current_line].hash,
                shaper,
                shaper_cache,
            );
            return false;
        }

        let next_font = fcx.map_cluster(cluster, &mut synth, fonts, state.span);
        if next_font != state.font_id || synth != state.synth {
            render_data.push_run(
                state.span,
                &current_font_id,
                state.size,
                current_line as u32,
                state.state.lines[current_line].hash,
                shaper,
                shaper_cache,
            );
            state.font_id = next_font;
            state.synth = synth;
            return true;
        }
    }
}
